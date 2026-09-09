import assert from "node:assert/strict"
import { arch, cpus, platform } from "node:os"

import { createNativeRenderer } from "../src/native-addon.js"
import { createNativeGPU } from "../src/webgpu-native.js"
import { createGPUCanvas } from "../src/webgpu.js"

const gpu = await createNativeGPU()
const adapter = await gpu.requestAdapter()
assert(adapter, "No WebGPU adapter")
const device = await adapter.requestDevice()
const renderer = createNativeRenderer()
renderer.init?.({ headless: true })
const results = []
try {
  for (const [width, height] of [
    [640, 400],
    [1920, 1080],
    [3840, 2160],
  ] as const) {
    const canvas = createGPUCanvas({ renderer, width, height })
    try {
      canvas.configure({ device, format: "bgra8unorm", alphaMode: "opaque" })
      const encoder = device.createCommandEncoder()
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: canvas.getContext("webgpu").getCurrentTexture().createView(),
            loadOp: "clear",
            storeOp: "store",
            clearValue: [0.2, 0.4, 0.8, 1],
          },
        ],
      })
      pass.end()
      device.queue.submit([encoder.finish()])
      const times: number[] = []
      for (let frame = 0; frame < 70; frame++) {
        const start = performance.now()
        assert(await canvas.present())
        if (frame >= 10) times.push(performance.now() - start)
      }
      times.sort((a, b) => a - b)
      results.push({
        width,
        height,
        samples: times.length,
        p50Ms: +times[30]!.toFixed(3),
        p95Ms: +times[57]!.toFixed(3),
        frames: canvas.stats.presented,
        failed: canvas.stats.failed,
      })
    } finally {
      canvas.destroy()
    }
  }
  console.log(
    JSON.stringify(
      {
        host: {
          platform: platform(),
          arch: arch(),
          cpu: cpus()[0]?.model,
          node: process.version,
          adapter: adapter.info.description,
        },
        scope:
          "GPU readback + native pixel conversion/storage; excludes scene rendering, atlas upload, window composition and display latency",
        results,
      },
      null,
      2,
    ),
  )
} finally {
  device.destroy()
  renderer.close?.()
}
