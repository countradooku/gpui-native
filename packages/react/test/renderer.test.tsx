import { handleGpuiEvent } from "@gpui-native/runtime/events"
import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import {
  act,
  createRef,
  createContext,
  useContext,
  useEffect,
  useState,
  Suspense,
  StrictMode,
  Component,
  type ReactNode,
} from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

import {
  Combobox,
  ComboboxInput,
  ComboboxContent,
  ComboboxList,
  ComboboxItem,
  ComboboxEmpty,
} from "../src/combobox.js"
import {
  GpuiDiv,
  GpuiInput,
  GpuiCode,
  GpuiCanvas,
  GpuiMarkdown,
  GpuiVirtualList,
  MotionDiv,
} from "../src/components.js"
import {
  useGpuiRequired,
  useElementRef,
  useWindowSize,
  useGpuiTimeline,
  useGpuiAudioFrames,
} from "../src/context.js"
import { createGpuiRenderer } from "../src/renderer.js"
import type { GpuiPublicInstance } from "../src/renderer.js"
import { createGpuiRuntime } from "../src/runtime-core.js"
import { Select, SelectTrigger, SelectContent, SelectItem, SelectValue } from "../src/select.js"
import { mountGpui } from "../src/testing.js"
import { useTextSearch } from "../src/text-search.js"
import { Tooltip, TooltipTrigger, TooltipContent } from "../src/tooltip.js"
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true)
const roots: { unmount(): void }[] = []
function mount(node: ReactNode) {
  let root!: ReturnType<typeof mountGpui>
  act(() => {
    root = mountGpui(node)
  })
  roots.push(root)
  return root
}
afterEach(() => {
  act(() => roots.splice(0).forEach((root) => root.unmount()))
  vi.useRealTimers()
})

describe("React retained renderer", () => {
  it("updates state, context, refs and controlled inputs and cleans effects", () => {
    const Context = createContext("default")
    const cleanup = vi.fn<(...args: unknown[]) => void>()
    const ref = createRef<GpuiPublicInstance>()
    function App() {
      const [count, setCount] = useState(0)
      const [value, setValue] = useState("")
      const context = useContext(Context)
      useEffect(() => cleanup, [])
      return (
        <>
          <GpuiDiv ref={ref} testId="button" onClick={() => setCount((n) => n + 1)}>
            {context} {count}
          </GpuiDiv>
          <GpuiInput testId="input" value={value} onChange={(e) => setValue(e.value ?? "")} />
        </>
      )
    }
    const root = mount(
      <StrictMode>
        <Context value="count">
          <App />
        </Context>
      </StrictMode>,
    )
    expect(ref.current?.id).toBe(root.findByTestId("button").id)
    act(() => root.findByTestId("button").click())
    expect(root.findByTestId("button").text).toBe("count 1")
    act(() => root.findByTestId("input").trigger("change", { value: "hello" }))
    expect(root.findByTestId("input").prop("value")).toBe("hello")
    act(() => root.unmount())
    expect(ref.current).toBeNull()
    expect(cleanup).toHaveBeenCalled()
    expect(root.renderer.snapshot().length).toBe(0)
  })
  it("preserves keyed instances, removes props, replaces handlers and types", () => {
    const a = vi.fn<(...args: unknown[]) => void>(),
      b = vi.fn<(...args: unknown[]) => void>()
    const root = mount(
      <div testId="parent">
        {["a", "b", "c"].map((key) => (
          <input key={key} testId={key} value={key} onClick={a} style={{ width: 20 }} />
        ))}
      </div>,
    )
    const id = root.findByTestId("b").id
    act(() =>
      root.render(
        <div testId="parent">
          {["c", "b", "a"].map((key) => (
            <input key={key} testId={key} onClick={b} />
          ))}
        </div>,
      ),
    )
    expect(root.findByTestId("b").id).toBe(id)
    expect(
      root
        .findByTestId("parent")
        .node.children.map((node) => (node.kind === "element" ? node.props.testId : "")),
    ).toEqual(["c", "b", "a"])
    act(() => root.findByTestId("b").click())
    expect(b).toHaveBeenCalledOnce()
    expect(a).not.toHaveBeenCalled()
    expect(
      root.renderer.snapshot().find((node) => node.id === id)?.customProps.value,
    ).toBeUndefined()
    act(() => root.render(<text testId="replacement">new</text>))
    expect(handleGpuiEvent({ elementId: id, eventType: "click" }, root.renderer)).toBe(false)
  })
  it("batches each commit, leaves raw props intact and skips unchanged mutations", () => {
    const native = new MemoryNativeRenderer()
    const batch = vi.spyOn(native, "applyBatch")
    const host = createGpuiRenderer(native)
    roots.push({ unmount: host.destroy })
    const node = (
      <canvas commands={[{ type: "rect", x: 0, y: 0, width: 10, height: 10, fill: "#fff" }]} />
    )
    act(() => host.render(node))
    expect(batch).toHaveBeenCalledTimes(1)
    const operations = JSON.parse(batch.mock.calls[0]![0]) as unknown[][]
    expect(
      operations.find((op) => op[0] === "setCustomPropValue" && op[2] === "commands")?.[3],
    ).toBeInstanceOf(Array)
    act(() => host.render(node))
    expect(batch).toHaveBeenCalledTimes(1)
  })
  it("does not leak speculative nodes when suspense abandons a render", async () => {
    let ready = false
    let resolve!: () => void
    const pending = new Promise<void>((done) => {
      resolve = done
    })
    function Wait() {
      if (!ready) throw pending
      return <text>ready</text>
    }
    const root = mount(
      <Suspense fallback={<text>loading</text>}>
        <div testId="speculative">
          <text>before</text>
          <Wait />
        </div>
      </Suspense>,
    )
    expect(root.renderer.snapshotJson()).not.toContain("speculative")
    await act(async () => {
      ready = true
      resolve()
      await pending
    })
    expect(root.findByTestId("speculative").text).toBe("beforeready")
    expect(root.renderer.snapshotJson()).not.toContain("loading")
  })
  it("allows React error boundaries to recover", () => {
    class Boundary extends Component<{ children: ReactNode }, { failed: boolean }> {
      state = { failed: false }
      static getDerivedStateFromError() {
        return { failed: true }
      }
      render() {
        return this.state.failed ? <text>recovered</text> : this.props.children
      }
    }
    function Fail(): ReactNode {
      throw new Error("expected")
    }
    const root = mount(
      <Boundary>
        <Fail />
      </Boundary>,
    )
    expect(root.findByText("recovered").text).toBe("recovered")
  })
  it("isolates roots and rejects duplicate ownership", () => {
    const a = mount(<text>A</text>),
      b = mount(<text>B</text>)
    expect(() => createGpuiRenderer(a.renderer)).toThrow("one live root")
    act(() => a.unmount())
    expect(b.findByText("B").text).toBe("B")
    const host = createGpuiRenderer(a.renderer)
    act(() => host.destroy())
  })
  it("provides native hooks and searches through the shared highlight pipeline", () => {
    function App() {
      const renderer = useGpuiRequired()
      const ref = useElementRef()
      const size = useWindowSize({ intervalMs: false })
      const timeline = useGpuiTimeline()
      const audio = useGpuiAudioFrames()
      const search = useTextSearch({ query: "hello" })
      expect(renderer).toBeDefined()
      expect(timeline).toBeDefined()
      expect(audio).toBeDefined()
      return (
        <div ref={ref} testId="search" {...search.props} onClick={search.next}>
          {size.width}:{search.active}
        </div>
      )
    }
    const root = mount(<App />)
    act(() => root.findByTestId("search").trigger("highlight", { matchCount: 3 }))
    act(() => root.findByTestId("search").click())
    expect(root.findByTestId("search").prop("highlight")).toMatchObject({
      query: "hello",
      activeIndex: 1,
    })
  })
  it("opens and closes independent windows", () => {
    const runtime = createGpuiRuntime(() => new MemoryNativeRenderer())
    let a!: ReturnType<typeof runtime.createWindow>, b!: typeof a
    act(() => {
      a = runtime.createWindow(<text>A</text>, { renderer: new MemoryNativeRenderer() })
      b = runtime.createWindow(<text>B</text>, { renderer: new MemoryNativeRenderer() })
    })
    act(() => a.close())
    expect(b.host.root.children).toHaveLength(1)
    act(() => b.close())
  })
  it("supports every native primitive and motion props", () => {
    const root = mount(
      <>
        <GpuiCode code="const a = 1" />
        <GpuiCanvas commands={[]} />
        <GpuiMarkdown source="# Hi" />
        <GpuiVirtualList itemCount={3} estimatedItemHeight={20} />
        <MotionDiv animate={{ opacity: 1 }}>
          <text>motion</text>
        </MotionDiv>
      </>,
    )
    expect(root.host.root.children).toHaveLength(5)
  })
})
describe("React controls", () => {
  it("opens tooltips on focus and dismisses on escape", () => {
    const root = mount(
      <Tooltip>
        <TooltipTrigger testId="trigger">help</TooltipTrigger>
        <TooltipContent testId="tip">details</TooltipContent>
      </Tooltip>,
    )
    expect(root.queryByTestId("tip")).toBeNull()
    act(() => root.findByTestId("trigger").trigger("focus"))
    expect(root.findByTestId("tip").text).toBe("details")
    act(() => root.findByTestId("trigger").trigger("keyDown", { key: "escape" }))
    expect(root.queryByTestId("tip")).toBeNull()
  })
  it("selects values with keyboard navigation skipping disabled items", () => {
    const changed = vi.fn<(...args: unknown[]) => void>()
    const root = mount(
      <Select onValueChange={changed}>
        <SelectTrigger testId="trigger">
          <SelectValue testId="value" placeholder="Choose" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="a" disabled>
            A
          </SelectItem>
          <SelectItem value="b">Bee</SelectItem>
        </SelectContent>
      </Select>,
    )
    act(() => root.findByTestId("trigger").trigger("keyDown", { key: "down" }))
    act(() => root.findByTestId("trigger").trigger("keyDown", { key: "enter" }))
    expect(changed).toHaveBeenCalledWith("b")
    expect(root.findByTestId("value").text).toBe("Bee")
  })
  it("filters combobox options, shows empty state and selects values", () => {
    const changed = vi.fn<(...args: unknown[]) => void>()
    const root = mount(
      <Combobox items={["Apple", "Banana"]} autoHighlight onValueChange={changed}>
        <ComboboxInput testId="input" />
        <ComboboxContent>
          <ComboboxList>
            {(item) => (
              <ComboboxItem key={item} value={item}>
                {item}
              </ComboboxItem>
            )}
          </ComboboxList>
          <ComboboxEmpty testId="empty">Nothing</ComboboxEmpty>
        </ComboboxContent>
      </Combobox>,
    )
    act(() => root.findByTestId("input").trigger("change", { value: "ban" }))
    expect(root.findAll((e) => e.prop("role") === "option").map((e) => e.text)).toEqual(["Banana"])
    act(() => root.findByTestId("input").trigger("keyDown", { key: "enter" }))
    expect(changed).toHaveBeenCalledWith("Banana")
    act(() => root.findByTestId("input").trigger("change", { value: "zzz" }))
    expect(root.findByTestId("empty").text).toBe("Nothing")
  })
})
