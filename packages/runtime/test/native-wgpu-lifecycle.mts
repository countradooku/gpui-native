import assert from "node:assert/strict"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { PNG } from "pngjs"

import { createNativeRenderer } from "../src/native-addon.js"
import { TestRenderer, hasNativeTestRenderer } from "../src/native-testing.js"
import { createNativeGPU } from "../src/webgpu-native.js"
import { createGPUCanvas } from "../src/webgpu.js"
import { nativeDevice } from "../src/wgpu-native-objects.js"

const gpu = await createNativeGPU()
const adapter = await gpu.requestAdapter()
assert(adapter)
const device = await adapter.requestDevice()
const native = Reflect.get(device, nativeDevice) as import("@gpui-native/core").NativeWgpuDevice
const validationErrors: string[] = []
device.addEventListener("uncapturederror", (event) =>
  validationErrors.push((event as GPUUncapturedErrorEvent).error.message),
)
const computeDescriptor = {
  layout: "auto" as const,
  compute: {
    module: device.createShaderModule({ code: "@compute @workgroup_size(1) fn main() {}" }),
  },
}

// Real async pipelines on both runtimes, including worker failure and teardown.
await Promise.all(
  Array.from({ length: 16 }, () => device.createComputePipelineAsync(computeDescriptor)),
)
await assert.rejects(
  device.createComputePipelineAsync({
    ...computeDescriptor,
    compute: { ...computeDescriptor.compute, entryPoint: "missing" },
  }),
  /entry|missing/i,
)
const beforeInvalid = native.resourceCount
device.pushErrorScope("validation")
device.createBuffer({ size: 16, usage: 0 })
assert(await device.popErrorScope(), "Invalid usage must be captured")
assert.equal(native.resourceCount, beforeInvalid, "Failed creations must not leak handles")
await assert.rejects(device.popErrorScope(), /scope/i)
assert.throws(
  () => device.createRenderBundleEncoder({ colorFormats: ["rgba8unorm"] }),
  /not supported/i,
)
assert.throws(() => native.create("unsupported", {}), /Unsupported/)
assert.throws(() => native.create("view", { texture: 0xffffffff }), /handle/)

const mapped = device.createBuffer({ size: 16, usage: 4 | 8, mappedAtCreation: true })
const range = mapped.getMappedRange()
new Uint32Array(range).set([1, 2, 3, 4])
assert.throws(() => mapped.getMappedRange(0, 8), /overlap/)
mapped.unmap()
assert.equal(range.byteLength, 0, "Unmap detaches all mapped ranges")
mapped.destroy()
mapped.destroy()

await assert.rejects(adapter.requestDevice(), /consumed/)
const otherAdapter = await gpu.requestAdapter()
assert(otherAdapter)
const other = await otherAdapter.requestDevice()
device.pushErrorScope("validation")
const foreign = other.createBuffer({ size: 4, usage: 8 })
device.queue.writeBuffer(foreign, 0, new Uint8Array(4))
assert(await device.popErrorScope(), "Foreign device resource must be rejected")
other.destroy()
foreign.destroy()

if (hasNativeTestRenderer && process.platform === "darwin") {
  const renderer = new TestRenderer({ width: 96, height: 64 })
  const directory = mkdtempSync(join(tmpdir(), "gpui-direct-lifecycle-"))
  const a = createGPUCanvas({ renderer, width: 32, height: 32 })
  const b = createGPUCanvas({ renderer, width: 32, height: 32 })
  const unrelated = createNativeRenderer()
  const foreignId = unrelated.createCanvasSource!()
  assert.notEqual(foreignId, a.id, "Renderer source IDs cannot alias between windows")
  await assert.rejects(unrelated.presentCanvasTexture!(a.id, native, 0, true), /source/)
  unrelated.destroyCanvasSource!(foreignId)
  unrelated.close?.()
  try {
    a.configure({ device, format: "rgba8unorm", alphaMode: "premultiplied" })
    b.configure({ device, format: "rgba8unorm" })
    renderer.applyBatch(
      JSON.stringify([
        ["createElement", 1, "div"],
        [
          "setStyle",
          1,
          {
            width: 96,
            height: 64,
            backgroundColor: "#0000ff",
            display: "flex",
            flexDirection: "row",
            gap: 8,
          },
        ],
        ["createElement", 2, "canvas"],
        ["setStyle", 2, { width: 32, height: 32 }],
        ["setCustomProp", 2, "source", a.id],
        ["appendChild", 1, 2],
        ["createElement", 3, "canvas"],
        ["setStyle", 3, { width: 32, height: 32 }],
        ["setCustomProp", 3, "source", b.id],
        ["appendChild", 1, 3],
        ["setRoot", 1],
      ]),
    )
    const fill = (canvas: typeof a, color: GPUColor) => {
      const encoder = device.createCommandEncoder()
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: canvas.getContext("webgpu").getCurrentTexture().createView(),
            clearValue: color,
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      })
      pass.end()
      device.queue.submit([encoder.finish()])
    }
    fill(a, [0.5, 0, 0, 0.5])
    fill(b, [0, 1, 0, 0]) // Opaque mode must ignore the producer's zero alpha.
    assert.deepEqual(await Promise.all([a.present(), b.present()]), [true, true])
    const screenshot = () => {
      const path = join(directory, "canvases.png")
      renderer.captureScreenshot(path)
      return PNG.sync.read(readFileSync(path))
    }
    const pixel = (png: PNG, x: number, y: number) => {
      const offset =
        (Math.floor((y * png.height) / 64) * png.width + Math.floor((x * png.width) / 96)) * 4
      return [...png.data.subarray(offset, offset + 4)]
    }
    let png = screenshot()
    assert.deepEqual(pixel(png, 16, 16), [128, 0, 127, 255])
    assert.deepEqual(pixel(png, 56, 16), [0, 255, 0, 255])
    assert.deepEqual(
      pixel(png, 36, 16),
      [0, 0, 255, 255],
      "Canvas pixels must not escape their bounds",
    )
    for (let frame = 0; frame < 40; frame++) {
      fill(b, frame % 2 ? [0, 1, 0, 1] : [1, 0, 0, 1])
      assert(await b.present(), "Completed compositor leases must return to the pool")
      screenshot()
    }
    const pending = a.present()
    a.resize(48, 24)
    assert.equal(await pending, false, "Resize invalidates an outstanding publication")
    fill(a, [1, 1, 0, 1])
    assert(await a.present())
    a.destroy()
    a.destroy()
    png = screenshot()
    assert.deepEqual(pixel(png, 16, 16), [0, 0, 255, 255])
    assert.equal(a.stats.bytesPresented, 0)
    assert.equal(b.stats.bytesPresented, 0)
    const busy = b.present()
    device.destroy()
    assert.equal(await busy.catch(() => false), false)
    await device.lost
    const recoveryAdapter = await gpu.requestAdapter()
    assert(recoveryAdapter)
    const recovered = await recoveryAdapter.requestDevice()
    b.configure({ device: recovered, format: "rgba8unorm" })
    const encoder = recovered.createCommandEncoder()
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: b.getContext("webgpu").getCurrentTexture().createView(),
          clearValue: [1, 0, 1, 1],
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    })
    pass.end()
    recovered.queue.submit([encoder.finish()])
    assert(await b.present())
    assert.deepEqual(pixel(screenshot(), 56, 16), [255, 0, 255, 255])
    b.destroy()
    screenshot() // Retire the compositor's final references to both sources.
    recovered.destroy()
    for (let attempt = 0; native.canvasSnapshotBytes && attempt < 100; attempt++)
      await new Promise((resolve) => setTimeout(resolve, 10))
    assert.equal(
      native.canvasSnapshotBytes,
      0,
      "All producer/compositor allocation leases must retire",
    )
  } finally {
    a.destroy()
    b.destroy()
    renderer.close?.()
    rmSync(directory, { recursive: true, force: true })
  }
}
device.destroy()
assert.deepEqual(validationErrors, [], "No unexpected uncaptured GPU errors")
assert.equal(native.resourceCount, 0, "Device destruction releases all registry handles")
console.log("wgpu lifecycle passed", {
  runtime: process.versions.bun ? `Bun ${process.versions.bun}` : process.version,
  adapter: adapter.info,
  validationErrors,
})
