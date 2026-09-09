/// <reference types="@webgpu/types" preserve="true" />
import type { GPUCanvas } from "@gpui-native/runtime/webgpu"

/** Real WGSL shaders and four-sample antialiasing, shared by both framework examples. */
export async function startWebGPUScene(
  canvas: GPUCanvas,
  gpu: GPU,
  onError: (error: unknown) => void,
): Promise<() => void> {
  const adapter = await gpu.requestAdapter({ powerPreference: "high-performance" })
  if (!adapter) throw new Error("No WebGPU adapter is available")
  const device = await adapter.requestDevice()
  if (canvas.destroyed) {
    device.destroy()
    return () => {}
  }
  let stopped = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let msaa: GPUTexture | undefined
  let uniforms: GPUBuffer | undefined
  const stop = () => {
    stopped = true
    clearTimeout(timer)
    msaa?.destroy()
    uniforms?.destroy()
    device.destroy()
  }
  try {
    canvas.configure({ device, format: "rgba8unorm", alphaMode: "opaque" })
    const shader = device.createShaderModule({
      code: `
      struct Uniforms { angle: f32 }
      @group(0) @binding(0) var<uniform> uniforms: Uniforms;
      struct Vertex { @builtin(position) position: vec4f, @location(0) color: vec3f }
      @vertex fn vertex(@builtin(vertex_index) i: u32) -> Vertex {
        let positions = array(vec2f(0, 0.75), vec2f(-0.7, -0.55), vec2f(0.7, -0.55));
        let colors = array(vec3f(0.3, 0.95, 1), vec3f(0.65, 0.3, 1), vec3f(1, 0.45, 0.25));
        let p = positions[i]; let c = cos(uniforms.angle); let s = sin(uniforms.angle);
        var out: Vertex;
        out.position = vec4f(p.x*c-p.y*s, p.x*s+p.y*c, 0, 1);
        out.color = colors[i]; return out;
      }
      @fragment fn fragment(in: Vertex) -> @location(0) vec4f { return vec4f(in.color, 1); }
    `,
    })
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: shader, entryPoint: "vertex" },
      fragment: { module: shader, entryPoint: "fragment", targets: [{ format: "rgba8unorm" }] },
      multisample: { count: 4 },
    })
    if (canvas.destroyed) {
      stop()
      return stop
    }
    uniforms = device.createBuffer({ size: 16, usage: 64 | 8 })
    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: uniforms } }],
    })
    msaa = device.createTexture({
      size: [canvas.width, canvas.height],
      sampleCount: 4,
      format: "rgba8unorm",
      usage: 16,
    })
    const samples = msaa.createView()
    const values = new Float32Array(4)
    const started = performance.now()
    const frame = async () => {
      if (stopped || canvas.destroyed) {
        stop()
        return
      }
      try {
        values[0] = (performance.now() - started) / 1800
        device.queue.writeBuffer(uniforms!, 0, values)
        const encoder = device.createCommandEncoder()
        const pass = encoder.beginRenderPass({
          colorAttachments: [
            {
              view: samples,
              resolveTarget: canvas.getContext("webgpu").getCurrentTexture().createView(),
              clearValue: { r: 0.025, g: 0.04, b: 0.09, a: 1 },
              loadOp: "clear",
              storeOp: "discard",
            },
          ],
        })
        pass.setPipeline(pipeline)
        pass.setBindGroup(0, bindGroup)
        pass.draw(3)
        pass.end()
        device.queue.submit([encoder.finish()])
        await canvas.present()
        if (!stopped)
          timer = setTimeout(() => {
            void frame()
          }, 16)
      } catch (error) {
        if (!stopped && !canvas.destroyed) onError(error)
        stop()
      }
    }
    void frame()
    return stop
  } catch (error) {
    stop()
    throw error
  }
}
