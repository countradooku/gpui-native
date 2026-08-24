import { defineComponent, h, onMounted, ref } from "@vue/runtime-core"
import { describe, expect, it } from "vitest"

import {
  MemoryNativeBridge,
  GpuiCanvas,
  GpuiCode,
  GpuiInput,
  Combobox,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  AUTOMATION_PROTOCOL_VERSION,
  automationText,
  connectTest,
  createAudioFrames,
  createGpuiRenderer,
  createTimeline,
  createWindow,
  decodeSseChunk,
  encodeSse,
  findAutomationNodeByTestId,
  mountGpui,
  MotionDiv,
  snapshotRenderer,
  startFrameLoop,
  useElementRef,
  useGpuiWindow,
  stagger,
  type NativeRenderer,
  type TestAutomationRenderer,
} from "../src/index.js"

describe("GPUI Vue renderer", () => {
  it("maps div and text nodes into the native retained tree", () => {
    const bridge = new MemoryNativeBridge()
    const host = createGpuiRenderer(bridge)

    host.render(
      h("div", { id: "app" }, ["hello", h("div", { class: "nested" }, "world")]),
      host.root,
    )

    if (bridge.rootId === null) throw new Error("Expected native root")
    const root = bridge.nodes.get(bridge.rootId)
    const outer = bridge.nodes.get(root?.children[0] ?? -1)
    const text = bridge.nodes.get(outer?.children[0] ?? -1)
    const nested = bridge.nodes.get(outer?.children[1] ?? -1)

    expect(root?.type).toBe("div")
    expect(outer).toMatchObject({ type: "div" })
    expect(outer?.customProps).toEqual({})
    expect(text).toMatchObject({ type: "text", text: "hello" })
    expect(nested).toMatchObject({
      type: "div",
    })
  })

  it("applies text updates and keyed reordering", () => {
    const bridge = new MemoryNativeBridge()
    const host = createGpuiRenderer(bridge)

    host.render(
      h("div", null, [h("div", { key: "a" }, "A"), h("div", { key: "b" }, "B")]),
      host.root,
    )
    if (bridge.rootId === null) throw new Error("Expected native root")
    const outerId = bridge.nodes.get(bridge.rootId)?.children[0]
    if (outerId === undefined) throw new Error("Expected outer div")

    const before = [...(bridge.nodes.get(outerId)?.children ?? [])]
    host.render(
      h("div", null, [h("div", { key: "b" }, "B2"), h("div", { key: "a" }, "A")]),
      host.root,
    )

    const after = bridge.nodes.get(outerId)?.children ?? []
    expect(after).toEqual([before[1], before[0]])
    const updatedTextId = bridge.nodes.get(after[0] ?? -1)?.children[0]
    expect(bridge.nodes.get(updatedTextId ?? -1)?.text).toBe("B2")
  })

  it("rejects elements that do not have a GPUI mapping yet", () => {
    const host = createGpuiRenderer(new MemoryNativeBridge())
    expect(() => host.render(h("span"), host.root)).toThrow("Unsupported GPUI element tag: span")
  })

  it("drives Vue reactivity through native event handlers", async () => {
    const Counter = defineComponent({
      setup() {
        const count = ref(0)
        return () =>
          h(
            "div",
            {
              testId: "counter",
              onClick: () => {
                count.value += 1
              },
            },
            `Count: ${count.value}`,
          )
      },
    })
    const root = mountGpui(Counter)

    root.findByTestId("counter").click()
    await root.flush()

    expect(root.findByTestId("counter").text).toBe("Count: 1")
    expect(root.renderer.focusedElementId).toBeNull()
    root.unmount()
  })

  it("exposes deterministic automation snapshots", () => {
    const root = mountGpui(
      defineComponent(
        () => () => h("div", { testId: "panel", style: { padding: 12 } }, "Snapshot text"),
      ),
    )

    const snapshot = snapshotRenderer(root.renderer)
    const panel = findAutomationNodeByTestId(snapshot, "panel")

    expect(panel).toMatchObject({ kind: "element", tag: "div" })
    expect(automationText(snapshot)).toContain("Snapshot text")
    root.unmount()
  })

  it("normalizes the native nested automation tree", () => {
    const native = {
      getAutomationTree: () =>
        JSON.stringify({
          type: "div",
          id: 1,
          testId: "root",
          children: [{ type: "text", id: 2, text: "Native text" }],
        }),
    } as unknown as NativeRenderer

    const snapshot = snapshotRenderer(native)
    expect(snapshot.rootId).toBe(1)
    expect(snapshot.nodes.get(2)?.parent).toBe(1)
    expect(findAutomationNodeByTestId(snapshot, "root")?.id).toBe(1)
    expect(automationText(snapshot)).toBe("Native text")
  })

  it("provides native element refs and window controls", () => {
    const root = mountGpui(
      defineComponent({
        setup() {
          const element = useElementRef()
          const window = useGpuiWindow()
          onMounted(() => {
            if (element.value !== null) window.focus(element.value)
          })
          return () => h("div", { ref: element, testId: "focus-target" }, "Focus me")
        },
      }),
    )

    expect(root.renderer.focusedElementId).toBe(root.findByTestId("focus-target").id)
    root.unmount()
  })

  it("exposes typed native components and Vue v-model input semantics", async () => {
    const value = ref("initial")
    const root = mountGpui(
      defineComponent(
        () => () =>
          h("div", null, [
            h(GpuiInput, {
              testId: "editor",
              modelValue: value.value,
              "onUpdate:modelValue": (next: string) => {
                value.value = next
              },
            }),
            h(GpuiCode, {
              testId: "source",
              code: "const answer = 42",
              language: "typescript",
            }),
          ]),
      ),
    )

    const editor = root.findByTestId("editor")
    expect(editor.tag).toBe("input")
    expect(editor.prop("value")).toBe("initial")
    expect(root.findByTestId("source").tag).toBe("code")

    editor.trigger("change", { value: "edited" })
    await root.flush()

    expect(value.value).toBe("edited")
    expect(root.findByTestId("editor").prop("value")).toBe("edited")
    root.unmount()
  })

  it("retains typed canvas commands without a JS paint callback", () => {
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(GpuiCanvas, {
            testId: "plot",
            style: { width: 200, height: 100 },
            commands: [
              { type: "rect", x: 0, y: 0, width: 200, height: 100, fill: "#111" },
              {
                type: "path",
                operations: [
                  { op: "moveTo", x: 0, y: 50 },
                  { op: "lineTo", x: 200, y: 50 },
                ],
                stroke: "#0f0",
                strokeWidth: 2,
              },
            ],
          }),
      ),
    )
    const commands = root.findByTestId("plot").prop("commands") as unknown[]
    expect(commands).toHaveLength(2)
    root.unmount()
  })

  it("lowers motion, keyframes, springs, and stagger to one native prop", () => {
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(MotionDiv, {
            testId: "motion",
            initial: { opacity: 0 },
            animate: [
              { at: 0, value: { left: 0, opacity: 0 } },
              { at: 1, value: { left: 100, opacity: 1 } },
            ],
            transition: stagger(3, 0.05, {
              type: "spring",
              stiffness: 200,
              damping: 24,
            }),
          }),
      ),
    )
    expect(root.findByTestId("motion").prop("motion")).toMatchObject({
      initial: { opacity: 0 },
      transition: {
        type: "spring",
        stagger: 0.05,
        staggerIndex: 3,
      },
    })
    root.unmount()
  })

  it("controls timeline playback and decoded audio chunks", () => {
    const renderer = new MemoryNativeBridge()
    renderer.init({ headless: true })
    const timeline = createTimeline(renderer)
    timeline.pause()
    timeline.seek(1_500)
    timeline.setPlaybackRate(2)
    expect(timeline.state()).toEqual({
      currentTimeMs: 1_500,
      playbackRate: 2,
      playing: false,
    })

    const audio = createAudioFrames(renderer)
    audio.configure(48_000, 2, 4)
    audio.enqueue(new Float32Array([0, 0.1, 0.2, 0.3]))
    expect(audio.state().queuedFrames).toBe(2)
    expect([...audio.dequeue(1)]).toEqual([0, 0.10000000149011612])
  })

  it("keeps independently created windows isolated", () => {
    const firstRenderer = new MemoryNativeBridge()
    const secondRenderer = new MemoryNativeBridge()
    const first = createWindow(
      defineComponent(() => () => h("div", null, "First")),
      { renderer: firstRenderer, headless: true },
    )
    const second = createWindow(
      defineComponent(() => () => h("div", null, "Second")),
      { renderer: secondRenderer, headless: true },
    )
    expect(firstRenderer.getAllText()).toContain("First")
    expect(secondRenderer.getAllText()).toContain("Second")
    first.close()
    expect(firstRenderer.isInitialized()).toBe(false)
    expect(secondRenderer.isInitialized()).toBe(true)
    second.close()
  })

  it("implements Vue-native select state and anchored content", async () => {
    const selected = ref("one")
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(
            Select,
            {
              defaultValue: "one",
              onValueChange: (value: string) => {
                selected.value = value
              },
            },
            {
              default: () => [
                h(
                  SelectTrigger,
                  { testId: "select-trigger" },
                  { default: () => h(SelectValue, { testId: "select-value" }) },
                ),
                h(
                  SelectContent,
                  { testId: "select-content" },
                  {
                    default: () => [
                      h(SelectItem, { value: "one", testId: "option-one" }, () => "One"),
                      h(SelectItem, { value: "two", testId: "option-two" }, () => "Two"),
                    ],
                  },
                ),
              ],
            },
          ),
      ),
    )

    root.findByTestId("select-trigger").click()
    await root.flush()
    expect(root.findByTestId("select-content").tag).toBe("div")

    root.findByTestId("option-two").click()
    await root.flush()
    expect(selected.value).toBe("two")
    expect(root.findByTestId("select-value").text).toBe("Two")
    root.unmount()
  })

  it("implements Vue-native combobox filtering and selection", async () => {
    const selected = ref<string | string[] | null>(null)
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(
            Combobox,
            {
              items: ["alpha", "beta", "gamma"],
              defaultOpen: true,
              onValueChange: (value: string | string[] | null) => {
                selected.value = value
              },
            },
            {
              default: () => [
                h(ComboboxInput, { testId: "combo-input" }),
                h(
                  ComboboxContent,
                  { testId: "combo-content" },
                  {
                    default: () =>
                      h(ComboboxList, null, {
                        default: ({ item }: { item: string }) =>
                          h(ComboboxItem, { value: item, testId: `combo-${item}` }, () => item),
                      }),
                  },
                ),
              ],
            },
          ),
      ),
    )

    root.findByTestId("combo-input").trigger("change", { value: "be" })
    await root.flush()
    expect(root.queryByTestId("combo-alpha")).toBeNull()

    root.findByTestId("combo-beta").click()
    await root.flush()
    expect(selected.value).toBe("beta")
    expect(root.findByTestId("combo-input").prop("value")).toBe("beta")
    root.unmount()
  })

  it("opens Vue-native tooltip content through GPUI hover events", async () => {
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(Tooltip, null, {
            default: () => [
              h(TooltipTrigger, { testId: "tooltip-trigger" }, { default: () => "Hover" }),
              h(TooltipContent, { testId: "tooltip-content" }, { default: () => "Native tooltip" }),
            ],
          }),
      ),
    )

    expect(root.queryByTestId("tooltip-content")).toBeNull()
    root.findByTestId("tooltip-trigger").trigger("mouseEnter")
    await root.flush()
    expect(root.findByTestId("tooltip-content").text).toBe("Native tooltip")
    root.unmount()
  })

  it("provides the typed locator and SSE automation protocol", async () => {
    const clicks: Array<[number, number]> = []
    const automationRenderer: TestAutomationRenderer = {
      nativeSimulateClick: (x, y) => clicks.push([x, y]),
      nativeSimulateMouseDown() {},
      nativeSimulateMouseUp() {},
      nativeSimulateMouseMove() {},
      nativeSimulateScrollWheel() {},
      simulateKeystrokes() {},
      nativeSimulateKeystrokes() {},
      nativeSimulateKeyDown() {},
      nativeSimulateKeyUp() {},
      scrollTo() {},
      getScrollOffset: () => null,
      getAllText: () => ["Save"],
      getPaintedText: () => ["Save"],
      getSelectedText: () => null,
      clearSelection() {},
      captureScreenshot() {},
      getAutomationTree: () =>
        JSON.stringify({
          id: 1,
          type: "div",
          children: [
            {
              id: 2,
              type: "div",
              text: "Save",
              testId: "save",
              bounds: { x: 10, y: 20, width: 80, height: 40 },
            },
          ],
        }),
      getElementBounds: () => null,
      clockPause: () => 0,
      clockSet: (nowMs) => nowMs,
      clockFastForward: (deltaMs) => deltaMs,
      clockResume: () => 0,
    }

    const app = await connectTest(automationRenderer)
    const save = app.getByTestId("save")
    expect(await save.textContent()).toBe("Save")
    await save.click()
    expect(clicks).toEqual([[50, 40]])
    await app.close()

    const wire = encodeSse({
      id: 7,
      method: "initialize",
      params: {
        protocolVersion: AUTOMATION_PROTOCOL_VERSION,
        client: "gpui-vue-test",
      },
    })
    expect(decodeSseChunk(wire)).toEqual([
      {
        id: 7,
        method: "initialize",
        params: {
          protocolVersion: AUTOMATION_PROTOCOL_VERSION,
          client: "gpui-vue-test",
        },
      },
    ])
  })

  it("keeps threaded native windows alive until the renderer closes", async () => {
    const renderer = new MemoryNativeBridge()
    renderer.init()
    let terminated = false
    const loop = startFrameLoop(renderer, {
      keepAlive: true,
      livenessMs: 1,
      onTerminated: () => {
        terminated = true
      },
    })

    renderer.close()
    await new Promise<void>((resolve) => setTimeout(resolve, 5))
    expect(terminated).toBe(true)
    loop.stop()
  })
})
