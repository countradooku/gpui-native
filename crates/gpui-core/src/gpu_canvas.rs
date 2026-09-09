//! Renderer-owned binary frame transport. WebGPU remains owned by the host.
//! Only the latest submitted image and the last painted image are retained.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui::RenderImage;

const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct CanvasFrames {
    next_id: u32,
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
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("canvas source IDs exhausted")?;
        self.frames.insert(self.next_id, None);
        Ok(self.next_id)
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
        if let Some(old) = self.frames.remove(&id) {
            self.bytes -= image_bytes(old.as_ref());
        }
    }

    pub fn clear(&mut self) {
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
