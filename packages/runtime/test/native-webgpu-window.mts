import assert from "node:assert/strict"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { PNG } from "pngjs"

import { startThreeScene } from "../../../examples/shared/three-scene.js"
import { startWebGPUScene } from "../../../examples/shared/webgpu-scene.js"
import { createNativeRenderer } from "../src/native-addon.js"
import { installWebGPU } from "../src/webgpu-native.js"
import { createGPUCanvas } from "../src/webgpu.js"

if (process.platform !== "darwin") {
  console.log("Production Metal window test skipped on this platform")
  process.exit(0)
}
const restore = await installWebGPU()
const renderer = createNativeRenderer()
renderer.init!({ title: "GPU window regression", width: 340, height: 480, focus: false })
const tick = setInterval(() => renderer.tick!(), 8)
const directory = mkdtempSync(join(tmpdir(), "gpui-live-window-"))
const canvases = [0, 1].map(() => createGPUCanvas({ renderer, width: 300, height: 180 }))
const errors: unknown[] = []
const stops: (() => void)[] = []
try {
  renderer.applyBatch!(
    JSON.stringify([
      ["createElement", 1, "div"],
      [
        "setStyle",
        1,
        {
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          backgroundColor: "#070b17",
          gap: 12,
        },
      ],
      ...canvases.flatMap((canvas, i) => [
        ["createElement", i + 2, "canvas"],
        ["setStyle", i + 2, { width: 300, height: 180 }],
        ["setCustomProp", i + 2, "source", canvas.id],
        ["appendChild", 1, i + 2],
      ]),
      ["setRoot", 1],
    ]),
  )
  // Unlike TestRenderer/headless transport, this exercises the actual AppKit
  // window and worker-to-main-thread repaint path used by desktop applications.
  stops.push(await startWebGPUScene(canvases[0]!, navigator.gpu, (error) => errors.push(error)))
  stops.push(await startThreeScene(canvases[1]!, navigator.gpu, (error) => errors.push(error)))
  const screenshot = () => {
    const path = join(directory, "window.png")
    renderer.captureScreenshot!(path)
    return PNG.sync.read(readFileSync(path))
  }
  const coloredPixels = (png: PNG, id: number) => {
    const bounds = renderer.getElementBounds!(id)!
    const size = renderer.getWindowSize!()
    const scale = png.width / size.width
    let count = 0
    for (
      let y = Math.ceil(bounds[1]! * scale);
      y < Math.min(png.height, (bounds[1]! + bounds[3]!) * scale);
      y++
    ) {
      for (
        let x = Math.ceil(bounds[0]! * scale);
        x < Math.min(png.width, (bounds[0]! + bounds[2]!) * scale);
        x++
      ) {
        const offset = (y * png.width + x) * 4
        if (Math.max(png.data[offset]!, png.data[offset + 1]!, png.data[offset + 2]!) > 100) count++
      }
    }
    return count
  }
  for (let frame = 0; frame < 100 && canvases.some((canvas) => canvas.stats.presented < 3); frame++)
    await new Promise((resolve) => setTimeout(resolve, 20))
  assert.deepEqual(
    errors,
    [],
    "Production-window presentations must not touch worker-thread AppKit state",
  )
  assert(
    canvases.every((canvas) => canvas.stats.presented >= 3),
    "Both scenes must sustain presentation",
  )
  const image = screenshot()
  assert(coloredPixels(image, 2) > 100, "Compute triangle must be visible")
  assert(coloredPixels(image, 3) > 100, "Textured Three.js cube must be visible")
  canvases[0]!.resize(220, 120)
  renderer.applyBatch!(JSON.stringify([["setStyle", 2, { width: 220, height: 120 }]]))
  await new Promise((resolve) => setTimeout(resolve, 100))
  assert(coloredPixels(screenshot(), 2) > 100, "Resized triangle must remain visible")
  assert.deepEqual(errors, [])
  console.log(
    "Production Metal window passed: compute, textured Three.js, asynchronous repaint and resize",
  )
} finally {
  for (const stop of stops) stop()
  for (const canvas of canvases) canvas.destroy()
  clearInterval(tick)
  renderer.close!()
  restore()
  rmSync(directory, { recursive: true, force: true })
}
