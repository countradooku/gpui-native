//! Bounded immutable GPU snapshots. Publication happens only after producer
//! completion. GPUI's scene and completion callback retain leases until sampling
//! finishes, so pool reuse cannot overwrite an in-flight texture.
use super::{Arc, GpuEngine, Mutex, Ordering, Result};
use std::sync::atomic::AtomicUsize;

const TOTAL_BUDGET: usize = 256 * 1024 * 1024;
static BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(not(target_family = "wasm"))]
pub(crate) fn allocated_bytes() -> u32 {
    u32::try_from(BYTES.load(Ordering::Acquire)).expect("snapshot budget fits u32")
}

struct Allocation(usize);
impl Allocation {
    fn reserve(bytes: usize) -> Result<Self> {
        BYTES
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes)
                    .filter(|total| *total <= TOTAL_BUDGET)
            })
            .map_err(|_| "GPU canvas snapshot budget (256 MiB) exceeded")?;
        Ok(Self(bytes))
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        BYTES.fetch_sub(self.0, Ordering::AcqRel);
    }
}

pub(crate) struct GpuFrame {
    pub texture: wgpu::Texture,
    pub opaque: bool,
    pub paint: Arc<dyn std::any::Any + Send + Sync>,
    _allocation: Allocation,
}
impl GpuFrame {
    fn new(engine: &GpuEngine, source: &wgpu::Texture, opaque: bool) -> Result<Self> {
        let bytes = source.width() as usize * source.height() as usize * 4;
        if bytes > 64 * 1024 * 1024 {
            return Err("GPU canvas frame exceeds 64 MiB".into());
        }
        let allocation = Allocation::reserve(bytes)?;
        let texture = engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GPUI immutable canvas snapshot"),
            size: source.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: source.format(),
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        #[cfg(target_os = "macos")]
        let paint = super::metal::share_texture(&texture)?;
        #[cfg(not(target_os = "macos"))]
        let paint: Arc<dyn std::any::Any + Send + Sync> = Arc::new(texture.clone());
        Ok(Self {
            texture,
            opaque,
            paint,
            _allocation: allocation,
        })
    }
}

#[derive(Default)]
struct State {
    epoch: u64,
    sequence: u64,
    published: u64,
    destroyed: bool,
    allocating: bool,
    slots: Vec<Arc<GpuFrame>>,
    latest: Option<Arc<GpuFrame>>,
    device: Option<Arc<wgpu::Device>>,
}
#[derive(Default)]
pub(crate) struct DirectSource {
    state: Mutex<State>,
}
impl DirectSource {
    pub fn reset(&self, destroy: bool) {
        let mut state = self.state.lock();
        state.epoch += 1;
        state.destroyed |= destroy;
        state.latest = None;
        // Old in-flight frames keep their budget reservations until completion.
        state.slots.clear();
        state.device = None;
    }
    pub fn latest(&self) -> Option<Arc<GpuFrame>> {
        self.state.lock().latest.clone()
    }

    pub async fn present(
        &self,
        engine: &GpuEngine,
        texture: wgpu::Texture,
        opaque: bool,
    ) -> Result<bool> {
        engine.alive()?;
        if texture.sample_count() != 1
            || texture.dimension() != wgpu::TextureDimension::D2
            || texture.depth_or_array_layers() != 1
            || !matches!(
                texture.format(),
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
            )
            || !texture.usage().contains(wgpu::TextureUsages::COPY_SRC)
        {
            return Err(
                "Direct canvas requires a COPY_SRC single-sample RGBA8/BGRA8 2D texture".into(),
            );
        }
        let (epoch, sequence, reuse) = {
            let mut state = self.state.lock();
            if state.destroyed {
                return Err("Canvas source destroyed".into());
            }
            if state
                .device
                .as_ref()
                .is_some_and(|d| !Arc::ptr_eq(d, &engine.device))
            {
                return Err("Reset canvas before replacing its GPU device".into());
            }
            state.device = Some(engine.device.clone());
            state.sequence += 1;
            let reuse = state
                .slots
                .iter()
                .find(|slot| {
                    Arc::strong_count(slot) == 1
                        && Arc::strong_count(&slot.paint) == 1
                        && slot.texture.size() == texture.size()
                        && slot.texture.format() == texture.format()
                        && slot.opaque == opaque
                })
                .cloned();
            if reuse.is_none() {
                if state.slots.len() >= 3 || state.allocating {
                    return Ok(false);
                }
                state.allocating = true;
            }
            (state.epoch, state.sequence, reuse)
        };
        let snapshot = if let Some(frame) = reuse {
            frame
        } else {
            // Allocation happens with no frame-store, tree, or source mutex held.
            let result = GpuFrame::new(engine, &texture, opaque).map(Arc::new);
            let mut state = self.state.lock();
            state.allocating = false;
            if state.epoch != epoch || state.destroyed {
                return Ok(false);
            }
            let frame = result?;
            state.slots.push(frame.clone());
            frame
        };
        let mut encoder = engine
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPUI canvas GPU snapshot"),
            });
        encoder.copy_texture_to_texture(
            texture.as_image_copy(),
            snapshot.texture.as_image_copy(),
            texture.size(),
        );
        engine.queue.submit([encoder.finish()]);
        // Cancellation can end publication before GPU completion. Keep the
        // allocation reservation charged while the producer still uses it.
        let producer_lease = snapshot.clone();
        engine
            .queue
            .on_submitted_work_done(move || drop(producer_lease));
        // An async completion notification is synchronization, not pixel readback.
        engine.submitted_work_done().await?;
        let mut state = self.state.lock();
        if state.destroyed || state.epoch != epoch || sequence <= state.published {
            return Ok(false);
        }
        state.published = sequence;
        state.latest = Some(snapshot);
        Ok(true)
    }
}
