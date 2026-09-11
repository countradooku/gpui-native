import { defineComponent, h } from "@vue/runtime-core"
import { expect, it, vi } from "vitest"

import { MemoryNativeRenderer } from "../src/native.js"
import { createGpuiRenderer } from "../src/renderer.js"
import { useGPUCanvas } from "../src/webgpu.js"

it("publishes a Vue canvas source through the shared mutation protocol and disposes its scope", () => {
  const destroy = vi.fn<(id: number) => void>()
  const renderer = Object.assign(new MemoryNativeRenderer(), {
    createCanvasSource: () => 42,
    presentCanvasFrame: vi.fn<() => void>(),
    destroyCanvasSource: destroy,
  })
  const host = createGpuiRenderer(renderer)
  const app = host.mount(
    defineComponent({
      setup() {
        const canvas = useGPUCanvas({ presentation: "async-readback", width: 80, height: 40 })
        return () => h("canvas", { source: canvas.id })
      },
    }),
  )
  host.flushMutations()
  expect(
    [...renderer.nodes.values()].find((node) => node.type === "canvas")?.customProps.source,
  ).toBe(42)
  app.unmount()
  host.destroy()
  expect(destroy.mock.calls).toEqual([[42]])
})
