import { defineComponent, Fragment, h, onMounted, ref } from "@vue/runtime-core"
import { describe, expect, it, vi } from "vitest"

import {
  MemoryNativeRenderer,
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
  SelectScrollDownButton,
  SelectTrigger,
  SelectValue,
  SseBackend,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
  AUTOMATION_PROTOCOL_VERSION,
  automationMethods,
  automationText,
  connectTest,
  createAudioFrames,
  createGpuiRenderer,
  createSseDecoder,
  createTimeline,
  createWindow,
  decodeSseChunk,
  encodeSse,
  findRanges,
  findAutomationNodeByTestId,
  handleGpuiEvent,
  mountGpui,
  MotionDiv,
  snapshotRenderer,
  startFrameLoop,
  useElementRef,
  useGpuiWindow,
  useTextSearch,
  wrapWithBatching,
  stagger,
  type NativeRenderer,
  type TestAutomationRenderer,
} from "../src/index.js"
import { jsx, jsxs } from "../src/jsx-runtime.js"

function countingRendererForTest() {
  const counts = new Map<string, number>()
  const count = (operation: string): void => {
    counts.set(operation, (counts.get(operation) ?? 0) + 1)
  }
  const renderer: NativeRenderer = {
    createElement: () => count("createElement"),
    destroyElement: () => {
      count("destroyElement")
      return []
    },
    appendChild: () => count("appendChild"),
    removeChild: () => count("removeChild"),
    insertBefore: () => count("insertBefore"),
    setStyle: () => count("setStyle"),
    setText: () => count("setText"),
    setEventListener: () => count("setEventListener"),
    setRoot: () => count("setRoot"),
    setCustomProp: () => count("setCustomProp"),
    commitMutations: () => count("commitMutations"),
  }
  return { counts, renderer }
}

describe("GPUI Vue renderer", () => {
  it("allows exactly one live Vue root per native renderer", () => {
    const bridge = new MemoryNativeRenderer()
    const first = createGpuiRenderer(bridge)

    expect(() => createGpuiRenderer(bridge)).toThrow(/one live Vue root/)
    first.destroy()
    first.destroy()

    const second = createGpuiRenderer(bridge)
    second.flushMutations()
    expect(bridge.getRetainedElementCount()).toBe(1)
    second.destroy()
  })

  it("reclaims removed native text nodes", () => {
    const bridge = new MemoryNativeRenderer()
    const host = createGpuiRenderer(bridge)
    host.render(h("div", null, "hello"), host.root)
    expect(bridge.getRetainedElementCount()).toBe(3)

    host.render(h("div"), host.root)
    expect(bridge.getRetainedElementCount()).toBe(2)
    host.destroy()
  })

  it("forwards virtual-list pixel anchors including negative offsets", () => {
    const bridge = new MemoryNativeRenderer()
    const host = createGpuiRenderer(bridge)
    host.render(h("virtual-list", { style: { height: 160 } }), host.root)
    const list = [...bridge.nodes.values()].find((node) => node.type === "virtual-list")
    if (list === undefined) throw new Error("Expected virtual-list")

    bridge.scrollToItem(list.id, 50, -100)
    expect(bridge.getListScrollTop(list.id)).toEqual([50, -100, 160])
    expect(bridge.getListScrollTop(host.root.id)).toBeNull()
    host.destroy()
  })

  it("opens hidden or unfocused and can activate later", () => {
    const bridge = new MemoryNativeRenderer()
    bridge.init({ focus: false, show: false })
    expect(bridge.windowActive).toBe(false)
    expect(bridge.windowVisible).toBe(false)

    bridge.activateWindow()
    expect(bridge.windowActive).toBe(true)
    expect(bridge.windowVisible).toBe(true)
  })

  it("adapts the automatic JSX children and key ABI", () => {
    const child = jsx("text", { children: "child" })
    const vnode = jsxs("div", { testId: "jsx", children: ["first", child] }, "stable")
    expect(vnode.key).toBe("stable")
    expect(vnode.props?.children).toBeUndefined()

    const bridge = new MemoryNativeRenderer()
    const host = createGpuiRenderer(bridge)
    host.render(vnode, host.root)
    expect(bridge.getAllText()).toEqual(["first", "child"])
  })

  it("maps div and text nodes into the native retained tree", () => {
    const bridge = new MemoryNativeRenderer()
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
    const bridge = new MemoryNativeRenderer()
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
    expect(updatedTextId).toBe(bridge.nodes.get(before[1] ?? -1)?.children[0])
    expect(bridge.nodes.get(updatedTextId ?? -1)?.text).toBe("B2")
  })

  it("does not resend structurally unchanged inline motion props", async () => {
    const updates = ref(0)
    const boundary = countingRendererForTest()
    const host = createGpuiRenderer(boundary.renderer)
    host.mount(
      defineComponent(
        () => () => h(MotionDiv, { animate: { opacity: 1 } }, () => String(updates.value)),
      ),
    )
    boundary.counts.clear()

    updates.value += 1
    await Promise.resolve()
    host.flushMutations()

    expect(boundary.counts.get("setCustomProp") ?? 0).toBe(0)
    expect(boundary.counts.get("setText")).toBe(1)
    host.destroy()
  })

  it("keeps Vue fragment anchors out of native layout", () => {
    const bridge = new MemoryNativeRenderer()
    const host = createGpuiRenderer(bridge)
    host.render(h(Fragment, null, [h("div", null, "first"), h("div", null, "second")]), host.root)

    expect([...bridge.nodes.values()].filter((node) => node.type === "text")).toHaveLength(2)
    expect([...bridge.nodes.values()].every((node) => node.text !== "")).toBe(true)
  })

  it("applies mutation batches atomically in the memory renderer", () => {
    const bridge = new MemoryNativeRenderer()
    bridge.createElement(1, "text")
    bridge.setText(1, "before")

    expect(() =>
      bridge.applyBatch(
        JSON.stringify([
          ["setText", 1, "after"],
          ["setStyle", 1, "{invalid json"],
        ]),
      ),
    ).toThrow(/JSON|position|property/i)
    expect(bridge.nodes.get(1)?.text).toBe("before")
    expect(bridge.commitCount).toBe(0)
  })

  it("drops rejected batching waves and reuses proxy method closures", () => {
    const waves: string[] = []
    let reject = true
    const inner = {
      applyBatch(json: string): number[] {
        waves.push(json)
        if (reject) throw new Error("rejected wave")
        return []
      },
    } as unknown as NativeRenderer
    const renderer = wrapWithBatching(inner)
    const stableSetText = renderer.setText

    renderer.setText(1, "bad")
    expect(() => renderer.flushMutations()).toThrow("rejected wave")
    reject = false
    renderer.setText(1, "good")
    renderer.flushMutations()

    expect(renderer.setText).toBe(stableSetText)
    expect(JSON.parse(waves[1] ?? "null")).toEqual([["setText", 1, "good"]])
  })

  it("rejects elements that do not have a GPUI mapping yet", () => {
    const host = createGpuiRenderer(new MemoryNativeRenderer())
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

  it("maps auxiliary clicks, visible ranges, and text highlights", () => {
    const bridge = new MemoryNativeRenderer()
    const host = createGpuiRenderer(bridge)
    host.render(
      h(
        "virtual-list",
        {
          itemCount: 100,
          estimatedItemHeight: 24,
          windowStart: 20,
          highlight: { query: "item", activeIndex: 1, matchIndexOffset: 5 },
          onAuxClick() {},
          onVisibleRange() {},
          onHighlight() {},
        },
        "item",
      ),
      host.root,
    )

    const root = bridge.nodes.get(bridge.rootId ?? -1)
    const list = bridge.nodes.get(root?.children[0] ?? -1)
    expect(list?.events).toEqual(new Set(["auxClick", "visibleRange", "highlight"]))
    expect(list?.customProps).toMatchObject({
      itemCount: 100,
      estimatedItemHeight: 24,
      windowStart: 20,
      highlight: { query: "item", activeIndex: 1, matchIndexOffset: 5 },
    })
  })

  it("matches native text-search rules and drives a reactive find cursor", () => {
    expect(findRanges({ text: "cat scatter CAT", query: "cat", wholeWord: true })).toEqual([
      [0, 3],
      [12, 15],
    ])
    expect(findRanges({ text: "İstanbul and fox", query: "fox" })).toEqual([[13, 16]])

    const query = ref("todo")
    const search = useTextSearch({ query })
    search.props.value.onHighlight?.({ elementId: 1, eventType: "highlight", matchCount: 3 })
    expect(search.total.value).toBe(3)
    search.next()
    expect(search.active.value).toBe(1)
    expect(search.props.value.highlight).toMatchObject({ query: "todo", activeIndex: 1 })
    query.value = ""
    expect(search.total.value).toBe(0)
    expect(search.props.value.highlight).toBeNull()
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
    expect(() =>
      automationMethods.getTree.result.parse({
        tree: JSON.parse(root.renderer.getAutomationTree()) as unknown,
      }),
    ).not.toThrow()
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
    const renderer = new MemoryNativeRenderer()
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

    audio.configure(48_000, 2, 100_000)
    expect(() => audio.enqueue(new Float32Array(200_000))).not.toThrow()
    expect(audio.state().queuedFrames).toBe(100_000)
  })

  it("keeps independently created windows isolated", () => {
    const firstRenderer = new MemoryNativeRenderer()
    const secondRenderer = new MemoryNativeRenderer()
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
              modelValue: selected.value,
              "onUpdate:modelValue": (value: string) => {
                selected.value = value
              },
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

  it("supports default v-model, textValue typeahead, and active-option scrolling", async () => {
    const selected = ref("one")
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(
            Select,
            {
              modelValue: selected.value,
              defaultOpen: true,
              "onUpdate:modelValue": (value: string) => {
                selected.value = value
              },
            },
            {
              default: () => [
                h(SelectTrigger, { testId: "typeahead-trigger" }, () => h(SelectValue)),
                h(
                  SelectContent,
                  { testId: "typeahead-content", style: { maxHeight: 30, overflowY: "scroll" } },
                  {
                    default: () => [
                      h(SelectItem, { value: "one", textValue: "Alpha" }, () => "First"),
                      h(SelectItem, { value: "two", textValue: "Bravo" }, () => "Second"),
                      h(SelectScrollDownButton, { testId: "select-scroll-down" }, () => "Down"),
                    ],
                  },
                ),
              ],
            },
          ),
      ),
    )

    root.findByTestId("typeahead-content").trigger("keyDown", { key: "b", keyChar: "b" })
    root.findByTestId("typeahead-content").trigger("keyDown", { key: "enter" })
    await root.flush()
    expect(selected.value).toBe("two")

    root.findByTestId("typeahead-trigger").click()
    await root.flush()
    root.findByTestId("select-scroll-down").click()
    root.findByTestId("typeahead-content").trigger("keyDown", { key: "enter" })
    await root.flush()
    expect(selected.value).toBe("one")
    root.unmount()
  })

  it("updates select registration when an item's value changes", async () => {
    const itemValue = ref("old")
    const selected = ref<string>()
    const root = mountGpui(
      defineComponent(
        () => () =>
          h(
            Select,
            {
              defaultOpen: true,
              "onUpdate:modelValue": (value: string) => {
                selected.value = value
              },
            },
            () => [
              h(SelectTrigger, null, () => h(SelectValue)),
              h(SelectContent, null, () =>
                h(
                  SelectItem,
                  { value: itemValue.value, testId: "dynamic-option" },
                  () => "Dynamic",
                ),
              ),
            ],
          ),
      ),
    )

    itemValue.value = "new"
    await root.flush()
    root.findByTestId("dynamic-option").click()
    await root.flush()

    expect(selected.value).toBe("new")
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
              modelValue: selected.value,
              defaultOpen: true,
              "onUpdate:modelValue": (value: string | string[] | null) => {
                selected.value = value
              },
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

  it("does not enable tooltip skip-delay when a closed trigger is pressed", async () => {
    vi.useFakeTimers()
    try {
      const root = mountGpui(
        defineComponent(
          () => () =>
            h(TooltipProvider, { delayDuration: 100, skipDelayDuration: 500 }, () =>
              h(Tooltip, null, {
                default: () => [
                  h(TooltipTrigger, { testId: "delayed-tooltip-trigger" }, () => "Hover"),
                  h(TooltipContent, { testId: "delayed-tooltip-content" }, () => "Delayed"),
                ],
              }),
            ),
        ),
      )

      const trigger = root.findByTestId("delayed-tooltip-trigger")
      trigger.trigger("mouseDown")
      trigger.trigger("mouseEnter")
      await root.flush()
      expect(root.queryByTestId("delayed-tooltip-content")).toBeNull()

      vi.advanceTimersByTime(100)
      await root.flush()
      expect(root.findByTestId("delayed-tooltip-content").text).toBe("Delayed")
      root.unmount()
    } finally {
      vi.useRealTimers()
    }
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
    const clicks: Array<[number, number, number | undefined, string | undefined]> = []
    const moves: Array<[number, number, number | undefined, string | undefined]> = []
    const downs: Array<[number, number, number | undefined, string | undefined]> = []
    const ups: Array<[number, number, number | undefined, string | undefined]> = []
    const automationRenderer: TestAutomationRenderer = {
      nativeSimulateClick: (x, y, button, modifiers) => clicks.push([x, y, button, modifiers]),
      nativeSimulateMouseDown: (x, y, button, modifiers) => downs.push([x, y, button, modifiers]),
      nativeSimulateMouseUp: (x, y, button, modifiers) => ups.push([x, y, button, modifiers]),
      nativeSimulateMouseMove: (x, y, button, modifiers) => moves.push([x, y, button, modifiers]),
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
    await save.click({ button: 2, modifiers: "shift" })
    expect(clicks).toEqual([[50, 40, 2, "shift"]])
    await save.dragBy(20, 10, { steps: 2, modifiers: "cmd" })
    expect(downs).toEqual([[50, 40, 0, "cmd"]])
    expect(moves).toEqual([
      [50, 40, undefined, "cmd"],
      [60, 45, 0, "cmd"],
      [70, 50, 0, "cmd"],
    ])
    expect(ups).toEqual([[70, 50, 0, "cmd"]])
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

  it("times out and closes pending SSE automation calls", async () => {
    const timeoutBackend = new SseBackend(
      () => {},
      () => {},
      undefined,
      { requestTimeoutMs: 5 },
    )
    await expect(
      timeoutBackend.call("initialize", {
        protocolVersion: AUTOMATION_PROTOCOL_VERSION,
        client: "timeout-test",
      }),
    ).rejects.toMatchObject({ code: "Timeout" })

    let closeTransport: ((reason?: unknown) => void) | undefined
    const closeBackend = new SseBackend(
      () => {},
      () => {},
      undefined,
      {
        requestTimeoutMs: 1_000,
        subscribeClose: (listener) => {
          closeTransport = listener
        },
      },
    )
    const pending = closeBackend.call("getTree", {})
    closeTransport?.("EOF")
    await expect(pending).rejects.toMatchObject({ code: "Closed" })
  })

  it("recovers the SSE decoder after malformed input", () => {
    const messages: unknown[] = []
    const decoder = createSseDecoder((message) => messages.push(message))
    expect(() => decoder.feed("data: {malformed}\n\n")).toThrow(/Invalid automation JSON/)
    decoder.reset()
    decoder.feed(
      encodeSse({
        id: 9,
        method: "initialize",
        params: { protocolVersion: AUTOMATION_PROTOCOL_VERSION, client: "recovered" },
      }),
    )
    expect(messages).toHaveLength(1)
  })

  it("keeps threaded native windows alive until the renderer closes", async () => {
    const renderer = new MemoryNativeRenderer()
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

  it("keeps event-driven native windows alive until the close event", () => {
    vi.useFakeTimers()
    try {
      const renderer: NativeRenderer = new MemoryNativeRenderer()
      renderer.init?.()
      renderer.supportsWindowEvents = () => true
      let terminated = false
      const loop = startFrameLoop(renderer, {
        keepAlive: true,
        onTerminated: () => {
          terminated = true
        },
      })

      expect(vi.getTimerCount()).toBe(1)
      handleGpuiEvent({ elementId: 0, eventType: "windowClose" }, renderer)
      expect(terminated).toBe(true)
      expect(vi.getTimerCount()).toBe(0)
      loop.stop()
    } finally {
      vi.useRealTimers()
    }
  })
})
