import assert from "node:assert/strict"
import { readFileSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { PNG } from "pngjs"
import {
  WebGPURenderer,
  Scene,
  PerspectiveCamera,
  Mesh,
  BoxGeometry,
  MeshBasicMaterial,
  DataTexture,
  RGBAFormat,
} from "three/webgpu"

import { createNativeRenderer } from "../src/native-addon.js"
import { TestRenderer, hasNativeTestRenderer } from "../src/native-testing.js"
import { createNativeGPU, installWebGPU } from "../src/webgpu-native.js"
import { createGPUCanvas } from "../src/webgpu.js"

const gpu = await createNativeGPU()
const adapter = await gpu.requestAdapter()
assert(adapter, "WebGPU requires an available adapter")
const device = await adapter.requestDevice()
device.pushErrorScope("validation")
const renderer = hasNativeTestRenderer
  ? new TestRenderer({ width: 80, height: 40 })
  : createNativeRenderer()
if (!hasNativeTestRenderer) renderer.init?.({ headless: true })
const canvas = createGPUCanvas({
  presentation: process.env.GPUI_GPU_PRESENTATION === "direct" ? "direct" : "async-readback",
  renderer,
  width: 65,
  height: 16,
})
const directory = mkdtempSync(join(tmpdir(), "gpui-webgpu-"))
const resources: { destroy(): void }[] = []
function screenshotPixel(name: string) {
  assert(renderer instanceof TestRenderer)
  const path = join(directory, `${name}.png`)
  renderer.captureScreenshot(path)
  const png = PNG.sync.read(readFileSync(path))
  const offset =
    (Math.floor((png.height / 40) * 8) * png.width + Math.floor((png.width / 80) * 32)) * 4
  return [...png.data.subarray(offset, offset + 4)]
}
try {
  // Real compute, upload, and readback use the backend's WebGPU implementation.
  const storage = device.createBuffer({ size: 16, usage: 128 | 4 | 8 })
  resources.push(storage)
  const readback = device.createBuffer({ size: 16, usage: 1 | 8 })
  resources.push(readback)
  device.queue.writeBuffer(storage, 0, new Uint32Array([1, 2, 3, 4]))
  const compute = await device.createComputePipelineAsync({
    layout: "auto",
    compute: {
      module: device.createShaderModule({
        code: "@group(0) @binding(0) var<storage, read_write> values: array<u32>; @compute @workgroup_size(4) fn main(@builtin(local_invocation_index) i: u32) { values[i] *= 3u; }",
      }),
      entryPoint: "main",
    },
  })
  const computeCommands = device.createCommandEncoder()
  const computePass = computeCommands.beginComputePass()
  computePass.setPipeline(compute)
  computePass.setBindGroup(
    0,
    device.createBindGroup({
      layout: compute.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: storage } }],
    }),
  )
  computePass.dispatchWorkgroups(1)
  computePass.end()
  computeCommands.copyBufferToBuffer(storage, 0, readback, 0, 16)
  device.queue.submit([computeCommands.finish()])
  await readback.mapAsync(1)
  assert.deepEqual([...new Uint32Array(readback.getMappedRange())], [3, 6, 9, 12])
  readback.unmap()

  canvas.configure({ device, format: "rgba8unorm" })
  renderer.applyBatch!(
    JSON.stringify([
      ["createElement", 1, "div"],
      ["setStyle", 1, { width: 80, height: 40, backgroundColor: "#0000ff" }],
      ["createElement", 2, "canvas"],
      ["setStyle", 2, { width: 65, height: 16 }],
      ["setCustomProp", 2, "source", canvas.id],
      ["appendChild", 1, 2],
      ["setRoot", 1],
    ]),
  )
  const cube = device.createTexture({ size: [1, 1, 6], format: "rgba8unorm", usage: 4 | 2 })
  resources.push(cube)
  for (let layer = 0; layer < 6; layer++)
    device.queue.writeTexture(
      { texture: cube, origin: [0, 0, layer] },
      new Uint8Array([255, 0, 0, 255]),
      { bytesPerRow: 4, rowsPerImage: 1 },
      [1, 1, 1],
    )
  const shader = device.createShaderModule({
    code: `
    @group(0) @binding(0) var cube: texture_cube<f32>;
    @group(0) @binding(1) var sample: sampler;
    @vertex fn vertex(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
      let p = array(vec2f(-1,-1), vec2f(3,-1), vec2f(-1,3)); return vec4f(p[i],0,1);
    }
    @fragment fn fragment() -> @location(0) vec4f { return textureSampleLevel(cube,sample,vec3f(1,0,0),0); }
  `,
  })
  const pipeline = await device.createRenderPipelineAsync({
    layout: "auto",
    vertex: { module: shader, entryPoint: "vertex" },
    fragment: { module: shader, entryPoint: "fragment", targets: [{ format: "rgba8unorm" }] },
    multisample: { count: 4 },
  })
  const msaa = device.createTexture({
    size: [65, 16],
    sampleCount: 4,
    format: "rgba8unorm",
    usage: 16,
  })
  resources.push(msaa)
  const commands = device.createCommandEncoder()
  const pass = commands.beginRenderPass({
    colorAttachments: [
      {
        view: msaa.createView(),
        resolveTarget: canvas.getContext("webgpu").getCurrentTexture().createView(),
        loadOp: "clear",
        storeOp: "discard",
        clearValue: [0, 0, 0, 1],
      },
    ],
  })
  pass.setPipeline(pipeline)
  pass.setBindGroup(
    0,
    device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: cube.createView({ dimension: "cube" }) },
        { binding: 1, resource: device.createSampler() },
      ],
    }),
  )
  pass.draw(3)
  pass.end()
  device.queue.submit([commands.finish()])
  assert(await canvas.present())
  if (renderer instanceof TestRenderer)
    assert.deepEqual(screenshotPixel("cube-msaa"), [255, 0, 0, 255])

  for (const format of ["rgba8unorm", "bgra8unorm"] as const) {
    canvas.configure({ device, format, alphaMode: "premultiplied" })
    const encoder = device.createCommandEncoder()
    const clear = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: canvas.getContext("webgpu").getCurrentTexture().createView(),
          loadOp: "clear",
          storeOp: "store",
          clearValue: [0.5, 0, 0, 0.5],
        },
      ],
    })
    clear.end()
    device.queue.submit([encoder.finish()])
    assert(await canvas.present())
    if (renderer instanceof TestRenderer) {
      const [r, g, b, a] = screenshotPixel(format)
      assert(
        Math.abs(r! - 128) <= 1 && g === 0 && Math.abs(b! - 127) <= 1 && a === 255,
        `premultiplied ${format}: ${r},${g},${b},${a}`,
      )
    }
  }
  const restoreGlobals = await installWebGPU()
  const three = new WebGPURenderer({
    canvas: canvas as unknown as HTMLCanvasElement,
    antialias: true,
  })
  const geometry = new BoxGeometry()
  const texture = new DataTexture(new Uint8Array([0, 255, 0, 255]), 1, 1, RGBAFormat)
  texture.needsUpdate = true
  const material = new MeshBasicMaterial({ map: texture })
  try {
    three.setSize(65, 16, false)
    const scene = new Scene()
    scene.add(new Mesh(geometry, material))
    const camera = new PerspectiveCamera(45, 65 / 16, 0.1, 10)
    camera.position.z = 2
    await three.init()
    three.render(scene, camera)
    assert(await canvas.present())
    if (renderer instanceof TestRenderer)
      assert.deepEqual(screenshotPixel("three"), [0, 255, 0, 255])
  } finally {
    geometry.dispose()
    material.dispose()
    texture.dispose()
    three.dispose()
    restoreGlobals()
  }
  canvas.destroy()
  if (renderer instanceof TestRenderer)
    assert.deepEqual(screenshotPixel("destroyed"), [0, 0, 255, 255])
  const validation = await device.popErrorScope()
  assert.equal(validation, null, validation?.message)
  console.log(
    "WebGPU native smoke passed: compute, cube uploads/sampling, 4x MSAA, padded readback, RGBA/BGRA, alpha compositing, textured Three.js, destruction",
    canvas.stats,
  )
} finally {
  canvas.destroy()
  for (const resource of resources) resource.destroy()
  device.destroy()
  renderer.close?.()
  rmSync(directory, { recursive: true, force: true })
}
