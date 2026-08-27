import type {
  AudioBufferState,
  DebugFrameOverlayStats,
  DebugFrameOverlayMode,
  EventPayload,
  GpuiElementType,
  HighlightMatch,
  NativeWindowInsets,
  NativeWindowOptions,
  StyleDesc,
  TimelineState,
  WindowSize,
} from "./types.js"

export type NativeNodeId = number
export type NativeEventCallback = (error: Error | null, event: EventPayload | null) => void

/** First-party mutation protocol implemented by the Rust `GpuiRenderer`. */
export interface NativeRenderer {
  createElement(id: NativeNodeId, elementType: string): void
  destroyElement(id: NativeNodeId): NativeNodeId[]
  appendChild(parentId: NativeNodeId, childId: NativeNodeId): void
  removeChild(parentId: NativeNodeId, childId: NativeNodeId): void
  insertBefore(parentId: NativeNodeId, childId: NativeNodeId, beforeId: NativeNodeId): void
  setStyle(id: NativeNodeId, style: string | StyleDesc | Record<string, unknown>): void
  setText(id: NativeNodeId, content: string): void
  setEventListener(id: NativeNodeId, eventType: string, hasHandler: boolean): void
  setRoot(id: NativeNodeId): void
  setCustomProp(
    id: NativeNodeId,
    key: string,
    value: string | object | number | boolean | null,
  ): void
  getCustomProp?(id: NativeNodeId, key: string): string | null
  commitMutations(): void
  applyBatch?(json: string): NativeNodeId[]

  init?(options?: NativeWindowOptions | null): void
  close?(): void
  isInitialized?(): boolean
  requiresTick?(): boolean
  tick?(): boolean
  getWindowSize?(): WindowSize
  getWindowInsets?(): NativeWindowInsets
  setWindowTitle?(title: string): void
  focusElement?(elementId: NativeNodeId): void
  blur?(): void
  getSelectedText?(): string | null
  clearSelection?(): void
  scrollTo?(elementId: NativeNodeId, x: number, y: number): void
  scrollToItem?(elementId: NativeNodeId, index: number): void
  getScrollOffset?(elementId: NativeNodeId): number[] | null
  setDebugFrameOverlay?(mode: DebugFrameOverlayMode): string
  cycleDebugFrameOverlay?(): string
  getDebugFrameOverlay?(): string
  resetDebugFrameOverlayStats?(): void
  getDebugFrameOverlayStats?(): DebugFrameOverlayStats
  getAutomationTree?(): string
  snapshotJson?(): string
  getElementBounds?(id: NativeNodeId): number[] | null
  getAllText?(): string[]
  getPaintedText?(): string[]
  getPaintedHighlights?(): HighlightMatch[]
  simulateKeystrokes?(keystrokes: string): void
  simulateKeyDown?(keystroke: string, isHeld?: boolean): void
  simulateKeyUp?(keystroke: string): void
  simulateClick?(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseDown?(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseUp?(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseMove?(x: number, y: number, pressedButton?: number, modifiers?: string): void
  simulateScrollWheel?(
    x: number,
    y: number,
    deltaX: number,
    deltaY: number,
    modifiers?: string,
  ): void
  clockPause?(): number
  clockSet?(nowMs: number): number
  clockFastForward?(deltaMs: number): number
  clockResume?(): number
  timelineGetState?(): TimelineState
  timelinePlay?(): TimelineState
  timelinePause?(): TimelineState
  timelineSeek?(currentTimeMs: number): TimelineState
  timelineSetPlaybackRate?(playbackRate: number): TimelineState
  configureAudio?(sampleRate: number, channels: number, capacityFrames?: number): AudioBufferState
  enqueueAudioFrames?(samples: Float32Array): AudioBufferState
  dequeueAudioFrames?(maxFrames: number): Float32Array
  clearAudioFrames?(): AudioBufferState
  getAudioBufferState?(): AudioBufferState
  captureScreenshot?(path: string): void
}

/** Backward-compatible name retained from the bootstrap API. */
export type NativeBridge = NativeRenderer

export interface NativeSnapshotNode {
  id: NativeNodeId
  type: GpuiElementType | string
  style: StyleDesc
  text: string | null
  events: Set<string>
  children: NativeNodeId[]
  parentId: NativeNodeId | null
  customProps: Record<string, unknown>
}

type BatchValue = string | number | boolean | object | null
type BatchTuple = [string, ...BatchValue[]]

function parseJsonObject(value: string | object): Record<string, unknown> {
  const parsed: unknown = typeof value === "string" ? JSON.parse(value) : value
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new TypeError("Expected a JSON object")
  }
  return parsed as Record<string, unknown>
}

/** A deterministic protocol implementation for unit tests and custom hosts. */
export class MemoryNativeRenderer implements NativeRenderer {
  readonly nodes = new Map<NativeNodeId, NativeSnapshotNode>()
  rootId: NativeNodeId | null = null
  commitCount = 0
  initialized = false
  windowSize: WindowSize = { width: 800, height: 600 }
  windowTitle = "gpui-vue"
  focusedElementId: NativeNodeId | null = null
  selectedText: string | null = null
  debugFrameOverlay: DebugFrameOverlayMode = "hidden"
  readonly scrollOffsets = new Map<NativeNodeId, [number, number]>()
  timelineState: TimelineState = {
    currentTimeMs: 0,
    playbackRate: 1,
    playing: true,
  }
  audioState: AudioBufferState = {
    sampleRate: 48_000,
    channels: 2,
    capacityFrames: 96_000,
    queuedFrames: 0,
    droppedFrames: 0,
  }
  #audioSamples: number[] = []

  constructor(private readonly eventCallback?: NativeEventCallback) {}

  createElement(id: NativeNodeId, elementType: string): void {
    if (this.nodes.has(id)) throw new Error(`Native node ${id} already exists`)
    this.nodes.set(id, {
      id,
      type: elementType,
      style: {},
      text: null,
      events: new Set(),
      children: [],
      parentId: null,
      customProps: {},
    })
  }

  destroyElement(id: NativeNodeId): NativeNodeId[] {
    const node = this.#node(id)
    if (node.parentId !== null) {
      const parent = this.#node(node.parentId)
      parent.children = parent.children.filter((childId) => childId !== id)
    }

    const destroyed: NativeNodeId[] = []
    const visit = (nodeId: NativeNodeId): void => {
      const current = this.#node(nodeId)
      for (const child of current.children) visit(child)
      this.nodes.delete(nodeId)
      this.scrollOffsets.delete(nodeId)
      destroyed.push(nodeId)
    }
    visit(id)
    if (this.rootId === id) this.rootId = null
    return destroyed
  }

  appendChild(parentId: NativeNodeId, childId: NativeNodeId): void {
    this.#insert(parentId, childId, null)
  }

  removeChild(parentId: NativeNodeId, childId: NativeNodeId): void {
    const parent = this.#node(parentId)
    const child = this.#node(childId)
    if (child.parentId !== parentId) return
    parent.children = parent.children.filter((id) => id !== childId)
    child.parentId = null
  }

  insertBefore(parentId: NativeNodeId, childId: NativeNodeId, beforeId: NativeNodeId): void {
    this.#insert(parentId, childId, beforeId)
  }

  setStyle(id: NativeNodeId, style: string | StyleDesc | Record<string, unknown>): void {
    this.#node(id).style = parseJsonObject(style) as StyleDesc
  }

  setText(id: NativeNodeId, content: string): void {
    this.#node(id).text = content
  }

  setEventListener(id: NativeNodeId, eventType: string, hasHandler: boolean): void {
    const events = this.#node(id).events
    if (hasHandler) events.add(eventType)
    else events.delete(eventType)
  }

  setRoot(id: NativeNodeId): void {
    this.#node(id)
    this.rootId = id
  }

  setCustomProp(
    id: NativeNodeId,
    key: string,
    value: string | object | number | boolean | null,
  ): void {
    const decoded: unknown = typeof value === "string" ? JSON.parse(value) : value
    this.#setCustomPropValue(id, key, decoded)
  }

  getCustomProp(id: NativeNodeId, key: string): string | null {
    const value = this.#node(id).customProps[key]
    return value === undefined ? null : JSON.stringify(value)
  }

  commitMutations(): void {
    this.commitCount += 1
  }

  applyBatch(json: string): NativeNodeId[] {
    const operations = JSON.parse(json) as BatchTuple[]
    const staged = this.#cloneTree()
    const destroyed = staged.#applyOperations(operations)

    this.nodes.clear()
    for (const [id, node] of staged.nodes) this.nodes.set(id, node)
    this.scrollOffsets.clear()
    for (const [id, offset] of staged.scrollOffsets) this.scrollOffsets.set(id, offset)
    this.rootId = staged.rootId
    this.commitCount += 1
    return destroyed
  }

  #applyOperations(operations: BatchTuple[]): NativeNodeId[] {
    const destroyed: NativeNodeId[] = []

    for (const [operation, ...args] of operations) {
      switch (operation) {
        case "createElement":
          this.createElement(Number(args[0]), String(args[1]))
          break
        case "destroyElement":
          destroyed.push(...this.destroyElement(Number(args[0])))
          break
        case "appendChild":
          this.appendChild(Number(args[0]), Number(args[1]))
          break
        case "removeChild":
          this.removeChild(Number(args[0]), Number(args[1]))
          break
        case "insertBefore":
          this.insertBefore(Number(args[0]), Number(args[1]), Number(args[2]))
          break
        case "setStyle":
          this.setStyle(Number(args[0]), args[1] as string | object)
          break
        case "setText":
          this.setText(Number(args[0]), String(args[1]))
          break
        case "setEventListener":
          this.setEventListener(Number(args[0]), String(args[1]), Boolean(args[2]))
          break
        case "setRoot":
          this.setRoot(Number(args[0]))
          break
        case "setCustomProp":
          this.setCustomProp(
            Number(args[0]),
            String(args[1]),
            args[2] as string | object | number | boolean | null,
          )
          break
        case "setCustomPropValue":
          this.#setCustomPropValue(Number(args[0]), String(args[1]), args[2])
          break
        default:
          throw new Error(`Unknown native batch operation: ${operation}`)
      }
    }
    return destroyed
  }

  init(options: NativeWindowOptions | null = null): void {
    this.initialized = true
    if (options?.width !== undefined) this.windowSize.width = options.width
    if (options?.height !== undefined) this.windowSize.height = options.height
    if (options?.title !== undefined) this.windowTitle = options.title
  }

  close(): void {
    this.initialized = false
    this.focusedElementId = null
  }

  isInitialized(): boolean {
    return this.initialized
  }

  requiresTick(): boolean {
    return false
  }

  tick(): boolean {
    return this.initialized
  }

  getWindowSize(): WindowSize {
    return { ...this.windowSize }
  }

  getWindowInsets(): NativeWindowInsets {
    const zero = { top: 0, right: 0, bottom: 0, left: 0 }
    return { safeArea: { ...zero }, ime: { ...zero }, effective: { ...zero } }
  }

  setWindowTitle(title: string): void {
    this.windowTitle = title
  }

  focusElement(elementId: NativeNodeId): void {
    this.#node(elementId)
    this.focusedElementId = elementId
  }

  blur(): void {
    this.focusedElementId = null
  }

  getSelectedText(): string | null {
    return this.selectedText
  }

  clearSelection(): void {
    this.selectedText = null
  }

  scrollTo(elementId: NativeNodeId, x: number, y: number): void {
    this.#node(elementId)
    this.scrollOffsets.set(elementId, [x, y])
  }

  scrollToItem(elementId: NativeNodeId, index: number): void {
    this.#node(elementId)
    this.scrollOffsets.set(elementId, [0, -index])
  }

  getScrollOffset(elementId: NativeNodeId): number[] | null {
    const offset = this.scrollOffsets.get(elementId)
    return offset === undefined ? null : [...offset]
  }

  setDebugFrameOverlay(mode: DebugFrameOverlayMode): string {
    this.debugFrameOverlay = mode
    return mode
  }

  cycleDebugFrameOverlay(): string {
    const modes: DebugFrameOverlayMode[] = ["hidden", "minimal", "full"]
    const current = modes.indexOf(this.debugFrameOverlay)
    this.debugFrameOverlay = modes[(current + 1) % modes.length] ?? "hidden"
    return this.debugFrameOverlay
  }

  getDebugFrameOverlay(): string {
    return this.debugFrameOverlay
  }

  resetDebugFrameOverlayStats(): void {}

  getDebugFrameOverlayStats(): DebugFrameOverlayStats {
    return { frames: 0, samples: 0 }
  }

  clockPause(): number {
    return this.timelinePause().currentTimeMs
  }

  clockSet(nowMs: number): number {
    this.timelineState.currentTimeMs = Math.max(0, nowMs)
    this.timelineState.playing = false
    return this.timelineState.currentTimeMs
  }

  clockFastForward(deltaMs: number): number {
    return this.clockSet(this.timelineState.currentTimeMs + Math.max(0, deltaMs))
  }

  clockResume(): number {
    return this.timelinePlay().currentTimeMs
  }

  timelineGetState(): TimelineState {
    return { ...this.timelineState }
  }

  timelinePlay(): TimelineState {
    this.timelineState.playing = true
    return this.timelineGetState()
  }

  timelinePause(): TimelineState {
    this.timelineState.playing = false
    return this.timelineGetState()
  }

  timelineSeek(currentTimeMs: number): TimelineState {
    if (!Number.isFinite(currentTimeMs) || currentTimeMs < 0) {
      throw new RangeError("timeline currentTimeMs must be finite and non-negative")
    }
    this.timelineState.currentTimeMs = currentTimeMs
    return this.timelineGetState()
  }

  timelineSetPlaybackRate(playbackRate: number): TimelineState {
    if (!Number.isFinite(playbackRate) || playbackRate <= 0 || playbackRate > 100) {
      throw new RangeError("timeline playbackRate must be between 0 and 100")
    }
    this.timelineState.playbackRate = playbackRate
    return this.timelineGetState()
  }

  configureAudio(
    sampleRate: number,
    channels: number,
    capacityFrames = sampleRate * 2,
  ): AudioBufferState {
    this.audioState = {
      sampleRate,
      channels,
      capacityFrames,
      queuedFrames: 0,
      droppedFrames: 0,
    }
    this.#audioSamples = []
    return this.getAudioBufferState()
  }

  enqueueAudioFrames(samples: Float32Array): AudioBufferState {
    if (samples.length % this.audioState.channels !== 0) {
      throw new RangeError("audio samples must contain complete interleaved frames")
    }
    this.#audioSamples.push(...samples)
    const capacity = this.audioState.capacityFrames * this.audioState.channels
    if (this.#audioSamples.length > capacity) {
      const dropped = this.#audioSamples.length - capacity
      this.#audioSamples.splice(0, dropped)
      this.audioState.droppedFrames += dropped / this.audioState.channels
    }
    this.audioState.queuedFrames = this.#audioSamples.length / this.audioState.channels
    return this.getAudioBufferState()
  }

  dequeueAudioFrames(maxFrames: number): Float32Array {
    const count = Math.min(this.#audioSamples.length, maxFrames * this.audioState.channels)
    const samples = new Float32Array(this.#audioSamples.splice(0, count))
    this.audioState.queuedFrames = this.#audioSamples.length / this.audioState.channels
    return samples
  }

  clearAudioFrames(): AudioBufferState {
    this.#audioSamples = []
    this.audioState.queuedFrames = 0
    return this.getAudioBufferState()
  }

  getAudioBufferState(): AudioBufferState {
    return { ...this.audioState }
  }

  getAllText(): string[] {
    if (this.rootId === null) return []
    const text: string[] = []
    const visit = (id: NativeNodeId): void => {
      const node = this.#node(id)
      if (node.text !== null) text.push(node.text)
      for (const child of node.children) visit(child)
    }
    visit(this.rootId)
    return text
  }

  getPaintedText(): string[] {
    return this.getAllText()
  }

  getPaintedHighlights(): HighlightMatch[] {
    return []
  }

  getTreeJson(): string {
    return JSON.stringify({
      rootId: this.rootId,
      elements: [...this.nodes.values()].map((node) => ({
        ...node,
        events: [...node.events],
      })),
    })
  }

  getAutomationTree(): string {
    const nodes: Record<string, Record<string, unknown>> = {}
    for (const node of this.nodes.values()) {
      nodes[String(node.id)] = {
        id: node.id,
        kind: node.type === "text" ? "text" : "element",
        ...(node.type === "text" ? { text: node.text ?? "" } : { tag: node.type }),
        parent: node.parentId,
        children: [...node.children],
        style: { ...node.style },
        events: [...node.events],
        customProps: { ...node.customProps },
      }
    }
    return JSON.stringify({ rootId: this.rootId, revision: this.commitCount, nodes })
  }

  snapshotJson(): string {
    return this.getAutomationTree()
  }

  snapshot(): NativeSnapshotNode[] {
    return [...this.nodes.values()].map((node) => ({
      ...node,
      style: { ...node.style },
      events: new Set(node.events),
      children: [...node.children],
      customProps: { ...node.customProps },
    }))
  }

  emit(event: EventPayload): void {
    this.eventCallback?.(null, event)
  }

  #cloneTree(): MemoryNativeRenderer {
    const clone = new MemoryNativeRenderer()
    clone.rootId = this.rootId
    for (const [id, node] of this.nodes) {
      clone.nodes.set(id, {
        ...node,
        style: { ...node.style },
        events: new Set(node.events),
        children: [...node.children],
        customProps: { ...node.customProps },
      })
    }
    for (const [id, offset] of this.scrollOffsets) clone.scrollOffsets.set(id, [...offset])
    return clone
  }

  #setCustomPropValue(id: NativeNodeId, key: string, value: unknown): void {
    const props = this.#node(id).customProps
    if (value === null || value === undefined) delete props[key]
    else props[key] = value
  }

  #insert(parentId: NativeNodeId, childId: NativeNodeId, beforeId: NativeNodeId | null): void {
    const parent = this.#node(parentId)
    const child = this.#node(childId)
    if (beforeId === childId && child.parentId === parentId) return
    if (beforeId !== null && !parent.children.includes(beforeId)) {
      throw new Error(`Anchor ${beforeId} is not a child of ${parentId}`)
    }

    let ancestor: NativeSnapshotNode | undefined = parent
    while (ancestor !== undefined) {
      if (ancestor.id === childId) {
        throw new Error(`Inserting ${childId} below ${parentId} would create a cycle`)
      }
      ancestor = ancestor.parentId === null ? undefined : this.nodes.get(ancestor.parentId)
    }

    if (child.parentId !== null) {
      const oldParent = this.#node(child.parentId)
      oldParent.children = oldParent.children.filter((id) => id !== childId)
    }
    const index = beforeId === null ? parent.children.length : parent.children.indexOf(beforeId)
    parent.children.splice(index, 0, childId)
    child.parentId = parentId
  }

  #node(id: NativeNodeId): NativeSnapshotNode {
    const node = this.nodes.get(id)
    if (node === undefined) throw new Error(`Unknown native node ${id}`)
    return node
  }
}

/** Backward-compatible class export retained from the bootstrap. */
export const MemoryNativeBridge = MemoryNativeRenderer
