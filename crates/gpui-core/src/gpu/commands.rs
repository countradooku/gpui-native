use super::{
    Arc, GpuEngine, Mutex, Resource, Result, Value, copy_layout, copy_texture, decode, extent, id,
    json, label, number,
};

fn u(v: &Value, index: usize) -> Result<u32> {
    id(&v[index])
}
fn n(v: &Value, index: usize) -> Result<u64> {
    v[index].as_u64().ok_or("Expected unsigned integer".into())
}
fn f(v: &Value, index: usize) -> Result<f32> {
    decode(v[index].clone())
}
fn operation(v: &Value) -> Result<&str> {
    v[0].as_str().ok_or("Expected GPU operation name".into())
}
fn list(v: &Value) -> Result<&[Value]> {
    v.as_array()
        .map(Vec::as_slice)
        .ok_or("Expected GPU command list".into())
}

impl GpuEngine {
    /// Decode into an unsubmitted encoder; errors discard it without GPU work.
    pub fn encode(&self, value: &Value) -> Result<u32> {
        self.alive()?;
        let commands = list(&value["commands"])?;
        if commands.len() > 1_000_000 {
            return Err("GPU command batch too large".into());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: label(value),
            });
        for c in commands {
            match operation(c)? {
                "renderPass" => self.render_pass(&mut encoder, &c[1], &c[2])?,
                "computePass" => self.compute_pass(&mut encoder, &c[1], &c[2])?,
                "copyBufferToBuffer" => encoder.copy_buffer_to_buffer(
                    &self.buffer(u(c, 1)?)?,
                    n(c, 2)?,
                    &self.buffer(u(c, 3)?)?,
                    n(c, 4)?,
                    n(c, 5)?,
                ),
                "copyTextureToBuffer" => {
                    let texture = self.texture(number(&c[1], "texture")?)?;
                    let buffer = self.buffer(number(&c[2], "buffer")?)?;
                    encoder.copy_texture_to_buffer(
                        copy_texture(&texture, &c[1])?,
                        wgpu::TexelCopyBufferInfo {
                            buffer: &buffer,
                            layout: copy_layout(&c[2])?,
                        },
                        extent(&c[3])?,
                    );
                }
                "copyBufferToTexture" => {
                    let buffer = self.buffer(number(&c[1], "buffer")?)?;
                    let texture = self.texture(number(&c[2], "texture")?)?;
                    encoder.copy_buffer_to_texture(
                        wgpu::TexelCopyBufferInfo {
                            buffer: &buffer,
                            layout: copy_layout(&c[1])?,
                        },
                        copy_texture(&texture, &c[2])?,
                        extent(&c[3])?,
                    );
                }
                "copyTextureToTexture" => {
                    let a = self.texture(number(&c[1], "texture")?)?;
                    let b = self.texture(number(&c[2], "texture")?)?;
                    encoder.copy_texture_to_texture(
                        copy_texture(&a, &c[1])?,
                        copy_texture(&b, &c[2])?,
                        extent(&c[3])?,
                    );
                }
                "clearBuffer" => encoder.clear_buffer(
                    &self.buffer(u(c, 1)?)?,
                    n(c, 2)?,
                    c.get(3)
                        .filter(|v| !v.is_null())
                        .map(|v| v.as_u64().ok_or("Invalid clear size"))
                        .transpose()?,
                ),
                "resolveQuerySet" => {
                    let Resource::QuerySet(query) = self.resource(u(c, 1)?)? else {
                        return Err("Expected query set".into());
                    };
                    let end = u(c, 2)?
                        .checked_add(u(c, 3)?)
                        .ok_or("Query range overflow")?;
                    encoder.resolve_query_set(
                        &query,
                        u(c, 2)?..end,
                        &self.buffer(u(c, 4)?)?,
                        n(c, 5)?,
                    );
                }
                "insertDebugMarker" => {
                    encoder.insert_debug_marker(c[1].as_str().ok_or("Invalid marker")?);
                }
                "pushDebugGroup" => encoder.push_debug_group(c[1].as_str().ok_or("Invalid group")?),
                "popDebugGroup" => encoder.pop_debug_group(),
                op => return Err(format!("Unsupported encoder operation {op}")),
            }
        }
        self.insert(Resource::CommandBuffer(Arc::new(Mutex::new(Some(
            encoder.finish(),
        )))))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive WebGPU pass decoder keeps attachment lifetimes visible"
    )]
    fn render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        descriptor: &Value,
        commands: &Value,
    ) -> Result<()> {
        let attachments = list(&descriptor["colorAttachments"])?;
        let views = attachments
            .iter()
            .map(|a| {
                if a.is_null() {
                    Ok(None)
                } else {
                    self.view(number(a, "view")?).map(Some)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let resolves = attachments
            .iter()
            .map(|a| {
                a.get("resolveTarget")
                    .filter(|v| !v.is_null())
                    .map(|v| self.view(id(v)?))
                    .transpose()
            })
            .collect::<Result<Vec<_>>>()?;
        let colors = attachments
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let Some(view) = &views[i] else {
                    return Ok(None);
                };
                let clear = if let Some(a) = a["clearValue"].as_array() {
                    wgpu::Color {
                        r: a.first()
                            .and_then(Value::as_f64)
                            .ok_or("Invalid clear color")?,
                        g: a.get(1)
                            .and_then(Value::as_f64)
                            .ok_or("Invalid clear color")?,
                        b: a.get(2)
                            .and_then(Value::as_f64)
                            .ok_or("Invalid clear color")?,
                        a: a.get(3)
                            .and_then(Value::as_f64)
                            .ok_or("Invalid clear color")?,
                    }
                } else {
                    decode(
                        a.get("clearValue")
                            .cloned()
                            .unwrap_or(json!({"r":0.,"g":0.,"b":0.,"a":0.})),
                    )?
                };
                Ok(Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: a.get("depthSlice").map(id).transpose()?,
                    resolve_target: resolves[i].as_ref(),
                    ops: wgpu::Operations {
                        load: match a["loadOp"].as_str() {
                            Some("clear") => wgpu::LoadOp::Clear(clear),
                            Some("load") => wgpu::LoadOp::Load,
                            _ => return Err("Invalid color loadOp".into()),
                        },
                        store: decode(a["storeOp"].clone())?,
                    },
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        let ds = &descriptor["depthStencilAttachment"];
        let depth = ds.get("view").map(|v| self.view(id(v)?)).transpose()?;
        let depth_attachment = depth
            .as_ref()
            .map(|view| {
                Ok::<_, String>(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: if ds["depthReadOnly"].as_bool().unwrap_or(false)
                        || ds.get("depthLoadOp").is_none()
                    {
                        None
                    } else {
                        Some(wgpu::Operations {
                            load: match ds["depthLoadOp"].as_str() {
                                Some("clear") => wgpu::LoadOp::Clear(decode(
                                    ds.get("depthClearValue").cloned().unwrap_or(json!(1.)),
                                )?),
                                Some("load") => wgpu::LoadOp::Load,
                                _ => return Err("Invalid depth loadOp".into()),
                            },
                            store: decode(ds["depthStoreOp"].clone())?,
                        })
                    },
                    stencil_ops: if ds["stencilReadOnly"].as_bool().unwrap_or(false)
                        || ds.get("stencilLoadOp").is_none()
                    {
                        None
                    } else {
                        Some(wgpu::Operations {
                            load: match ds["stencilLoadOp"].as_str() {
                                Some("clear") => wgpu::LoadOp::Clear(
                                    ds.get("stencilClearValue").map_or(Ok(0), id)?,
                                ),
                                Some("load") => wgpu::LoadOp::Load,
                                _ => return Err("Invalid stencil loadOp".into()),
                            },
                            store: decode(ds["stencilStoreOp"].clone())?,
                        })
                    },
                })
            })
            .transpose()?;
        let query = descriptor
            .get("occlusionQuerySet")
            .map(|v| match self.resource(id(v)?)? {
                Resource::QuerySet(q) => Ok::<_, String>(q),
                _ => Err("Expected query set".into()),
            })
            .transpose()?;
        if descriptor.get("timestampWrites").is_some() {
            return Err("Pass timestampWrites is not implemented by this binding".into());
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: label(descriptor),
            color_attachments: &colors,
            depth_stencil_attachment: depth_attachment,
            timestamp_writes: None,
            occlusion_query_set: query.as_ref(),
            multiview_mask: None,
        });
        for c in list(commands)? {
            match operation(c)? {
                "setPipeline" => {
                    let Resource::RenderPipeline(p) = self.resource(u(c, 1)?)? else {
                        return Err("Expected render pipeline".into());
                    };
                    pass.set_pipeline(&p);
                }
                "setBindGroup" => {
                    let group = if c[2].is_null() {
                        None
                    } else {
                        let Resource::BindGroup(g) = self.resource(u(c, 2)?)? else {
                            return Err("Expected bind group".into());
                        };
                        Some(g)
                    };
                    let offsets: Vec<u32> = decode(c[3].clone())?;
                    pass.set_bind_group(u(c, 1)?, group.as_ref(), &offsets);
                }
                "setVertexBuffer" => {
                    let buffer = self.buffer(u(c, 2)?)?;
                    let start = n(c, 3)?;
                    let end = c[4]
                        .as_u64()
                        .map(|n| start.checked_add(n).ok_or("Vertex range overflow"))
                        .transpose()?
                        .unwrap_or(buffer.size());
                    pass.set_vertex_buffer(u(c, 1)?, buffer.slice(start..end));
                }
                "setIndexBuffer" => {
                    let buffer = self.buffer(u(c, 1)?)?;
                    let start = n(c, 3)?;
                    let end = c[4]
                        .as_u64()
                        .map(|n| start.checked_add(n).ok_or("Index range overflow"))
                        .transpose()?
                        .unwrap_or(buffer.size());
                    pass.set_index_buffer(buffer.slice(start..end), decode(c[2].clone())?);
                }
                "draw" => pass.draw(
                    u(c, 3)?
                        ..u(c, 3)?
                            .checked_add(u(c, 1)?)
                            .ok_or("Draw range overflow")?,
                    u(c, 4)?
                        ..u(c, 4)?
                            .checked_add(u(c, 2)?)
                            .ok_or("Instance range overflow")?,
                ),
                "drawIndexed" => pass.draw_indexed(
                    u(c, 3)?
                        ..u(c, 3)?
                            .checked_add(u(c, 1)?)
                            .ok_or("Index range overflow")?,
                    decode(c[4].clone())?,
                    u(c, 5)?
                        ..u(c, 5)?
                            .checked_add(u(c, 2)?)
                            .ok_or("Instance range overflow")?,
                ),
                "drawIndirect" => pass.draw_indirect(&self.buffer(u(c, 1)?)?, n(c, 2)?),
                "drawIndexedIndirect" => {
                    pass.draw_indexed_indirect(&self.buffer(u(c, 1)?)?, n(c, 2)?);
                }
                "setViewport" => {
                    pass.set_viewport(f(c, 1)?, f(c, 2)?, f(c, 3)?, f(c, 4)?, f(c, 5)?, f(c, 6)?);
                }
                "setScissorRect" => pass.set_scissor_rect(u(c, 1)?, u(c, 2)?, u(c, 3)?, u(c, 4)?),
                "setStencilReference" => pass.set_stencil_reference(u(c, 1)?),
                "setBlendConstant" => pass.set_blend_constant(decode(c[1].clone())?),
                "beginOcclusionQuery" => pass.begin_occlusion_query(u(c, 1)?),
                "endOcclusionQuery" => pass.end_occlusion_query(),
                "insertDebugMarker" => {
                    pass.insert_debug_marker(c[1].as_str().ok_or("Invalid marker")?);
                }
                "pushDebugGroup" => pass.push_debug_group(c[1].as_str().ok_or("Invalid group")?),
                "popDebugGroup" => pass.pop_debug_group(),
                op => return Err(format!("Unsupported render pass operation {op}")),
            }
        }
        Ok(())
    }

    fn compute_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        descriptor: &Value,
        commands: &Value,
    ) -> Result<()> {
        if descriptor.get("timestampWrites").is_some() {
            return Err("Pass timestampWrites is not implemented by this binding".into());
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: label(descriptor),
            timestamp_writes: None,
        });
        for c in list(commands)? {
            match operation(c)? {
                "setPipeline" => {
                    let Resource::ComputePipeline(p) = self.resource(u(c, 1)?)? else {
                        return Err("Expected compute pipeline".into());
                    };
                    pass.set_pipeline(&p);
                }
                "setBindGroup" => {
                    let Resource::BindGroup(g) = self.resource(u(c, 2)?)? else {
                        return Err("Expected bind group".into());
                    };
                    let offsets: Vec<u32> = decode(c[3].clone())?;
                    pass.set_bind_group(u(c, 1)?, &g, &offsets);
                }
                "dispatchWorkgroups" => pass.dispatch_workgroups(u(c, 1)?, u(c, 2)?, u(c, 3)?),
                "dispatchWorkgroupsIndirect" => {
                    pass.dispatch_workgroups_indirect(&self.buffer(u(c, 1)?)?, n(c, 2)?);
                }
                "insertDebugMarker" => {
                    pass.insert_debug_marker(c[1].as_str().ok_or("Invalid marker")?);
                }
                "pushDebugGroup" => pass.push_debug_group(c[1].as_str().ok_or("Invalid group")?),
                "popDebugGroup" => pass.pop_debug_group(),
                op => return Err(format!("Unsupported compute pass operation {op}")),
            }
        }
        Ok(())
    }
}
