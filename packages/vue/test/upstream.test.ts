import { defineComponent, h, nextTick } from "@vue/runtime-core"
import { describe, expect, it, vi } from "vitest"

import { handleGpuiEvent } from "../src/events.js"
import { MemoryNativeRenderer } from "../src/native.js"
import { createGpuiRenderer } from "../src/renderer.js"
import { createGpuiRuntime, startFrameLoop } from "../src/runtime-core.js"
import { reportRuntimeError } from "../src/runtime-errors.js"
import type { EventPayload } from "../src/types.js"

const event = (elementId: number, eventType = "click"): EventPayload => ({ elementId, eventType })

describe("upstream behavior in the Vue adapter", () => {
  it("forwards and removes accessibility props and structured gradients", () => {
    const renderer = new MemoryNativeRenderer()
    const host = createGpuiRenderer(renderer)
    const gradient = {
      type: "linear-gradient",
      angle: 90,
      stops: [
        { color: "red", position: 0 },
        { color: "blue", position: 1 },
      ],
    }
    host.render(
      h("div", {
        role: "button",
        "aria-label": "Delete",
        "aria-selected": false,
        style: { background: gradient, textDecoration: "underline" },
      }),
    )
    const id = host.root.children[0]!.id
    expect(renderer.getCustomProp(id, "role")).toBe('"button"')
    expect(renderer.getCustomProp(id, "aria-selected")).toBe("false")
    expect(renderer.getTreeJson()).toContain("linear-gradient")
    host.render(h("div"))
    expect(renderer.getCustomProp(id, "role")).toBeNull()
    expect(renderer.getCustomProp(id, "aria-label")).toBeNull()
    host.destroy()
  })

  it("does not reuse node IDs or dispatch queued clicks into a replacement root", () => {
    const renderer = new MemoryNativeRenderer()
    const first = createGpuiRenderer(renderer)
    first.render(h("div", { onClick: () => undefined }))
    const oldId = first.root.children[0]!.id
    first.destroy()
    const click = vi.fn<(event: EventPayload) => void>()
    const second = createGpuiRenderer(renderer)
    second.render(h("div", { onClick: click }))
    const newId = second.root.children[0]!.id
    expect(newId).toBeGreaterThan(oldId)
    handleGpuiEvent(event(oldId), renderer)
    expect(click).not.toHaveBeenCalled()
    handleGpuiEvent(event(newId), renderer)
    expect(click).toHaveBeenCalledOnce()
    second.destroy()
  })

  it("continues pumping after a frame error", async () => {
    const renderer = new MemoryNativeRenderer()
    renderer.requiresTick = () => true
    let ticks = 0
    renderer.tick = () => {
      if (++ticks === 1) throw new Error("transient frame error")
      return false
    }
    const log = vi.spyOn(console, "error").mockImplementation(() => undefined)
    const loop = startFrameLoop(renderer, { frameMs: 1 })
    try {
      await new Promise((resolve) => setTimeout(resolve, 25))
      expect(ticks).toBe(2)
      expect(log).toHaveBeenCalledOnce()
    } finally {
      loop.stop()
      log.mockRestore()
    }
  })

  it("renders an error stack in the owning window", async () => {
    const renderer = new MemoryNativeRenderer()
    const runtime = createGpuiRuntime(() => renderer, "__gpuiUpstreamErrorTest")
    const root = runtime.render(defineComponent({ setup: () => () => h("text", "Working") }), {
      renderer,
    })
    const log = vi.spyOn(console, "error").mockImplementation(() => undefined)
    try {
      reportRuntimeError(renderer, new Error("handler exploded"))
      await nextTick()
      root.host.flushMutations()
      expect(renderer.getAllText().join("\n")).toContain("handler exploded")
      expect(renderer.getAllText()).toContain("Application runtime error")
    } finally {
      runtime.resetRender()
      log.mockRestore()
    }
  })

  it("rejects queued window key events after remount and disables them on unmount", () => {
    class KeyRenderer extends MemoryNativeRenderer {
      keyEvents: [boolean, boolean, number] = [false, false, 0]
      setWindowKeyEvents(down: boolean, up: boolean, id: number): void {
        this.keyEvents = [down, up, id]
      }
    }
    const renderer = new KeyRenderer()
    const runtime = createGpuiRuntime(() => renderer, "__gpuiUpstreamKeyTest")
    const app = defineComponent({ setup: () => () => h("text", "Keys") })
    const first = vi.fn<(event: EventPayload) => void>()
    const second = vi.fn<(event: EventPayload) => void>()
    try {
      runtime.render(app, { renderer, onKeyDown: first })
      const oldId = renderer.keyEvents[2]
      const root = runtime.render(app, { renderer, onKeyDown: second })
      handleGpuiEvent(event(oldId, "windowKeyDown"), renderer)
      expect(first).not.toHaveBeenCalled()
      expect(second).not.toHaveBeenCalled()
      handleGpuiEvent(event(renderer.keyEvents[2], "windowKeyDown"), renderer)
      expect(second).toHaveBeenCalledOnce()
      root.unmount()
      expect(renderer.keyEvents.slice(0, 2)).toEqual([false, false])
    } finally {
      runtime.resetRender()
    }
  })
})
