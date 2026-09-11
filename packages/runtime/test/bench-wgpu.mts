import assert from "node:assert/strict"
import { writeFileSync } from "node:fs"
import { arch, cpus, platform, release } from "node:os"

import { createNativeRenderer } from "../src/native-addon.js"
import { createNativeGPU } from "../src/webgpu-native.js"
import { createGPUCanvas } from "../src/webgpu.js"

const backend = process.env.GPUI_BENCH_BACKEND ?? "wgpu"
const seconds = Number(process.env.GPUI_BENCH_SECONDS ?? 3)
if (backend === "dawn" && process.versions.bun)
  throw new Error("The Dawn baseline must run under Node")
const dawnBackend =
  process.env.GPUI_BENCH_DAWN_BACKEND ??
  (platform() === "darwin" ? "metal" : platform() === "win32" ? "d3d12" : "vulkan")
const gpu =
  backend === "dawn"
    ? (await import("webgpu")).create([`backend=${dawnBackend}`])
    : await createNativeGPU()
// Dawn requires its root GPU wrapper to outlive all scheduled native callbacks.
// Pin it for this benchmark process, even after its last ordinary JS use.
Reflect.set(globalThis, Symbol.for("gpui.benchmark.gpu"), gpu)
console.error(`Benchmark ${backend}: GPU created`)
const adapter = await gpu.requestAdapter()
assert(adapter)
console.error(`Benchmark ${backend}: adapter acquired`)
const renderer = createNativeRenderer()
renderer.init?.({ headless: true })
console.error(`Benchmark ${backend}: headless renderer initialized`)
const results: unknown[] = []
const percentile = (values: number[]) => {
  const sorted = [...values].sort((a, b) => a - b)
  const at = (q: number) =>
    Number(sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))]!.toFixed(3))
  return { p50: at(0.5), p95: at(0.95), p99: at(0.99) }
}
try {
  for (const presentation of backend === "dawn"
    ? (["async-readback"] as const)
    : (["async-readback", "direct"] as const)) {
    for (const [width, height] of [
      [640, 400],
      [1920, 1080],
      [3840, 2160],
    ] as const) {
      for (const count of [1, 4]) {
        console.error(`Benchmark ${backend}: ${presentation} ${width}x${height} canvases=${count}`)
        const caseAdapter = await gpu.requestAdapter()
        assert(caseAdapter)
        const device = await caseAdapter.requestDevice()
        const canvases = Array.from({ length: count }, () =>
          createGPUCanvas({ renderer, width, height, presentation, maxFramesInFlight: 2 }),
        )
        const encoding: number[] = [],
          submission: number[] = [],
          presentationTimes: number[] = [],
          frames: number[] = []
        let peakRss = process.memoryUsage().rss
        const beginCpu = process.cpuUsage()
        let started = 0,
          measured = 0
        try {
          for (const canvas of canvases)
            canvas.configure({ device, format: "bgra8unorm", alphaMode: "opaque" })
          for (let frame = 0; ; frame++) {
            if (frame === 30) started = performance.now()
            if (started && performance.now() - started >= seconds * 1000) break
            const begin = performance.now()
            const commands = canvases.map((canvas) => {
              const encoder = device.createCommandEncoder()
              const pass = encoder.beginRenderPass({
                colorAttachments: [
                  {
                    view: canvas.getContext("webgpu").getCurrentTexture().createView(),
                    clearValue: [(frame % 60) / 60, 0.4, 0.8, 1],
                    loadOp: "clear",
                    storeOp: "store",
                  },
                ],
              })
              pass.end()
              return encoder.finish()
            })
            const encoded = performance.now()
            device.queue.submit(commands)
            const submitted = performance.now()
            assert((await Promise.all(canvases.map((canvas) => canvas.present()))).every(Boolean))
            const done = performance.now()
            peakRss = Math.max(peakRss, process.memoryUsage().rss)
            if (frame >= 30) {
              measured++
              encoding.push(encoded - begin)
              submission.push(submitted - encoded)
              presentationTimes.push(done - submitted)
              frames.push(done - begin)
            }
          }
          results.push({
            presentation,
            width,
            height,
            canvases: count,
            measuredFrames: measured,
            measuredSeconds: (performance.now() - started) / 1000,
            ms: {
              sceneEncoding: percentile(encoding),
              submission: percentile(submission),
              synchronizationAndTransport: percentile(presentationTimes),
              total: percentile(frames),
            },
            cpuIncludingWarmup: process.cpuUsage(beginCpu),
            peakRssBytes: peakRss,
            cpuPixelBytes: canvases.reduce((sum, canvas) => sum + canvas.stats.bytesPresented, 0),
            gpuSceneTime: null,
            compositionTime: null,
          })
        } finally {
          for (const canvas of canvases) canvas.destroy()
          device.destroy()
        }
      }
    }
  }
  const report = {
    timestamp: new Date().toISOString(),
    backend,
    host: {
      platform: platform(),
      osRelease: release(),
      arch: arch(),
      cpu: cpus()[0]?.model,
      node: process.version,
      bun: process.versions.bun ?? null,
      adapter: {
        vendor: adapter.info.vendor,
        architecture: adapter.info.architecture,
        device: adapter.info.device,
        description: adapter.info.description,
        backend: backend === "dawn" ? dawnBackend : Reflect.get(adapter.info, "backend"),
        isFallbackAdapter: adapter.info.isFallbackAdapter,
      },
    },
    methodology:
      "Changing RGBA clear each frame; one queue submission for all canvases; 30 warmup frames, sustained wall-clock sample per case; 2 requested in-flight slots; await all presentations. Headless transport benchmark. CPU times include encoding/validation. Presentation includes scene completion, polling, copy/readback, and publication. GPU timestamps, atlas upload, window composition, screenshot capture, vsync and display latency are NOT measured. RSS includes unified-memory effects and runtime GC; it is not isolated GPU memory.",
    results,
  }
  const output = JSON.stringify(report, null, 2)
  if (process.env.GPUI_BENCH_OUTPUT) writeFileSync(process.env.GPUI_BENCH_OUTPUT, output + "\n")
  console.log(output)
} finally {
  renderer.close?.()
}
