use super::{
    Deserialize, DeserializeOwned, GpuEngine, HashMap, Resource, Result, Value, decode, extent, id,
    json, label, number,
};

fn with_defaults<T: DeserializeOwned + serde::Serialize + Default>(value: &Value) -> Result<T> {
    let mut defaults = serde_json::to_value(T::default()).map_err(|e| e.to_string())?;
    if let Some(fields) = value.as_object() {
        for (key, value) in fields {
            defaults[key] = value.clone();
        }
    }
    decode(defaults)
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("Expected {key} array"))
}
fn optional<T: DeserializeOwned>(v: &Value, key: &str) -> Result<Option<T>> {
    v.get(key)
        .filter(|v| !v.is_null())
        .map(|v| decode(v.clone()))
        .transpose()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VertexBuffer {
    array_stride: u64,
    #[serde(default)]
    step_mode: wgpu::VertexStepMode,
    attributes: Vec<wgpu::VertexAttribute>,
}

impl GpuEngine {
    #[allow(
        clippy::too_many_lines,
        reason = "explicit resource descriptor dispatch mirrors WebGPU resource kinds"
    )]
    pub fn create(&self, kind: &str, v: &Value) -> Result<u32> {
        self.alive()?;
        let resource = match kind {
            "buffer" => {
                let size = v["size"].as_u64().ok_or("Invalid buffer size")?;
                if size > self.device.limits().max_buffer_size {
                    return Err("Buffer exceeds device size limit".into());
                }
                Resource::Buffer(
                    self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: label(v),
                        size,
                        usage: wgpu::BufferUsages::from_bits(number(v, "usage")?)
                            .ok_or("Invalid buffer usage")?,
                        mapped_at_creation: v["mappedAtCreation"].as_bool().unwrap_or(false),
                    }),
                )
            }
            "texture" => {
                let formats: Vec<wgpu::TextureFormat> =
                    decode(v.get("viewFormats").cloned().unwrap_or(json!([])))?;
                Resource::Texture(
                    self.device.create_texture(&wgpu::TextureDescriptor {
                        label: label(v),
                        size: extent(&v["size"])?,
                        mip_level_count: v.get("mipLevelCount").map_or(Ok(1), id)?,
                        sample_count: v.get("sampleCount").map_or(Ok(1), id)?,
                        dimension: decode(v.get("dimension").cloned().unwrap_or(json!("2d")))?,
                        format: decode(v["format"].clone())?,
                        usage: wgpu::TextureUsages::from_bits(number(v, "usage")?)
                            .ok_or("Invalid texture usage")?,
                        view_formats: &formats,
                    }),
                )
            }
            "view" => {
                let texture = self.texture(number(v, "texture")?)?;
                Resource::View(texture.create_view(&wgpu::TextureViewDescriptor {
                    label: label(v),
                    format: optional(v, "format")?,
                    dimension: optional(v, "dimension")?,
                    usage: None,
                    aspect: decode(v.get("aspect").cloned().unwrap_or(json!("all")))?,
                    base_mip_level: v.get("baseMipLevel").map_or(Ok(0), id)?,
                    mip_level_count: optional(v, "mipLevelCount")?,
                    base_array_layer: v.get("baseArrayLayer").map_or(Ok(0), id)?,
                    array_layer_count: optional(v, "arrayLayerCount")?,
                }))
            }
            "sampler" => Resource::Sampler(
                self.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: label(v),
                    address_mode_u: decode(
                        v.get("addressModeU")
                            .cloned()
                            .unwrap_or(json!("clamp-to-edge")),
                    )?,
                    address_mode_v: decode(
                        v.get("addressModeV")
                            .cloned()
                            .unwrap_or(json!("clamp-to-edge")),
                    )?,
                    address_mode_w: decode(
                        v.get("addressModeW")
                            .cloned()
                            .unwrap_or(json!("clamp-to-edge")),
                    )?,
                    mag_filter: decode(v.get("magFilter").cloned().unwrap_or(json!("nearest")))?,
                    min_filter: decode(v.get("minFilter").cloned().unwrap_or(json!("nearest")))?,
                    mipmap_filter: decode(
                        v.get("mipmapFilter").cloned().unwrap_or(json!("nearest")),
                    )?,
                    lod_min_clamp: optional(v, "lodMinClamp")?.unwrap_or(0.),
                    lod_max_clamp: optional(v, "lodMaxClamp")?.unwrap_or(32.),
                    compare: optional(v, "compare")?,
                    anisotropy_clamp: optional(v, "maxAnisotropy")?.unwrap_or(1),
                    border_color: None,
                }),
            ),
            "shader" => Resource::Shader(self.device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: label(v),
                    source: wgpu::ShaderSource::Wgsl(
                        v["code"].as_str().ok_or("Expected WGSL code")?.into(),
                    ),
                },
            )),
            "querySet" => {
                Resource::QuerySet(self.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: label(v),
                    ty: decode(v["type"].clone())?,
                    count: number(v, "count")?,
                }))
            }
            "bindGroupLayout" => {
                let entries = array(v, "entries")?
                    .iter()
                    .map(binding_layout)
                    .collect::<Result<Vec<_>>>()?;
                Resource::BindGroupLayout(self.device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
                        label: label(v),
                        entries: &entries,
                    },
                ))
            }
            "pipelineLayout" => {
                let layouts = array(v, "bindGroupLayouts")?
                    .iter()
                    .map(|v| {
                        if v.is_null() {
                            return Ok(None);
                        }
                        match self.resource(id(v)?)? {
                            Resource::BindGroupLayout(layout) => Ok(Some(layout)),
                            _ => Err("Expected bind group layout".into()),
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                let references = layouts.iter().map(Option::as_ref).collect::<Vec<_>>();
                Resource::PipelineLayout(self.device.create_pipeline_layout(
                    &wgpu::PipelineLayoutDescriptor {
                        label: label(v),
                        bind_group_layouts: &references,
                        immediate_size: 0,
                    },
                ))
            }
            "bindGroup" => {
                let Resource::BindGroupLayout(layout) = self.resource(number(v, "layout")?)? else {
                    return Err("Expected bind group layout".into());
                };
                let entries = array(v, "entries")?;
                let resources = entries
                    .iter()
                    .map(|entry| {
                        self.resource(if entry["resource"].is_object() {
                            number(&entry["resource"], "buffer")?
                        } else {
                            id(&entry["resource"])?
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let bindings = entries
                    .iter()
                    .zip(&resources)
                    .map(|(entry, resource)| {
                        let resource = match resource {
                            Resource::Buffer(buffer) => {
                                wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer,
                                    offset: entry["resource"]["offset"].as_u64().unwrap_or(0),
                                    size: entry["resource"]["size"]
                                        .as_u64()
                                        .map(|s| {
                                            std::num::NonZeroU64::new(s)
                                                .ok_or("Buffer binding size cannot be zero")
                                        })
                                        .transpose()?,
                                })
                            }
                            Resource::View(view) => wgpu::BindingResource::TextureView(view),
                            Resource::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
                            _ => return Err("Unsupported binding resource".into()),
                        };
                        Ok(wgpu::BindGroupEntry {
                            binding: number(entry, "binding")?,
                            resource,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Resource::BindGroup(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: label(v),
                    layout: &layout,
                    entries: &bindings,
                }))
            }
            "renderPipeline" => self.render_pipeline(v)?,
            "computePipeline" => {
                let Resource::Shader(module) = self.resource(number(&v["compute"], "module")?)?
                else {
                    return Err("Expected shader module".into());
                };
                let layout = self.pipeline_layout(v)?;
                let constants = constants(&v["compute"])?;
                let constants_ref = constants
                    .iter()
                    .map(|(k, v)| (k.as_str(), *v))
                    .collect::<Vec<_>>();
                Resource::ComputePipeline(self.device.create_compute_pipeline(
                    &wgpu::ComputePipelineDescriptor {
                        label: label(v),
                        layout: layout.as_ref(),
                        module: &module,
                        entry_point: v["compute"]["entryPoint"].as_str(),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &constants_ref,
                            ..Default::default()
                        },
                        cache: None,
                    },
                ))
            }
            "pipelineBindGroupLayout" => {
                let pipeline = self.resource(number(v, "pipeline")?)?;
                let index = number(v, "index")?;
                Resource::BindGroupLayout(match pipeline {
                    Resource::RenderPipeline(p) => p.get_bind_group_layout(index),
                    Resource::ComputePipeline(p) => p.get_bind_group_layout(index),
                    _ => return Err("Expected pipeline".into()),
                })
            }
            _ => return Err(format!("Unsupported GPU resource kind: {kind}")),
        };
        self.insert(resource)
    }

    fn pipeline_layout(&self, v: &Value) -> Result<Option<wgpu::PipelineLayout>> {
        if v["layout"] == "auto" {
            return Ok(None);
        }
        match self.resource(number(v, "layout")?)? {
            Resource::PipelineLayout(v) => Ok(Some(v)),
            _ => Err("Expected pipeline layout or auto".into()),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "pipeline-owned descriptors must live through synchronous pipeline creation"
    )]
    fn render_pipeline(&self, v: &Value) -> Result<Resource> {
        let Resource::Shader(vertex) = self.resource(number(&v["vertex"], "module")?)? else {
            return Err("Expected vertex shader".into());
        };
        let fragment = if v["fragment"].is_object() {
            match self.resource(number(&v["fragment"], "module")?)? {
                Resource::Shader(v) => Some(v),
                _ => return Err("Expected fragment shader".into()),
            }
        } else {
            None
        };
        let layout = self.pipeline_layout(v)?;
        let buffers: Vec<VertexBuffer> =
            decode(v["vertex"].get("buffers").cloned().unwrap_or(json!([])))?;
        let buffers = buffers
            .iter()
            .map(|b| wgpu::VertexBufferLayout {
                array_stride: b.array_stride,
                step_mode: b.step_mode,
                attributes: &b.attributes,
            })
            .collect::<Vec<_>>();
        let targets = v["fragment"]["targets"]
            .as_array()
            .map(|targets| {
                targets
                    .iter()
                    .map(|t| {
                        if t.is_null() {
                            return Ok(None);
                        }
                        let blend = t
                            .get("blend")
                            .map(|b| {
                                Ok::<_, String>(wgpu::BlendState {
                                    color: with_defaults(&b["color"])?,
                                    alpha: with_defaults(&b["alpha"])?,
                                })
                            })
                            .transpose()?;
                        Ok(Some(wgpu::ColorTargetState {
                            format: decode(t["format"].clone())?,
                            blend,
                            write_mask: wgpu::ColorWrites::from_bits(
                                t.get("writeMask").map_or(Ok(15), id)?,
                            )
                            .ok_or("Invalid color write mask")?,
                        }))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let vc = constants(&v["vertex"])?;
        let fc = constants(&v["fragment"])?;
        let vc = vc.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>();
        let fc = fc.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>();
        let depth_stencil = v
            .get("depthStencil")
            .map(|d| {
                Ok::<_, String>(wgpu::DepthStencilState {
                    format: decode(d["format"].clone())?,
                    depth_write_enabled: d["depthWriteEnabled"].as_bool(),
                    depth_compare: optional(d, "depthCompare")?,
                    stencil: wgpu::StencilState {
                        front: with_defaults(&d["stencilFront"])?,
                        back: with_defaults(&d["stencilBack"])?,
                        read_mask: d.get("stencilReadMask").map_or(Ok(u32::MAX), id)?,
                        write_mask: d.get("stencilWriteMask").map_or(Ok(u32::MAX), id)?,
                    },
                    bias: wgpu::DepthBiasState {
                        constant: optional(d, "depthBias")?.unwrap_or(0),
                        slope_scale: optional(d, "depthBiasSlopeScale")?.unwrap_or(0.),
                        clamp: optional(d, "depthBiasClamp")?.unwrap_or(0.),
                    },
                })
            })
            .transpose()?;
        Ok(Resource::RenderPipeline(
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: label(v),
                    layout: layout.as_ref(),
                    vertex: wgpu::VertexState {
                        module: &vertex,
                        entry_point: v["vertex"]["entryPoint"].as_str(),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &vc,
                            ..Default::default()
                        },
                        buffers: &buffers,
                    },
                    primitive: with_defaults(&v["primitive"])?,
                    depth_stencil,
                    multisample: with_defaults(&v["multisample"])?,
                    fragment: fragment.as_ref().map(|module| wgpu::FragmentState {
                        module,
                        entry_point: v["fragment"]["entryPoint"].as_str(),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &fc,
                            ..Default::default()
                        },
                        targets: &targets,
                    }),
                    multiview_mask: None,
                    cache: None,
                }),
        ))
    }
}

fn constants(v: &Value) -> Result<HashMap<String, f64>> {
    decode(v.get("constants").cloned().unwrap_or(json!({})))
}
fn binding_layout(v: &Value) -> Result<wgpu::BindGroupLayoutEntry> {
    let ty = if let Some(b) = v.get("buffer") {
        wgpu::BindingType::Buffer {
            ty: match b["type"].as_str().unwrap_or("uniform") {
                "uniform" => wgpu::BufferBindingType::Uniform,
                "storage" => wgpu::BufferBindingType::Storage { read_only: false },
                "read-only-storage" => wgpu::BufferBindingType::Storage { read_only: true },
                _ => return Err("Invalid buffer binding type".into()),
            },
            has_dynamic_offset: b["hasDynamicOffset"].as_bool().unwrap_or(false),
            min_binding_size: b["minBindingSize"]
                .as_u64()
                .and_then(std::num::NonZeroU64::new),
        }
    } else if let Some(s) = v.get("sampler") {
        wgpu::BindingType::Sampler(decode(
            s.get("type").cloned().unwrap_or(json!("filtering")),
        )?)
    } else if let Some(t) = v.get("texture") {
        wgpu::BindingType::Texture {
            sample_type: match t["sampleType"].as_str().unwrap_or("float") {
                "float" => wgpu::TextureSampleType::Float { filterable: true },
                "unfilterable-float" => wgpu::TextureSampleType::Float { filterable: false },
                "depth" => wgpu::TextureSampleType::Depth,
                "sint" => wgpu::TextureSampleType::Sint,
                "uint" => wgpu::TextureSampleType::Uint,
                _ => return Err("Invalid texture sample type".into()),
            },
            view_dimension: decode(t.get("viewDimension").cloned().unwrap_or(json!("2d")))?,
            multisampled: t["multisampled"].as_bool().unwrap_or(false),
        }
    } else if let Some(t) = v.get("storageTexture") {
        wgpu::BindingType::StorageTexture {
            access: decode(t.get("access").cloned().unwrap_or(json!("write-only")))?,
            format: decode(t["format"].clone())?,
            view_dimension: decode(t.get("viewDimension").cloned().unwrap_or(json!("2d")))?,
        }
    } else {
        return Err("Unsupported binding layout (external textures are not available)".into());
    };
    Ok(wgpu::BindGroupLayoutEntry {
        binding: number(v, "binding")?,
        visibility: wgpu::ShaderStages::from_bits(number(v, "visibility")?)
            .ok_or("Invalid shader visibility")?,
        ty,
        count: None,
    })
}
