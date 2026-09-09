//! Framework-neutral wgpu resources and command submission.
//!
//! Handles are monotonically allocated across all devices. Removing a handle never
//! reassigns it. Command buffers retain their underlying wgpu resources, and no
//! registry lock is held while waiting for the GPU.

#![allow(
    clippy::missing_errors_doc,
    reason = "internal protocol returns descriptive validation errors"
)]

use parking_lot::Mutex;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

mod commands;
mod descriptors;
#[cfg(target_os = "macos")]
mod metal;
pub(crate) mod presentation;

static NEXT_RESOURCE_ID: AtomicU32 = AtomicU32::new(0);

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
pub(crate) enum Resource {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
    View(wgpu::TextureView),
    Sampler(wgpu::Sampler),
    Shader(wgpu::ShaderModule),
    BindGroupLayout(wgpu::BindGroupLayout),
    BindGroup(wgpu::BindGroup),
    PipelineLayout(wgpu::PipelineLayout),
    RenderPipeline(wgpu::RenderPipeline),
    ComputePipeline(wgpu::ComputePipeline),
    CommandBuffer(Arc<Mutex<Option<wgpu::CommandBuffer>>>),
    QuerySet(wgpu::QuerySet),
}

/// One device's resources. Applications must retain this owner until shutdown.
pub struct GpuEngine {
    pub device: Arc<wgpu::Device>,
    owns_device: bool,
    shutdown: Arc<event_listener::Event>,
    pub queue: Arc<wgpu::Queue>,
    resources: Mutex<HashMap<u32, Resource>>,
    destroyed: AtomicBool,
    pub(crate) loss: Arc<Mutex<Option<Value>>>,
    #[cfg_attr(
        target_family = "wasm",
        allow(
            dead_code,
            reason = "browser reports uncaptured errors on its real GPUDevice"
        )
    )]
    pub(crate) errors: Arc<Mutex<Vec<String>>>,
}

impl GpuEngine {
    #[must_use]
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, owns_device: bool) -> Arc<Self> {
        let loss = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(event_listener::Event::new());
        let errors = Arc::new(Mutex::new(Vec::new()));
        if owns_device {
            let state = loss.clone();
            let signal = shutdown.clone();
            device.set_device_lost_callback(move |reason, message| {
                *state.lock() = Some(json!({"reason": if reason == wgpu::DeviceLostReason::Destroyed {"destroyed"} else {"unknown"}, "message": message}));
                signal.notify(usize::MAX);
            });
            let state = errors.clone();
            device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
                let mut errors = state.lock();
                if errors.len() < 64 {
                    let kind = match &error {
                        wgpu::Error::Validation { .. } => "validation",
                        wgpu::Error::OutOfMemory { .. } => "out-of-memory",
                        wgpu::Error::Internal { .. } => "internal",
                    };
                    errors.push(format!("{kind}: {error}"));
                }
            }));
        }
        let engine = Arc::new(Self {
            device,
            queue,
            owns_device,
            shutdown,
            resources: Mutex::new(HashMap::new()),
            destroyed: AtomicBool::new(false),
            loss,
            errors,
        });
        #[cfg(not(target_family = "wasm"))]
        {
            let weak = Arc::downgrade(&engine);
            std::thread::spawn(move || {
                loop {
                    let Some(engine) = weak.upgrade() else {
                        break;
                    };
                    if let Err(error) = engine.device.poll(wgpu::PollType::Poll) {
                        let mut errors = engine.errors.lock();
                        if errors.len() < 64 {
                            errors.push(format!("internal: {error}"));
                        }
                    }
                    if engine.alive().is_err() {
                        break;
                    }
                    drop(engine);
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            });
        }
        engine
    }

    pub fn alive(&self) -> Result<()> {
        if self.destroyed.load(Ordering::Acquire) || self.loss.lock().is_some() {
            return Err("GPU device is lost or destroyed".into());
        }
        Ok(())
    }

    fn insert(&self, resource: Resource) -> Result<u32> {
        self.alive()?;
        let id = NEXT_RESOURCE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "GPU handle space exhausted")?
            + 1;
        let mut resources = self.resources.lock();
        self.alive()?;
        if resources.len() >= 65536 {
            return Err("GPU device resource limit (65536) exceeded".into());
        }
        resources.insert(id, resource);
        Ok(id)
    }

    fn resource(&self, id: u32) -> Result<Resource> {
        self.alive()?;
        self.resources
            .lock()
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("Unknown, released, or foreign GPU handle {id}"))
    }

    pub fn release(&self, id: u32) {
        self.resources.lock().remove(&id);
    }

    pub fn destroy_resource(&self, id: u32) -> Result<()> {
        if self.destroyed.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(resource) = self.resources.lock().get(&id).cloned() else {
            return Ok(());
        };
        match resource {
            Resource::Buffer(buffer) => buffer.destroy(),
            Resource::Texture(texture) => texture.destroy(),
            Resource::QuerySet(_) => self.release(id),
            _ => return Err("This GPU resource has no destroy operation".into()),
        }
        self.release(id);
        Ok(())
    }

    /// Invalidate every handle before releasing resources. Idempotent.
    pub fn destroy(&self) {
        if self.destroyed.swap(true, Ordering::AcqRel) {
            return;
        }
        let resources = std::mem::take(&mut *self.resources.lock());
        for resource in resources.values() {
            if let Resource::Buffer(buffer) = resource {
                buffer.destroy();
            }
        }
        if self.owns_device {
            self.device.destroy();
        }
        self.shutdown.notify(usize::MAX);
        // GPUI may own this device. Engine disposal must not destroy GPUI's GPU.
        *self.loss.lock() = Some(json!({"reason":"destroyed", "message":"GPU engine destroyed"}));
    }

    pub async fn compilation_info(&self, id: u32) -> Result<Value> {
        let Resource::Shader(shader) = self.resource(id)? else {
            return Err("Expected shader module".into());
        };
        let info = shader.get_compilation_info().await;
        Ok(
            json!({"messages": info.messages.iter().map(|m| json!({"message":m.message,"type":match m.message_type { wgpu::CompilationMessageType::Error => "error",wgpu::CompilationMessageType::Warning => "warning",wgpu::CompilationMessageType::Info => "info" },"lineNum":m.location.map_or(0,|l|l.line_number),"linePos":m.location.map_or(0,|l|l.line_position),"offset":m.location.map_or(0,|l|l.offset),"length":m.location.map_or(0,|l|l.length)})).collect::<Vec<_>>() }),
        )
    }

    pub fn resource_count(&self) -> usize {
        self.resources.lock().len()
    }

    pub fn texture(&self, id: u32) -> Result<wgpu::Texture> {
        match self.resource(id)? {
            Resource::Texture(v) => Ok(v),
            _ => Err("Expected GPUTexture".into()),
        }
    }
    fn buffer(&self, id: u32) -> Result<wgpu::Buffer> {
        match self.resource(id)? {
            Resource::Buffer(v) => Ok(v),
            _ => Err("Expected GPUBuffer".into()),
        }
    }
    fn view(&self, id: u32) -> Result<wgpu::TextureView> {
        match self.resource(id)? {
            Resource::View(v) => Ok(v),
            _ => Err("Expected GPUTextureView".into()),
        }
    }

    pub fn write_buffer(&self, id: u32, offset: u64, data: &[u8]) -> Result<()> {
        let buffer = self.buffer(id)?;
        if offset
            .checked_add(data.len() as u64)
            .is_none_or(|end| end > buffer.size())
        {
            return Err("Buffer write is out of bounds".into());
        }
        self.queue.write_buffer(&buffer, offset, data);
        Ok(())
    }

    pub fn write_texture(
        &self,
        destination: &Value,
        data: &[u8],
        layout: &Value,
        size: &Value,
    ) -> Result<()> {
        let texture = self.texture(number(destination, "texture")?)?;
        self.queue.write_texture(
            copy_texture(&texture, destination)?,
            data,
            copy_layout(layout)?,
            extent(size)?,
        );
        Ok(())
    }

    pub fn mapped_bytes(&self, id: u32, offset: u64, size: u64) -> Result<Vec<u8>> {
        let buffer = self.buffer(id)?;
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= buffer.size())
            .ok_or("Mapped range is out of bounds")?;
        Ok(buffer.slice(offset..end).get_mapped_range().to_vec())
    }

    pub fn write_mapped(&self, id: u32, offset: u64, bytes: &[u8]) -> Result<()> {
        let buffer = self.buffer(id)?;
        let end = offset
            .checked_add(bytes.len() as u64)
            .filter(|end| *end <= buffer.size())
            .ok_or("Mapped range is out of bounds")?;
        buffer
            .slice(offset..end)
            .get_mapped_range_mut()
            .copy_from_slice(bytes);
        Ok(())
    }
    pub fn unmap(&self, id: u32) -> Result<()> {
        if self.destroyed.load(Ordering::Acquire) {
            return Ok(());
        }
        self.buffer(id)?.unmap();
        Ok(())
    }

    pub async fn map(&self, id: u32, mode: u32, offset: u64, size: u64) -> Result<()> {
        let buffer = self.buffer(id)?;
        let end = offset
            .checked_add(size)
            .filter(|end| *end <= buffer.size())
            .ok_or("Mapped range is out of bounds")?;
        let mode = match mode {
            1 => wgpu::MapMode::Read,
            2 => wgpu::MapMode::Write,
            _ => return Err("Invalid map mode".into()),
        };
        let listener = self.shutdown.listen();
        self.alive()?;
        let (send, receive) = futures::channel::oneshot::channel();
        buffer.slice(offset..end).map_async(mode, move |result| {
            let _ = send.send(result);
        });
        match futures::future::select(receive, Box::pin(listener)).await {
            futures::future::Either::Left((result, _)) => result
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string()),
            futures::future::Either::Right(_) => Err("GPU device lost during mapping".into()),
        }
    }

    pub async fn submitted_work_done(&self) -> Result<()> {
        let listener = self.shutdown.listen();
        self.alive()?;
        let (send, receive) = futures::channel::oneshot::channel();
        self.queue.on_submitted_work_done(move || {
            let _ = send.send(());
        });
        match futures::future::select(receive, Box::pin(listener)).await {
            futures::future::Either::Left((result, _)) => result.map_err(|e| e.to_string())?,
            futures::future::Either::Right(_) => {
                return Err("GPU device lost during submission".into());
            }
        }
        self.alive()
    }

    pub fn submit(&self, ids: &[u32]) -> Result<()> {
        // Validate the entire submission before consuming any command buffers.
        let mut unique = std::collections::HashSet::new();
        let mut buffers = Vec::with_capacity(ids.len());
        for id in ids {
            if !unique.insert(id) {
                return Err("Duplicate command buffer in submission".into());
            }
            let Resource::CommandBuffer(buffer) = self.resource(*id)? else {
                return Err("Expected GPUCommandBuffer".into());
            };
            if buffer.lock().is_none() {
                return Err("Command buffer already submitted".into());
            }
            buffers.push(buffer);
        }
        let commands = buffers
            .iter()
            .map(|b| {
                b.lock()
                    .take()
                    .ok_or("Command buffer already submitted".into())
            })
            .collect::<Result<Vec<_>>>()?;
        self.queue.submit(commands);
        Ok(())
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}
fn number(value: &Value, key: &str) -> Result<u32> {
    value[key]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("Expected integer {key}"))
}
fn id(value: &Value) -> Result<u32> {
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("Expected resource handle".into())
}
fn label(value: &Value) -> Option<&str> {
    value["label"].as_str()
}
fn extent(value: &Value) -> Result<wgpu::Extent3d> {
    if let Some(a) = value.as_array() {
        Ok(wgpu::Extent3d {
            width: a
                .first()
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("Invalid texture width")?,
            height: a.get(1).map_or(Ok(1), id)?,
            depth_or_array_layers: a.get(2).map_or(Ok(1), id)?,
        })
    } else {
        Ok(wgpu::Extent3d {
            width: number(value, "width")?,
            height: value.get("height").map_or(Ok(1), id)?,
            depth_or_array_layers: value.get("depthOrArrayLayers").map_or(Ok(1), id)?,
        })
    }
}
fn copy_texture<'a>(
    texture: &'a wgpu::Texture,
    value: &Value,
) -> Result<wgpu::TexelCopyTextureInfo<'a>> {
    let origin = if let Some(a) = value["origin"].as_array() {
        wgpu::Origin3d {
            x: a.first().map_or(Ok(0), id)?,
            y: a.get(1).map_or(Ok(0), id)?,
            z: a.get(2).map_or(Ok(0), id)?,
        }
    } else {
        wgpu::Origin3d {
            x: value["origin"].get("x").map_or(Ok(0), id)?,
            y: value["origin"].get("y").map_or(Ok(0), id)?,
            z: value["origin"].get("z").map_or(Ok(0), id)?,
        }
    };
    Ok(wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: value.get("mipLevel").map_or(Ok(0), id)?,
        origin,
        aspect: decode(value.get("aspect").cloned().unwrap_or(json!("all")))?,
    })
}

fn copy_layout(v: &Value) -> Result<wgpu::TexelCopyBufferLayout> {
    Ok(wgpu::TexelCopyBufferLayout {
        offset: v["offset"].as_u64().unwrap_or(0),
        bytes_per_row: v.get("bytesPerRow").map(id).transpose()?,
        rows_per_image: v.get("rowsPerImage").map(id).transpose()?,
    })
}
