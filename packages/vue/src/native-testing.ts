import { createRequire } from "node:module"

import {
  defineComponent,
  h,
  isVNode,
  nextTick,
  type App,
  type Component,
  type VNode,
} from "@vue/runtime-core"

import { handleGpuiEvent } from "./events.js"
import { MutationRenderer } from "./mutation-renderer.js"
import type { NativeRenderer } from "./native.js"
import { createGpuiRenderer, type GpuiRendererHost } from "./renderer.js"
import type {
  DebugFrameOverlayMode,
  DebugFrameOverlayStats,
  EventPayload,
  HighlightMatch,
  WindowSize,
} from "./types.js"

interface NativeTestRendererApi extends NativeRenderer {
  applyBatch(json: string): number[]
  flush(): void
  drainEvents(): EventPayload[]
  simulateKeystrokes(keystrokes: string): void
  simulateKeyDown(keystroke: string, isHeld?: boolean): void
  simulateKeyUp(keystroke: string): void
  simulateClick(x: number, y: number, button?: number, modifiers?: string): void
  simulateScrollWheel(
    x: number,
    y: number,
    deltaX: number,
    deltaY: number,
    modifiers?: string,
  ): void
  simulateMouseMove(x: number, y: number, pressedButton?: number, modifiers?: string): void
  simulateMouseDown(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseUp(x: number, y: number, button?: number, modifiers?: string): void
  getTreeJson(): string
  getAutomationTree(): string
  getElementBounds(elementId: number): number[] | null
  getRootId(): number | null
  getWindowSize(): WindowSize
  getAllText(): string[]
  getRetainedElementCount(): number
  getPaintedText(): string[]
  getPaintedHighlights(): HighlightMatch[]
  getSyntaxCacheStats(): number[]
  getSelectedText(): string | null
  clearSelection(): void
  dragSelect(x1: number, y1: number, x2: number, y2: number): void
  scrollTo(elementId: number, x: number, y: number): void
  scrollToItem(elementId: number, index: number, offsetInItem?: number): void
  getScrollOffset(elementId: number): number[] | null
  getListScrollTop(elementId: number): number[] | null
  setDebugFrameOverlay(mode: DebugFrameOverlayMode): string
  getDebugFrameOverlay(): string
  cycleDebugFrameOverlay(): string
  resetDebugFrameOverlayStats(): void
  getDebugFrameOverlayStats(): DebugFrameOverlayStats
  advanceTime(milliseconds: number): void
  getA11yTree(): string
  captureScreenshot(path: string): void
  clockPause(): number
  clockSet(nowMs: number): number
  clockFastForward(deltaMs: number): number
  clockResume(): number
}

interface NativeTestRendererConstructor {
  new (width?: number, height?: number): NativeTestRendererApi
}

/** Offscreen window size for a test root. Native defaults to 1280×800. */
export interface TestWindowOptions {
  width?: number
  height?: number
}

const require = createRequire(import.meta.url)
let NativeTestRenderer: NativeTestRendererConstructor | null = null
try {
  const native = require("@gpui-native/core") as {
    TestGpuiRenderer?: NativeTestRendererConstructor
  }
  NativeTestRenderer = native.TestGpuiRenderer ?? null
} catch {
  // GPU-backed tests are optional and supplied by GPUI on macOS and Windows.
}

export const hasNativeTestRenderer = NativeTestRenderer !== null

export interface NativeTestElement {
  id: number
  type: string
  style: Record<string, unknown>
  text: string | null
  events: Set<string>
  children: number[]
  parentId: number | null
  customProps?: Record<string, unknown>
}

/** Vue adapter over GPUI's real GPU-backed TestGpuiRenderer. */
export class TestRenderer extends MutationRenderer implements NativeRenderer {
  commitCount = 0
  readonly #native: NativeTestRendererApi

  constructor(options: TestWindowOptions = {}) {
    super()
    if (NativeTestRenderer === null) {
      throw new Error(
        "Native TestGpuiRenderer is unavailable; build on macOS or Windows with test-support",
      )
    }
    this.#native = new NativeTestRenderer(options.width, options.height)
  }

  getCustomProp(id: number, key: string): string | null {
    return this.#native.getCustomProp?.(id, key) ?? null
  }

  commitMutations(): void {
    this.#native.commitMutations()
    this.commitCount += 1
  }

  applyBatch(json: string): number[] {
    this.commitCount += 1
    return this.#native.applyBatch(json)
  }

  protected applyMutations(mutations: unknown[][]): number[] {
    return this.#native.applyBatch(JSON.stringify(mutations))
  }

  flush(): void {
    this.#native.flush()
  }

  drainEvents(): EventPayload[] {
    return this.#native.drainEvents()
  }

  dispatchNativeEvents(): void {
    for (;;) {
      const events = this.#native.drainEvents()
      if (events.length === 0) return
      for (const event of events) handleGpuiEvent(event, this)
    }
  }

  simulateKeystrokes(keystrokes: string): void {
    this.#native.flush()
    this.#native.simulateKeystrokes(keystrokes)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateKeystrokes(elementId: number, keystrokes: string): void {
    this.#native.flush()
    this.#native.focusElement?.(elementId)
    this.#native.simulateKeystrokes(keystrokes)
    this.dispatchNativeEvents()
  }

  nativeSimulateKeyDown(elementId: number, key: string, isHeld?: boolean): void {
    this.#native.flush()
    this.#native.focusElement?.(elementId)
    this.#native.simulateKeyDown(key, isHeld)
    this.dispatchNativeEvents()
  }

  nativeSimulateKeyUp(elementId: number, key: string): void {
    this.#native.flush()
    this.#native.focusElement?.(elementId)
    this.#native.simulateKeyUp(key)
    this.dispatchNativeEvents()
  }

  nativeSimulateClick(x: number, y: number, button?: number, modifiers?: string): void {
    this.#native.flush()
    this.#native.simulateClick(x, y, button, modifiers)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateScrollWheel(
    x: number,
    y: number,
    deltaX: number,
    deltaY: number,
    modifiers?: string,
  ): void {
    this.#native.flush()
    this.#native.simulateScrollWheel(x, y, deltaX, deltaY, modifiers)
    this.dispatchNativeEvents()
  }

  nativeSimulateMouseMove(x: number, y: number, pressedButton?: number, modifiers?: string): void {
    this.#native.flush()
    this.#native.simulateMouseMove(x, y, pressedButton, modifiers)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateMouseDown(x: number, y: number, button = 0, modifiers?: string): void {
    this.#native.flush()
    this.#native.simulateMouseDown(x, y, button, modifiers)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateMouseUp(x: number, y: number, button = 0, modifiers?: string): void {
    this.#native.flush()
    this.#native.simulateMouseUp(x, y, button, modifiers)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  getRoot(): NativeTestElement | undefined {
    const rootId = this.#native.getRootId()
    return rootId === null ? undefined : this.#elementMap().get(rootId)
  }

  getElement(id: number): NativeTestElement | undefined {
    return this.#elementMap().get(id)
  }

  findByType(type: string): NativeTestElement[] {
    return [...this.#elementMap().values()].filter((element) => element.type === type)
  }

  findByText(text: string): NativeTestElement | undefined {
    return [...this.#elementMap().values()].find((element) => element.text?.includes(text) === true)
  }

  toJSON(): unknown {
    return JSON.parse(this.#native.getTreeJson()) as unknown
  }

  getAutomationTree(): string {
    return this.#native.getAutomationTree()
  }

  getElementBounds(elementId: number): number[] | null {
    return this.#native.getElementBounds(elementId)
  }

  getAllText(): string[] {
    return this.#native.getAllText()
  }

  getRetainedElementCount(): number {
    return this.#native.getRetainedElementCount()
  }

  getPaintedText(): string[] {
    return this.#native.getPaintedText()
  }

  getPaintedHighlights(): HighlightMatch[] {
    return this.#native.getPaintedHighlights()
  }

  getWindowSize(): WindowSize {
    return this.#native.getWindowSize()
  }

  getSyntaxCacheStats(): [number, number, number] {
    const values = this.#native.getSyntaxCacheStats()
    return [values[0] ?? 0, values[1] ?? 0, values[2] ?? 0]
  }

  focusElement(elementId: number): void {
    this.#native.flush()
    this.#native.focusElement?.(elementId)
    this.dispatchNativeEvents()
  }

  scrollTo(elementId: number, x: number, y: number): void {
    this.#native.flush()
    this.#native.scrollTo(elementId, x, y)
    this.#native.flush()
  }

  scrollToItem(elementId: number, index: number, offsetInItem?: number): void {
    this.#native.flush()
    this.#native.scrollToItem(elementId, index, offsetInItem)
    this.#native.flush()
  }

  getScrollOffset(elementId: number): [number, number] | null {
    const offset = this.#native.getScrollOffset(elementId)
    return offset === null ? null : [offset[0] ?? 0, offset[1] ?? 0]
  }

  getListScrollTop(elementId: number): [number, number, number] | null {
    this.#native.flush()
    const top = this.#native.getListScrollTop(elementId)
    return top === null ? null : [top[0] ?? 0, top[1] ?? 0, top[2] ?? 0]
  }

  dragSelect(x1: number, y1: number, x2: number, y2: number): string | null {
    this.#native.dragSelect(x1, y1, x2, y2)
    return this.#native.getSelectedText()
  }

  getSelectedText(): string | null {
    return this.#native.getSelectedText()
  }

  clearSelection(): void {
    this.#native.clearSelection()
    this.#native.flush()
  }

  setDebugFrameOverlay(mode: DebugFrameOverlayMode): string {
    return this.#native.setDebugFrameOverlay(mode)
  }

  getDebugFrameOverlay(): string {
    return this.#native.getDebugFrameOverlay()
  }

  cycleDebugFrameOverlay(): string {
    return this.#native.cycleDebugFrameOverlay()
  }

  resetDebugFrameOverlayStats(): void {
    this.#native.resetDebugFrameOverlayStats()
  }

  getDebugFrameOverlayStats(): DebugFrameOverlayStats {
    return this.#native.getDebugFrameOverlayStats()
  }

  advanceTime(milliseconds: number): void {
    this.#native.advanceTime(milliseconds)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  getA11yTree(): unknown {
    this.flush()
    return JSON.parse(this.#native.getA11yTree()) as unknown
  }

  captureScreenshot(path: string): void {
    this.#native.flush()
    this.#native.captureScreenshot(path)
  }

  clockPause(): number {
    return this.#native.clockPause()
  }

  clockSet(nowMs: number): number {
    return this.#native.clockSet(nowMs)
  }

  clockFastForward(deltaMs: number): number {
    return this.#native.clockFastForward(deltaMs)
  }

  clockResume(): number {
    return this.#native.clockResume()
  }

  get hasNative(): boolean {
    return true
  }

  #elementMap(): Map<number, NativeTestElement> {
    const root = JSON.parse(this.#native.getTreeJson()) as unknown
    const elements = new Map<number, NativeTestElement>()
    const visit = (value: unknown, parentId: number | null): void => {
      if (value === null || typeof value !== "object") return
      const node = value as {
        id?: unknown
        type?: unknown
        style?: unknown
        text?: unknown
        events?: unknown
        customProps?: unknown
        children?: unknown
      }
      if (typeof node.id !== "number" || typeof node.type !== "string") return
      const children = Array.isArray(node.children) ? node.children : []
      const childIds = children.flatMap((child) => {
        if (child === null || typeof child !== "object") return []
        const id = (child as { id?: unknown }).id
        return typeof id === "number" ? [id] : []
      })
      elements.set(node.id, {
        id: node.id,
        type: node.type,
        style:
          node.style !== null && typeof node.style === "object"
            ? (node.style as Record<string, unknown>)
            : {},
        text: typeof node.text === "string" ? node.text : null,
        events: new Set(Array.isArray(node.events) ? node.events.map(String) : []),
        children: childIds,
        parentId,
        ...(node.customProps !== null && typeof node.customProps === "object"
          ? { customProps: node.customProps as Record<string, unknown> }
          : {}),
      })
      for (const child of children) visit(child, node.id)
    }
    visit(root, null)
    return elements
  }
}

export interface NativeGpuiTestRoot {
  readonly host: GpuiRendererHost
  readonly renderer: TestRenderer
  render(node: Component | VNode): void
  flush(): Promise<void>
  unmount(): void
}

/** Creates a Vue root backed by GPUI's native GPU test application. */
export function createTestRoot(options: TestWindowOptions = {}): NativeGpuiTestRoot {
  const renderer = new TestRenderer(options)
  const host = createGpuiRenderer(renderer)
  let app: App | null = null

  const unmount = (): void => {
    app?.unmount()
    app = null
    host.flushMutations()
  }

  return {
    host,
    renderer,
    render(node): void {
      unmount()
      const Root = defineComponent({
        name: "GpuiNativeTestRoot",
        setup: () => () => (isVNode(node) ? node : h(node)),
      })
      app = host.mount(Root)
      renderer.flush()
    },
    async flush(): Promise<void> {
      await nextTick()
      host.flushMutations()
      renderer.flush()
    },
    unmount,
  }
}
