import initWasm, { WebGpuiRenderer as WasmGpuiRenderer } from "@gpui-vue/wasm"

import { handleGpuiEvent } from "./events.js"
import type { NativeNodeId, NativeRenderer } from "./native.js"
import type {
  AudioBufferState,
  DebugFrameOverlayMode,
  EventPayload,
  HighlightMatch,
  NativeWindowInsets,
  NativeWindowOptions,
  StyleDesc,
  TimelineState,
  WindowSize,
} from "./types.js"

let initialization: Promise<void> | undefined
let initialized = false

/** Load the Rust GPUI Web module. Call once before render() or createWindow(). */
export function initGpuiWeb(): Promise<void> {
  initialization ??= initWasm().then(() => {
    initialized = true
  })
  return initialization
}

function parseJson<T>(json: string): T {
  return JSON.parse(json) as T
}

export class WebNativeRenderer implements NativeRenderer {
  readonly #wasm: WasmGpuiRenderer
  readonly #onEvent: ((event: EventPayload) => void) | undefined
  #eventFrame: number | undefined

  constructor(onEvent?: (event: EventPayload) => void) {
    if (!initialized) {
      throw new Error("gpui-vue WebAssembly is not initialized; await initGpuiWeb() first")
    }
    this.#wasm = new WasmGpuiRenderer()
    this.#onEvent = onEvent
  }

  createElement(id: NativeNodeId, elementType: string): void {
    this.#apply([["createElement", id, elementType]])
  }

  destroyElement(id: NativeNodeId): NativeNodeId[] {
    return this.#apply([["destroyElement", id]])
  }

  appendChild(parentId: NativeNodeId, childId: NativeNodeId): void {
    this.#apply([["appendChild", parentId, childId]])
  }

  removeChild(parentId: NativeNodeId, childId: NativeNodeId): void {
    this.#apply([["removeChild", parentId, childId]])
  }

  insertBefore(parentId: NativeNodeId, childId: NativeNodeId, beforeId: NativeNodeId): void {
    this.#apply([["insertBefore", parentId, childId, beforeId]])
  }

  setStyle(id: NativeNodeId, style: string | StyleDesc | Record<string, unknown>): void {
    this.#apply([["setStyle", id, style]])
  }

  setText(id: NativeNodeId, content: string): void {
    this.#apply([["setText", id, content]])
  }

  setEventListener(id: NativeNodeId, eventType: string, hasHandler: boolean): void {
    this.#apply([["setEventListener", id, eventType, hasHandler]])
  }

  setRoot(id: NativeNodeId): void {
    this.#apply([["setRoot", id]])
  }

  setCustomProp(
    id: NativeNodeId,
    key: string,
    value: string | object | number | boolean | null,
  ): void {
    this.#apply([["setCustomPropValue", id, key, value]])
  }

  getCustomProp(id: NativeNodeId, key: string): string | null {
    return this.#wasm.getCustomProp(id, key) ?? null
  }

  commitMutations(): void {
    this.#wasm.commitMutations()
  }

  applyBatch(json: string): NativeNodeId[] {
    return Array.from(this.#wasm.applyBatch(json))
  }

  init(options: NativeWindowOptions | null = null): void {
    this.#wasm.init(JSON.stringify(options ?? {}))
    this.#startEventLoop()
  }

  close(): void {
    if (this.#eventFrame !== undefined) cancelAnimationFrame(this.#eventFrame)
    this.#eventFrame = undefined
    this.#wasm.close()
  }

  isInitialized(): boolean {
    return this.#wasm.isInitialized()
  }

  requiresTick(): boolean {
    return this.#wasm.requiresTick()
  }

  tick(): boolean {
    const alive = this.#wasm.tick()
    this.#drainEvents()
    return alive
  }

  getWindowSize(): WindowSize {
    return parseJson(this.#wasm.getWindowSizeJson())
  }

  getWindowInsets(): NativeWindowInsets {
    return parseJson(this.#wasm.getWindowInsetsJson())
  }

  setWindowTitle(title: string): void {
    this.#wasm.setWindowTitle(title)
  }

  focusElement(elementId: NativeNodeId): void {
    this.#wasm.focusElement(elementId)
  }

  blur(): void {
    this.#wasm.blur()
  }

  getSelectedText(): string | null {
    return this.#wasm.getSelectedText() ?? null
  }

  clearSelection(): void {
    this.#wasm.clearSelection()
  }

  scrollTo(elementId: NativeNodeId, x: number, y: number): void {
    this.#wasm.scrollTo(elementId, x, y)
  }

  scrollToItem(elementId: NativeNodeId, index: number): void {
    this.#wasm.scrollToItem(elementId, index)
  }

  getScrollOffset(elementId: NativeNodeId): number[] | null {
    return parseJson(this.#wasm.getScrollOffsetJson(elementId))
  }

  getAutomationTree(): string {
    return this.#wasm.getAutomationTree()
  }

  getElementBounds(id: NativeNodeId): number[] | null {
    return parseJson(this.#wasm.getElementBoundsJson(id))
  }

  getAllText(): string[] {
    return parseJson(this.#wasm.getAllTextJson())
  }

  getPaintedText(): string[] {
    return parseJson(this.#wasm.getPaintedTextJson())
  }

  getPaintedHighlights(): HighlightMatch[] {
    return parseJson(this.#wasm.getPaintedHighlightsJson())
  }

  simulateKeystrokes(keystrokes: string): void {
    this.#wasm.simulateKeystrokes(keystrokes)
  }

  simulateKeyDown(keystroke: string, isHeld?: boolean): void {
    this.#wasm.simulateKeyDown(keystroke, isHeld)
  }

  simulateKeyUp(keystroke: string): void {
    this.#wasm.simulateKeyUp(keystroke)
  }

  simulateClick(x: number, y: number, button?: number, modifiers?: string): void {
    this.#wasm.simulateClick(x, y, button, modifiers)
  }

  simulateMouseDown(x: number, y: number, button?: number, modifiers?: string): void {
    this.#wasm.simulateMouseDown(x, y, button, modifiers)
  }

  simulateMouseUp(x: number, y: number, button?: number, modifiers?: string): void {
    this.#wasm.simulateMouseUp(x, y, button, modifiers)
  }

  simulateMouseMove(x: number, y: number, pressedButton?: number, modifiers?: string): void {
    this.#wasm.simulateMouseMove(x, y, pressedButton, modifiers)
  }

  simulateScrollWheel(
    x: number,
    y: number,
    deltaX: number,
    deltaY: number,
    modifiers?: string,
  ): void {
    this.#wasm.simulateScrollWheel(x, y, deltaX, deltaY, modifiers)
  }

  clockPause(): number {
    return this.#wasm.clockPause()
  }

  clockSet(nowMs: number): number {
    return this.#wasm.clockSet(nowMs)
  }

  clockFastForward(deltaMs: number): number {
    return this.#wasm.clockFastForward(deltaMs)
  }

  clockResume(): number {
    return this.#wasm.clockResume()
  }

  setDebugFrameOverlay(mode: DebugFrameOverlayMode): string {
    return this.#wasm.setDebugFrameOverlay(mode)
  }

  cycleDebugFrameOverlay(): string {
    return this.#wasm.cycleDebugFrameOverlay()
  }

  getDebugFrameOverlay(): string {
    return this.#wasm.getDebugFrameOverlay()
  }

  resetDebugFrameOverlayStats(): void {
    this.#wasm.resetDebugFrameOverlayStats()
  }

  timelineGetState(): TimelineState {
    return parseJson(this.#wasm.timelineGetStateJson())
  }

  timelinePlay(): TimelineState {
    return parseJson(this.#wasm.timelinePlayJson())
  }

  timelinePause(): TimelineState {
    return parseJson(this.#wasm.timelinePauseJson())
  }

  timelineSeek(currentTimeMs: number): TimelineState {
    return parseJson(this.#wasm.timelineSeekJson(currentTimeMs))
  }

  timelineSetPlaybackRate(playbackRate: number): TimelineState {
    return parseJson(this.#wasm.timelineSetPlaybackRateJson(playbackRate))
  }

  configureAudio(sampleRate: number, channels: number, capacityFrames?: number): AudioBufferState {
    return parseJson(this.#wasm.configureAudioJson(sampleRate, channels, capacityFrames))
  }

  enqueueAudioFrames(samples: Float32Array): AudioBufferState {
    return parseJson(this.#wasm.enqueueAudioFramesJson(samples))
  }

  dequeueAudioFrames(maxFrames: number): Float32Array {
    return this.#wasm.dequeueAudioFrames(maxFrames)
  }

  clearAudioFrames(): AudioBufferState {
    return parseJson(this.#wasm.clearAudioFramesJson())
  }

  getAudioBufferState(): AudioBufferState {
    return parseJson(this.#wasm.getAudioBufferStateJson())
  }

  #apply(mutations: unknown[][]): NativeNodeId[] {
    return Array.from(this.#wasm.applyBatch(JSON.stringify(mutations)))
  }

  #startEventLoop(): void {
    const drain = (): void => {
      this.#drainEvents()
      if (this.#wasm.isInitialized()) this.#eventFrame = requestAnimationFrame(drain)
      else this.#eventFrame = undefined
    }
    this.#eventFrame = requestAnimationFrame(drain)
  }

  #drainEvents(): void {
    const events = parseJson<EventPayload[]>(this.#wasm.drainEventsJson())
    for (const event of events) {
      handleGpuiEvent(event, this)
      this.#onEvent?.(event)
    }
  }
}

export function createWebNativeRenderer(
  onEvent?: (event: EventPayload) => void,
): WebNativeRenderer {
  return new WebNativeRenderer(onEvent)
}
