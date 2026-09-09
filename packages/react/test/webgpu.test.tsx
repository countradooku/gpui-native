import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import { act, StrictMode } from "react"
import { expect, it, vi } from "vitest"

import { createGpuiRenderer } from "../src/renderer.js"
import { useGPUCanvas } from "../src/webgpu.js"
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true)

it("owns GPU canvas sources in committed React effects and cleans StrictMode replay", () => {
  let next = 0
  const destroyed = vi.fn<(id: number) => void>()
  const renderer = Object.assign(new MemoryNativeRenderer(), {
    createCanvasSource: () => ++next,
    presentCanvasFrame: vi.fn<() => void>(),
    destroyCanvasSource: destroyed,
  })
  const host = createGpuiRenderer(renderer)
  function App({ width }: { width: number }) {
    const canvas = useGPUCanvas({ width, height: 20, presentation: "async-readback" })
    return canvas && <canvas source={canvas.id} style={{ width, height: 20 }} />
  }
  try {
    act(() =>
      host.render(
        <StrictMode>
          <App width={40} />
        </StrictMode>,
      ),
    )
    host.flush()
    expect(
      [...renderer.nodes.values()].find((node) => node.type === "canvas")?.customProps.source,
    ).toBe(next)
    const allocated = next
    act(() =>
      host.render(
        <StrictMode>
          <App width={80} />
        </StrictMode>,
      ),
    )
    expect(next).toBe(allocated)
  } finally {
    act(() => host.destroy())
  }
  expect(destroyed.mock.calls.map(([id]) => id).sort()).toEqual(
    Array.from({ length: next }, (_, i) => i + 1),
  )
})
