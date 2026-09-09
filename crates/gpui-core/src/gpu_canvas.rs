//! Renderer-owned binary frame transport. WebGPU remains owned by the host.
//! Only the latest submitted image and the last painted image are retained.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui::RenderImage;

static NEXT_SOURCE_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct CanvasFrames {
    pub direct: HashMap<u32, Arc<crate::gpu::presentation::DirectSource>>,
    direct_painting: HashMap<u32, Arc<crate::gpu::presentation::GpuFrame>>,
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_family = "wasm"))]
    pub shared_gpu: Option<Arc<crate::gpu::GpuEngine>>,

    pub active: HashSet<u32>,
    frames: HashMap<u32, Option<Arc<RenderImage>>>,
    painted: HashMap<u32, Arc<RenderImage>>,
    painting: HashMap<u32, Arc<RenderImage>>,
    bytes: usize,
}

impl CanvasFrames {
    pub fn create(&mut self) -> Result<u32, &'static str> {
        if self.frames.len() >= 256 {
            return Err("at most 256 canvas sources per renderer");
        }
        let id = NEXT_SOURCE_ID
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |id| id.checked_add(1),
            )
            .map_err(|_| "canvas source IDs exhausted")?
            + 1;
        self.frames.insert(id, None);
        self.direct.insert(id, Arc::default());
        Ok(id)
    }

    pub fn publish_image(&mut self, id: u32, image: Arc<RenderImage>) -> Result<(), &'static str> {
        let old = self
            .frames
            .get(&id)
            .ok_or("unknown or destroyed canvas source")?;
        let total = self.bytes - image_bytes(old.as_ref()) + image_bytes(Some(&image));
        if total > MAX_TOTAL_BYTES {
            return Err("canvas frames exceed the renderer's 256 MiB budget");
        }
        self.frames.insert(id, Some(image));
        self.bytes = total;
        Ok(())
    }

    #[cfg(test)]
    #[allow(
        clippy::too_many_arguments,
        reason = "test helper for the flat binary bridge signature"
    )]
    fn publish(
        &mut self,
        id: u32,
        width: u32,
        height: u32,
        stride: u32,
        pixels: &[u8],
        bgra: bool,
        opaque: bool,
    ) -> Result<(), &'static str> {
        self.publish_image(
            id,
            decode_frame(width, height, stride, pixels, bgra, opaque)?,
        )
    }

    pub fn destroy(&mut self, id: u32) {
        if let Some(source) = self.direct.remove(&id) {
            source.reset(true);
        }
        if let Some(old) = self.frames.remove(&id) {
            self.bytes -= image_bytes(old.as_ref());
        }
    }

    pub fn clear(&mut self) {
        for source in self.direct.values() {
            source.reset(true);
        }
        self.direct.clear();
        self.direct_painting.clear();
        #[cfg(any(target_os = "linux", target_os = "freebsd", target_family = "wasm"))]
        {
            if let Some(engine) = self.shared_gpu.take() {
                engine.destroy();
            }
        }

        self.frames.clear();
        self.painted.clear();
        self.painting.clear();
        self.active.clear();
        self.bytes = 0;
    }

    fn freeze(&mut self) {
        // Publication may run concurrently with GPUI painting on Windows/Linux.
        // Freeze one generation for the entire frame so a second canvas using
        // the same source cannot evict an atlas tile already referenced by it.
        self.direct_painting = self
            .active
            .iter()
            .filter_map(|id| {
                self.direct
                    .get(id)
                    .and_then(|s| s.latest())
                    .map(|frame| (*id, frame))
            })
            .collect();
        self.painting.clear();
        self.painting.extend(self.active.iter().filter_map(|id| {
            self.frames
                .get(id)
                .and_then(Option::as_ref)
                .map(|image| (*id, image.clone()))
        }));
    }

    /// Evict obsolete atlas entries on the window thread, including unmounts.
    pub fn prepare_frame(&mut self, window: &mut gpui::Window) {
        #[cfg(any(target_os = "linux", target_os = "freebsd", target_family = "wasm"))]
        if window.gpu_device_lost() != Some(true) {
            if let Some(context) = window
                .gpu_context()
                .and_then(|v| v.downcast::<(Arc<wgpu::Device>, Arc<wgpu::Queue>)>().ok())
            {
                let (device, queue) = *context;
                if self.shared_gpu.as_ref().is_none_or(|engine| {
                    engine.alive().is_err() || !Arc::ptr_eq(&engine.device, &device)
                }) {
                    for source in self.direct.values() {
                        source.reset(false);
                    }
                    if let Some(old) = self.shared_gpu.take() {
                        old.destroy();
                    }
                    self.shared_gpu = Some(crate::gpu::GpuEngine::new(device, queue, false));
                }
            }
        } else {
            for source in self.direct.values() {
                source.reset(false);
            }
            if let Some(old) = self.shared_gpu.take() {
                old.destroy();
            }
        }

        self.freeze();
        self.painted.retain(|id, previous| {
            let keep = self.active.contains(id)
                && self
                    .painting
                    .get(id)
                    .is_some_and(|current| Arc::ptr_eq(current, previous));
            if !keep && let Err(error) = window.drop_image(previous.clone()) {
                log::warn!("canvas atlas eviction failed: {error}");
            }
            keep
        });
    }

    pub fn gpu_frame(&self, id: u32) -> Option<Arc<crate::gpu::presentation::GpuFrame>> {
        self.direct_painting.get(&id).cloned()
    }

    pub fn reset(&mut self, id: u32) -> Result<(), &'static str> {
        self.direct
            .get(&id)
            .ok_or("Unknown canvas source")?
            .reset(false);
        if let Some(old) = self.frames.get_mut(&id) {
            self.bytes -= image_bytes(old.as_ref());
            *old = None;
        }
        Ok(())
    }

    pub fn image(&mut self, id: u32) -> Option<Arc<RenderImage>> {
        let image = self.painting.get(&id)?.clone();
        self.painted.insert(id, image.clone());
        Some(image)
    }
}

/// Decode without holding the tree or frame-store mutex; publication is a short
/// pointer swap so a large readback cannot stall layout or the paint thread.
pub(crate) fn decode_frame(
    width: u32,
    height: u32,
    stride: u32,
    pixels: &[u8],
    bgra: bool,
    opaque: bool,
) -> Result<Arc<RenderImage>, &'static str> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err("canvas dimensions must be between 1 and 8192");
    }
    let row = width as usize * 4;
    let bytes = row * height as usize;
    let stride = stride as usize;
    let required = stride
        .checked_mul(height as usize)
        .ok_or("canvas stride overflow")?;
    if stride < row
        || pixels.len() != required
        || bytes > MAX_FRAME_BYTES
        || required > MAX_FRAME_BYTES
    {
        return Err("invalid canvas pixel buffer, stride, or frame exceeds 64 MiB");
    }
    let mut packed = Vec::with_capacity(bytes);
    for line in pixels.chunks_exact(stride) {
        packed.extend_from_slice(&line[..row]);
    }
    for pixel in packed.as_chunks_mut::<4>().0 {
        if !bgra {
            pixel.swap(0, 2);
        }
        if opaque {
            pixel[3] = 255;
        } else {
            // GPUI's image shader blends straight alpha; WebGPU's canvas
            // contract supplies premultiplied alpha.
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                let value = (u32::from(*channel) * 255 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0);
                *channel = u8::try_from(value.min(255)).unwrap_or(255);
            }
        }
    }
    let buffer = image::RgbaImage::from_raw(width, height, packed).ok_or("invalid canvas image")?;
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

fn image_bytes(image: Option<&Arc<RenderImage>>) -> usize {
    image
        .and_then(|image| image.as_bytes(0))
        .map_or(0, <[u8]>::len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_publication_does_not_replace_an_image_during_paint() {
        let mut frames = CanvasFrames::default();
        let id = frames.create().unwrap();
        frames.active.insert(id);
        frames
            .publish(id, 1, 1, 4, &[1, 2, 3, 255], true, true)
            .unwrap();
        frames.freeze();
        let first = frames.image(id).unwrap();
        frames
            .publish(id, 1, 1, 4, &[4, 5, 6, 255], true, true)
            .unwrap();
        assert!(Arc::ptr_eq(&frames.image(id).unwrap(), &first));
        frames.freeze();
        assert!(!Arc::ptr_eq(&frames.image(id).unwrap(), &first));
    }

    #[test]
    fn padded_rgba_is_packed_and_swizzled_and_replaced() {
        let mut frames = CanvasFrames::default();
        let id = frames.create().unwrap();
        frames
            .publish(
                id,
                1,
                2,
                8,
                &[255, 0, 0, 255, 9, 9, 9, 9, 0, 0, 255, 128, 9, 9, 9, 9],
                false,
                false,
            )
            .unwrap();
        assert_eq!(
            frames
                .frames
                .get(&id)
                .and_then(Option::as_ref)
                .unwrap()
                .as_bytes(0)
                .unwrap(),
            &[0, 0, 255, 255, 255, 0, 0, 128]
        );
        frames
            .publish(id, 1, 1, 4, &[1, 2, 3, 255], true, false)
            .unwrap();
        assert_eq!(frames.bytes, 4);
        assert_eq!(
            frames
                .frames
                .get(&id)
                .and_then(Option::as_ref)
                .unwrap()
                .as_bytes(0)
                .unwrap(),
            &[1, 2, 3, 255]
        );
        frames.destroy(id);
        frames.destroy(id);
        assert_eq!(frames.bytes, 0);
        assert!(frames.frames.get(&id).and_then(Option::as_ref).is_none());
        assert!(frames.publish(id, 1, 1, 4, &[0; 4], true, false).is_err());
    }

    #[test]
    fn bad_frames_leave_last_good_frame_intact() {
        let mut frames = CanvasFrames::default();
        let id = frames.create().unwrap();
        frames
            .publish(id, 1, 1, 4, &[1, 2, 3, 255], true, false)
            .unwrap();
        for (w, h, stride, data) in [
            (0, 1, 4, vec![0; 4]),
            (8193, 1, 4, vec![0; 4]),
            (1, 1, 3, vec![0; 3]),
            (1, 2, u32::MAX, vec![]),
        ] {
            assert!(
                frames
                    .publish(id, w, h, stride, &data, true, false)
                    .is_err()
            );
            assert_eq!(
                frames
                    .frames
                    .get(&id)
                    .and_then(Option::as_ref)
                    .unwrap()
                    .as_bytes(0)
                    .unwrap(),
                &[1, 2, 3, 255]
            );
        }
        frames.clear();
        assert_eq!(frames.bytes, 0);
        assert!(frames.frames.get(&id).and_then(Option::as_ref).is_none());
        assert_ne!(frames.create().unwrap(), id);
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn renderer_source_ids_do_not_alias() {
        let mut first = CanvasFrames::default();
        let mut second = CanvasFrames::default();
        let id = first.create().unwrap();
        let other = second.create().unwrap();
        assert_ne!(id, other);
        assert!(second.reset(id).is_err());
        first.destroy(id);
        assert!(first.reset(id).is_err());
    }

    #[test]
    fn reset_releases_latest_cpu_frame_and_rejects_stale_sources() {
        let mut frames = CanvasFrames::default();
        let id = frames.create().unwrap();
        frames.publish(id, 1, 1, 4, &[255; 4], false, true).unwrap();
        frames.reset(id).unwrap();
        assert_eq!(frames.bytes, 0);
        assert!(frames.frames[&id].is_none());
        frames.destroy(id);
        assert!(frames.publish(id, 1, 1, 4, &[255; 4], false, true).is_err());
    }
}
