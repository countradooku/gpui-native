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

import { GpuiRendererKey } from "./context.js"
import { handleGpuiEvent } from "./events.js"
import type { NativeRenderer } from "./native.js"
import { createGpuiRenderer, type GpuiRendererHost } from "./renderer.js"
import type { DebugFrameOverlayMode, EventPayload, StyleDesc } from "./types.js"

interface NativeTestRendererApi extends NativeRenderer {
  applyBatch(json: string): number[]
  flush(): void
  drainEvents(): EventPayload[]
  simulateKeystrokes(keystrokes: string): void
  simulateKeyDown(keystroke: string, isHeld?: boolean): void
  simulateKeyUp(keystroke: string): void
  simulateClick(x: number, y: number): void
  simulateScrollWheel(x: number, y: number, deltaX: number, deltaY: number): void
  simulateMouseMove(x: number, y: number, pressedButton?: number): void
  simulateMouseDown(x: number, y: number, button?: number): void
  simulateMouseUp(x: number, y: number, button?: number): void
  getTreeJson(): string
  getAutomationTree(): string
  getElementBounds(elementId: number): number[] | null
  getRootId(): number | null
  getAllText(): string[]
  getPaintedText(): string[]
  getSyntaxCacheStats(): number[]
  getSelectedText(): string | null
  clearSelection(): void
  dragSelect(x1: number, y1: number, x2: number, y2: number): void
  scrollTo(elementId: number, x: number, y: number): void
  scrollToItem(elementId: number, index: number): void
  getScrollOffset(elementId: number): number[] | null
  setDebugFrameOverlay(mode: DebugFrameOverlayMode): string
  getDebugFrameOverlay(): string
  cycleDebugFrameOverlay(): string
  resetDebugFrameOverlayStats(): void
  captureScreenshot(path: string): void
  clockPause(): number
  clockSet(nowMs: number): number
  clockFastForward(deltaMs: number): number
  clockResume(): number
}

interface NativeTestRendererConstructor {
  new (): NativeTestRendererApi
}

const require = createRequire(import.meta.url)
let NativeTestRenderer: NativeTestRendererConstructor | null = null
try {
  const native = require("@gpui-vue/native") as {
    TestGpuiRenderer?: NativeTestRendererConstructor
  }
  NativeTestRenderer = native.TestGpuiRenderer ?? null
} catch {
  // GPU-backed tests are optional and currently supplied by GPUI on macOS.
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
export class TestRenderer implements NativeRenderer {
  commitCount = 0
  readonly #native: NativeTestRendererApi

  constructor() {
    if (NativeTestRenderer === null) {
      throw new Error("Native TestGpuiRenderer is unavailable; build on macOS with test-support")
    }
    this.#native = new NativeTestRenderer()
  }

  createElement(id: number, elementType: string): void {
    this.#native.createElement(id, elementType)
  }

  destroyElement(id: number): number[] {
    return this.#native.destroyElement(id)
  }

  appendChild(parentId: number, childId: number): void {
    this.#native.appendChild(parentId, childId)
  }

  removeChild(parentId: number, childId: number): void {
    this.#native.removeChild(parentId, childId)
  }

  insertBefore(parentId: number, childId: number, beforeId: number): void {
    this.#native.insertBefore(parentId, childId, beforeId)
  }

  setStyle(id: number, style: string | StyleDesc | Record<string, unknown>): void {
    this.#native.setStyle(id, typeof style === "string" ? style : JSON.stringify(style))
  }

  setText(id: number, content: string): void {
    this.#native.setText(id, content)
  }

  setEventListener(id: number, eventType: string, hasHandler: boolean): void {
    this.#native.setEventListener(id, eventType, hasHandler)
  }

  setRoot(id: number): void {
    this.#native.setRoot(id)
  }

  setCustomProp(id: number, key: string, value: string | object | number | boolean | null): void {
    this.#native.setCustomProp(id, key, typeof value === "string" ? value : JSON.stringify(value))
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
      for (const event of events) handleGpuiEvent(event)
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

  nativeSimulateClick(x: number, y: number): void {
    this.#native.flush()
    this.#native.simulateClick(x, y)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateScrollWheel(x: number, y: number, deltaX: number, deltaY: number): void {
    this.#native.flush()
    this.#native.simulateScrollWheel(x, y, deltaX, deltaY)
    this.dispatchNativeEvents()
  }

  nativeSimulateMouseMove(x: number, y: number, pressedButton?: number): void {
    this.#native.flush()
    this.#native.simulateMouseMove(x, y, pressedButton)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateMouseDown(x: number, y: number, button = 0): void {
    this.#native.flush()
    this.#native.simulateMouseDown(x, y, button)
    this.dispatchNativeEvents()
    this.#native.flush()
  }

  nativeSimulateMouseUp(x: number, y: number, button = 0): void {
    this.#native.flush()
    this.#native.simulateMouseUp(x, y, button)
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

  getPaintedText(): string[] {
    return this.#native.getPaintedText()
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

  scrollToItem(elementId: number, index: number): void {
    this.#native.flush()
    this.#native.scrollToItem(elementId, index)
    this.#native.flush()
  }

  getScrollOffset(elementId: number): [number, number] | null {
    const offset = this.#native.getScrollOffset(elementId)
    return offset === null ? null : [offset[0] ?? 0, offset[1] ?? 0]
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
export function createTestRoot(): NativeGpuiTestRoot {
  const renderer = new TestRenderer()
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
      app = host.createApp(Root)
      app.provide(GpuiRendererKey, host.renderer)
      app.mount(host.root)
      host.flushMutations()
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
