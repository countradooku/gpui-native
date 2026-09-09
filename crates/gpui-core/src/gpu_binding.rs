//! Node-API boundary shared by Node and Bun. No JavaScript engine handles cross
//! worker threads: asynchronous tasks own only Rust/wgpu data.
#![allow(
    clippy::needless_pass_by_value,
    reason = "Node-API owns boundary arguments"
)]
#![allow(
    clippy::missing_errors_doc,
    reason = "Node-API protocol failures are surfaced as JavaScript errors; public compatibility is documented in docs/webgpu.md"
)]
use crate::gpu::GpuEngine;
use napi::{Task, bindgen_prelude::*};
use napi_derive::napi;
use serde_json::{Value, json};
use std::sync::Arc;

fn error(e: impl ToString) -> Error {
    Error::from_reason(e.to_string())
}

#[napi]
pub struct NativeWgpuAdapter {
    adapter: wgpu::Adapter,
}

#[napi]
pub async fn request_wgpu_adapter(options: Value) -> Result<NativeWgpuAdapter> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: if options["powerPreference"] == "low-power" {
                wgpu::PowerPreference::LowPower
            } else {
                wgpu::PowerPreference::HighPerformance
            },
            force_fallback_adapter: options["forceFallbackAdapter"].as_bool().unwrap_or(false),
            compatible_surface: None,
        })
        .await
        .map_err(error)?;
    Ok(NativeWgpuAdapter { adapter })
}

fn feature_names(features: wgpu::Features) -> Vec<String> {
    // wgpu's WebGPU feature names are kebab-case; native-only flags are not exposed.
    let names = [
        ("depth-clip-control", wgpu::Features::DEPTH_CLIP_CONTROL),
        (
            "depth32float-stencil8",
            wgpu::Features::DEPTH32FLOAT_STENCIL8,
        ),
        (
            "texture-compression-bc",
            wgpu::Features::TEXTURE_COMPRESSION_BC,
        ),
        (
            "texture-compression-etc2",
            wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        ),
        (
            "texture-compression-astc",
            wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        ),
        (
            "indirect-first-instance",
            wgpu::Features::INDIRECT_FIRST_INSTANCE,
        ),
        ("shader-f16", wgpu::Features::SHADER_F16),
        (
            "rg11b10ufloat-renderable",
            wgpu::Features::RG11B10UFLOAT_RENDERABLE,
        ),
        ("bgra8unorm-storage", wgpu::Features::BGRA8UNORM_STORAGE),
        ("float32-filterable", wgpu::Features::FLOAT32_FILTERABLE),
        ("float32-blendable", wgpu::Features::FLOAT32_BLENDABLE),
        ("clip-distances", wgpu::Features::CLIP_DISTANCES),
        ("dual-source-blending", wgpu::Features::DUAL_SOURCE_BLENDING),
    ];
    names
        .iter()
        .filter(|(_, flag)| features.contains(*flag))
        .map(|(name, _)| (*name).to_string())
        .collect()
}
fn features_from(names: &[String], supported: wgpu::Features) -> Result<wgpu::Features> {
    let mut result = wgpu::Features::empty();
    for name in names {
        let flag = supported
            .iter()
            .find(|flag| feature_names(*flag).contains(name))
            .ok_or_else(|| error(format!("Unsupported feature {name}")))?;
        result |= flag;
    }
    Ok(result)
}

#[napi]
impl NativeWgpuAdapter {
    #[napi(getter)]
    #[must_use]
    pub fn info(&self) -> Value {
        let i = self.adapter.get_info();
        json!({"vendor":i.vendor.to_string(), "architecture":"", "device":i.device.to_string(), "description":i.name, "backend":format!("{:?}",i.backend), "isFallbackAdapter": i.device_type == wgpu::DeviceType::Cpu})
    }
    #[napi(getter)]
    #[must_use]
    pub fn features(&self) -> Vec<String> {
        feature_names(self.adapter.features())
    }
    #[napi(getter)]
    pub fn limits(&self) -> Result<Value> {
        serde_json::to_value(self.adapter.limits()).map_err(error)
    }
    #[napi]
    pub async fn request_device(&self, descriptor: Value) -> Result<NativeWgpuDevice> {
        let names: Vec<String> = serde_json::from_value(
            descriptor
                .get("requiredFeatures")
                .cloned()
                .unwrap_or(json!([])),
        )
        .map_err(error)?;
        let features = features_from(&names, self.adapter.features())?;
        let mut limits = serde_json::to_value(wgpu::Limits::default()).map_err(error)?;
        if let Some(requested) = descriptor["requiredLimits"].as_object() {
            for (key, value) in requested {
                if limits.get(key).is_none() {
                    return Err(error(format!("Unknown GPU limit {key}")));
                }
                limits[key] = value.clone();
            }
        }
        let (device, queue) = self
            .adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: descriptor["label"].as_str(),
                required_features: features,
                required_limits: serde_json::from_value(limits).map_err(error)?,
                ..Default::default()
            })
            .await
            .map_err(error)?;
        Ok(NativeWgpuDevice::new(GpuEngine::new(
            Arc::new(device),
            Arc::new(queue),
            true,
        )))
    }
}

#[napi]
pub struct NativeWgpuDevice {
    pub(crate) engine: Arc<GpuEngine>,
}
impl NativeWgpuDevice {
    pub(crate) fn new(engine: Arc<GpuEngine>) -> Self {
        Self { engine }
    }
}

// A task-local validation scope also captures errors from worker-thread pipeline
// compilation (wgpu 29 error scopes are thread-local).
fn validated<T>(engine: &GpuEngine, action: impl FnOnce() -> crate::gpu::Result<T>) -> Result<T> {
    let out_of_memory = engine
        .device
        .push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let internal = engine.device.push_error_scope(wgpu::ErrorFilter::Internal);
    let validation = engine
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let result = action();
    let validation_error = futures::executor::block_on(validation.pop());
    let internal_error = futures::executor::block_on(internal.pop());
    let memory_error = futures::executor::block_on(out_of_memory.pop());
    if let Some(e) = validation_error {
        return Err(error(format!("validation: {e}")));
    }
    if let Some(e) = internal_error {
        return Err(error(format!("internal: {e}")));
    }
    if let Some(e) = memory_error {
        return Err(error(format!("out-of-memory: {e}")));
    }
    result.map_err(error)
}

pub struct PipelineTask {
    engine: Arc<GpuEngine>,
    kind: String,
    descriptor: Value,
}
impl Task for PipelineTask {
    type Output = u32;
    type JsValue = u32;
    fn compute(&mut self) -> Result<u32> {
        let mut created = None;
        let result = validated(&self.engine, || {
            let id = self.engine.create(&self.kind, &self.descriptor)?;
            created = Some(id);
            Ok(id)
        });
        if result.is_err()
            && let Some(id) = created
        {
            self.engine.release(id);
        }
        result
    }
    fn resolve(&mut self, _: Env, output: u32) -> Result<u32> {
        Ok(output)
    }
}

#[napi]
impl NativeWgpuDevice {
    #[napi(getter)]
    #[must_use]
    pub fn features(&self) -> Vec<String> {
        feature_names(self.engine.device.features())
    }
    #[napi(getter)]
    pub fn limits(&self) -> Result<Value> {
        serde_json::to_value(self.engine.device.limits()).map_err(error)
    }
    #[napi(getter)]
    #[must_use]
    pub fn loss(&self) -> Option<Value> {
        self.engine.loss.lock().clone()
    }
    #[napi]
    #[must_use]
    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.engine.errors.lock())
    }
    #[napi]
    pub fn create(&self, kind: String, descriptor: Value) -> Result<u32> {
        let mut created = None;
        let result = validated(&self.engine, || {
            let id = self.engine.create(&kind, &descriptor)?;
            created = Some(id);
            Ok(id)
        });
        if result.is_err()
            && let Some(id) = created
        {
            self.engine.release(id);
        }
        result
    }
    #[napi]
    pub async fn compilation_info(&self, id: u32) -> Result<Value> {
        self.engine.compilation_info(id).await.map_err(error)
    }

    #[napi]
    pub fn create_pipeline_async(
        &self,
        kind: String,
        descriptor: Value,
    ) -> Result<AsyncTask<PipelineTask>> {
        if kind != "renderPipeline" && kind != "computePipeline" {
            return Err(error("Expected pipeline kind"));
        }
        self.engine.alive().map_err(error)?;
        Ok(AsyncTask::new(PipelineTask {
            engine: self.engine.clone(),
            kind,
            descriptor,
        }))
    }
    #[napi]
    pub fn encode(&self, descriptor: Value) -> Result<u32> {
        validated(&self.engine, || self.engine.encode(&descriptor))
    }
    #[napi]
    pub fn submit(&self, handles: Vec<u32>) -> Result<()> {
        validated(&self.engine, || self.engine.submit(&handles))
    }
    #[napi]
    pub fn write_buffer(&self, handle: u32, offset: i64, data: Uint8Array) -> Result<()> {
        validated(&self.engine, || {
            self.engine.write_buffer(
                handle,
                u64::try_from(offset).map_err(|e| e.to_string())?,
                &data,
            )
        })
    }
    #[napi]
    pub fn write_texture(
        &self,
        destination: Value,
        data: Uint8Array,
        layout: Value,
        size: Value,
    ) -> Result<()> {
        validated(&self.engine, || {
            self.engine
                .write_texture(&destination, &data, &layout, &size)
        })
    }
    #[napi]
    pub async fn map(&self, handle: u32, mode: u32, offset: i64, size: i64) -> Result<()> {
        self.engine
            .map(
                handle,
                mode,
                u64::try_from(offset).map_err(error)?,
                u64::try_from(size).map_err(error)?,
            )
            .await
            .map_err(error)
    }
    #[napi]
    pub fn mapped_bytes(&self, handle: u32, offset: i64, size: i64) -> Result<Uint8Array> {
        self.engine
            .mapped_bytes(
                handle,
                u64::try_from(offset).map_err(error)?,
                u64::try_from(size).map_err(error)?,
            )
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub fn write_mapped(&self, handle: u32, offset: i64, data: Uint8Array) -> Result<()> {
        self.engine
            .write_mapped(handle, u64::try_from(offset).map_err(error)?, &data)
            .map_err(error)
    }
    #[napi]
    pub fn unmap(&self, handle: u32) -> Result<()> {
        self.engine.unmap(handle).map_err(error)
    }
    #[napi]
    pub async fn submitted_work_done(&self) -> Result<()> {
        self.engine.submitted_work_done().await.map_err(error)
    }
    #[napi]
    pub fn release(&self, handle: u32) {
        self.engine.release(handle);
    }
    #[napi]
    pub fn destroy_resource(&self, handle: u32) -> Result<()> {
        self.engine.destroy_resource(handle).map_err(error)
    }
    #[napi]
    pub fn destroy(&self) {
        self.engine.destroy();
    }
    /// Process-wide canvas snapshots, including producer/compositor leases.
    #[napi(getter)]
    #[must_use]
    pub fn canvas_snapshot_bytes(&self) -> u32 {
        crate::gpu::presentation::allocated_bytes()
    }

    #[napi(getter)]
    #[must_use]
    pub fn resource_count(&self) -> u32 {
        u32::try_from(self.engine.resource_count()).unwrap_or(u32::MAX)
    }
}
