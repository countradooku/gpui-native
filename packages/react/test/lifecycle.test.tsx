import { snapshotRenderer } from "@gpui-native/runtime/automation"
import { handleGpuiEvent } from "@gpui-native/runtime/events"
import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import { reportRuntimeError } from "@gpui-native/runtime/runtime-errors"
import { act, Activity, createRef, useEffect, type ReactNode } from "react"
import { afterEach, expect, it, vi } from "vitest"

import {
  Combobox,
  ComboboxInput,
  ComboboxContent,
  ComboboxList,
  ComboboxItem,
} from "../src/combobox.js"
import { useWindowSize, useWindowInsets } from "../src/context.js"
import { createGpuiRenderer } from "../src/renderer.js"
import type { GpuiPublicInstance } from "../src/renderer.js"
import { createGpuiRuntime } from "../src/runtime-core.js"
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from "../src/select.js"
import { mountGpui } from "../src/testing.js"
import { Tooltip, TooltipTrigger, TooltipContent } from "../src/tooltip.js"
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true)
const cleanup: (() => void)[] = []
function mount(node: ReactNode) {
  let root!: ReturnType<typeof mountGpui>
  act(() => {
    root = mountGpui(node)
  })
  cleanup.push(root.unmount)
  return root
}
afterEach(() => {
  act(() => cleanup.splice(0).forEach((stop) => stop()))
  vi.useRealTimers()
})
it("coalesces interpolated text and splits it again around host elements", () => {
  const root = mount(<text testId="label">Count {1}!</text>)
  const label = root.renderer.snapshot().find((node) => node.id === root.findByTestId("label").id)!
  expect(label.children).toHaveLength(1)
  expect(root.renderer.snapshot().find((node) => node.id === label.children[0])?.text).toBe(
    "Count 1!",
  )
  act(() =>
    root.render(
      <text testId="label">
        Count <text>2</text>!
      </text>,
    ),
  )
  expect(root.renderer.snapshot().find((node) => node.id === label.id)?.children).toHaveLength(3)
  act(() => root.render(<text testId="label">Count {3}!</text>))
  expect(root.renderer.snapshot().find((node) => node.id === label.id)?.children).toHaveLength(1)
  expect(root.findByTestId("label").text).toBe("Count 3!")
})
it("preserves native state while Activity hides and restores its subtree", async () => {
  const cleanupEffect = vi.fn<() => void>()
  function Child() {
    useEffect(() => cleanupEffect, [])
    return (
      <div testId="child" style={{ opacity: 0.5 }}>
        retained
      </div>
    )
  }
  const root = mount(
    <Activity mode="visible">
      <Child />
    </Activity>,
  )
  const id = root.findByTestId("child").id
  await act(async () =>
    root.render(
      <Activity mode="hidden">
        <Child />
      </Activity>,
    ),
  )
  expect(root.renderer.snapshot().find((node) => node.id === id)?.style.display).toBe("none")
  expect(cleanupEffect).toHaveBeenCalledOnce()
  await act(async () =>
    root.render(
      <Activity mode="visible">
        <Child />
      </Activity>,
    ),
  )
  expect(root.findByTestId("child").id).toBe(id)
  expect(root.renderer.snapshot().find((node) => node.id === id)?.style).toEqual({ opacity: 0.5 })
})
it("supports same-root portals without losing refs or portal children", () => {
  const target = createRef<GpuiPublicInstance>()
  const root = mount(<div ref={target} testId="target" />)
  act(() =>
    root.render(
      <>
        <div ref={target} testId="target" />
        {root.host.createPortal(<text testId="portal">portal</text>, target.current!)}
      </>,
    ),
  )
  expect(root.findByTestId("target").text).toBe("portal")
  act(() => root.render(<div ref={target} testId="target" />))
  expect(root.queryByTestId("portal")).toBeNull()
})
it("shares automation snapshots with the native mutation tree", () => {
  const root = mount(
    <div testId="button" onClick={() => {}}>
      Hello {"React"}
    </div>,
  )
  const snapshot = snapshotRenderer(root.renderer)
  expect(snapshot.nodes.get(root.findByTestId("button").id)?.customProps.testId).toBe("button")
  expect([...snapshot.nodes.values()].some((node) => node.text === "Hello React")).toBe(true)
})
it("delivers window events to multiple hooks and unsubscribes each on cleanup", () => {
  class WindowRenderer extends MemoryNativeRenderer {
    supportsWindowEvents() {
      return true
    }
  }
  const native = new WindowRenderer()
  const size = vi.spyOn(native, "getWindowSize").mockReturnValue({ width: 800, height: 600 })
  const host = createGpuiRenderer(native)
  cleanup.push(host.destroy)
  function App() {
    const a = useWindowSize(),
      b = useWindowSize(),
      insets = useWindowInsets()
    return (
      <text>
        {a.width}:{b.width}:{insets.visibleHeight}
      </text>
    )
  }
  act(() => host.render(<App />))
  size.mockReturnValue({ width: 900, height: 700 })
  act(() => {
    handleGpuiEvent({ elementId: 0, eventType: "windowResize" }, native)
  })
  expect(native.snapshot().some((node) => node.text === "900:900:700")).toBe(true)
  act(() => host.destroy())
  expect(handleGpuiEvent({ elementId: 0, eventType: "windowResize" }, native)).toBe(false)
})
it("clears stale window handlers on rerender and unmount", () => {
  class WindowRenderer extends MemoryNativeRenderer {
    eventId = 0
    setWindowKeyEvents(_down: boolean, _up: boolean, id: number) {
      this.eventId = id
    }
  }
  const native = new WindowRenderer(),
    a = vi.fn<() => void>(),
    b = vi.fn<() => void>()
  const runtime = createGpuiRuntime(() => native)
  cleanup.push(runtime.resetRender)
  let root!: ReturnType<typeof runtime.render>
  act(() => {
    root = runtime.render(<text>first</text>, { renderer: native, onKeyDown: a })
  })
  const first = native.eventId
  act(() => {
    runtime.render(<text>second</text>, { renderer: native, onKeyDown: b })
  })
  expect(handleGpuiEvent({ elementId: first, eventType: "windowKeyDown" }, native)).toBe(false)
  handleGpuiEvent({ elementId: native.eventId, eventType: "windowKeyDown" }, native)
  expect(a).not.toHaveBeenCalled()
  expect(b).toHaveBeenCalledOnce()
  act(() => root.unmount())
  expect(handleGpuiEvent({ elementId: native.eventId, eventType: "windowKeyDown" }, native)).toBe(
    false,
  )
})
it("shows runtime failures inside the owning window", () => {
  const native = new MemoryNativeRenderer()
  const runtime = createGpuiRuntime(() => native)
  cleanup.push(runtime.resetRender)
  act(() => runtime.render(<text>App</text>, { renderer: native }))
  const logged = vi.spyOn(console, "error").mockImplementation(() => {})
  try {
    act(() => reportRuntimeError(native, new Error("render failure")))
  } finally {
    logged.mockRestore()
  }
  expect(native.snapshot().some((node) => node.text?.includes("render failure"))).toBe(true)
})
it("cancels delayed tooltip opening on unmount", () => {
  vi.useFakeTimers()
  const opened = vi.fn<(open: boolean) => void>()
  const root = mount(
    <Tooltip delayDuration={300} onOpenChange={opened}>
      <TooltipTrigger testId="trigger">help</TooltipTrigger>
      <TooltipContent>tip</TooltipContent>
    </Tooltip>,
  )
  act(() => root.findByTestId("trigger").trigger("mouseEnter"))
  act(() => root.unmount())
  act(() => vi.advanceTimersByTime(400))
  expect(opened).not.toHaveBeenCalled()
})
it("composes asChild callbacks and refs", () => {
  const ref = createRef<GpuiPublicInstance>(),
    called = vi.fn<() => void>()
  const root = mount(
    <Tooltip>
      <TooltipTrigger asChild>
        <div ref={ref} testId="trigger" onFocus={called}>
          help
        </div>
      </TooltipTrigger>
      <TooltipContent testId="tip">tip</TooltipContent>
    </Tooltip>,
  )
  expect(ref.current?.id).toBe(root.findByTestId("trigger").id)
  act(() => root.findByTestId("trigger").trigger("focus"))
  expect(called).toHaveBeenCalledOnce()
  expect(root.findByTestId("tip").text).toBe("tip")
})
it("keeps controlled selection unchanged until the owner updates it", () => {
  const change = vi.fn<(value: string) => void>()
  const app = (value: string) => (
    <Select value={value} onValueChange={change}>
      <SelectTrigger testId="trigger">
        <SelectValue testId="value" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="a" testId="a">
          A
        </SelectItem>
        <SelectItem value="b" testId="b">
          B
        </SelectItem>
      </SelectContent>
    </Select>
  )
  const root = mount(app("a"))
  act(() => root.findByTestId("b").click())
  expect(change).toHaveBeenCalledWith("b")
  expect(root.findByTestId("value").text).toBe("A")
  act(() => root.render(app("b")))
  expect(root.findByTestId("value").text).toBe("B")
})
it("toggles multiple combobox values and ignores disabled items", () => {
  const change = vi.fn<(value: string | string[] | null) => void>()
  const root = mount(
    <Combobox multiple items={["a", "b"]} onValueChange={change}>
      <ComboboxInput />
      <ComboboxContent>
        <ComboboxList>
          {(item) => <ComboboxItem key={item} value={item} testId={item} disabled={item === "b"} />}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>,
  )
  act(() => root.findByTestId("a").click())
  expect(change).toHaveBeenLastCalledWith(["a"])
  act(() => root.findByTestId("a").click())
  expect(change).toHaveBeenLastCalledWith([])
  act(() => root.findByTestId("b").click())
  expect(change).toHaveBeenCalledTimes(2)
})
