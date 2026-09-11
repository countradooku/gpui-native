declare module "@gpui-native/wasm" {
  export default function init(): Promise<WebAssembly.Exports>

  export class WebGpuiRenderer {
    constructor()
    canvasPresentation(): "shared-wgpu"
    canvasGpuDevice(): GPUDevice | undefined
    createCanvasTexture(descriptor: string): { texture: GPUTexture; handle: number }
    releaseCanvasTexture(handle: number): void
    resetCanvasSource(id: number): void
    presentCanvasTexture(id: number, handle: number, opaque: boolean): Promise<boolean>
    createCanvasSource(): number
    presentCanvasFrame(
      id: number,
      width: number,
      height: number,
      stride: number,
      pixels: Uint8Array,
      bgra: boolean,
      opaque: boolean,
    ): void
    destroyCanvasSource(id: number): void
    init(optionsJson: string): void
    applyBatch(mutationsJson: string): Float64Array
    commitMutations(): void
    getCustomProp(id: number, key: string): string | undefined
    isInitialized(): boolean
    requiresTick(): boolean
    supportsWindowEvents(): boolean
    tick(): boolean
    close(): void
    getWindowSizeJson(): string
    getWindowInsetsJson(): string
    activateWindow(): void
    setWindowTitle(title: string): void
    focusElement(elementId: number): void
    focusNext(): void
    focusPrevious(): void
    setWindowKeyEvents(keyDown: boolean, keyUp: boolean, eventId: number): void
    blur(): void
    getSelectedText(): string | undefined
    clearSelection(): void
    scrollTo(elementId: number, x: number, y: number): void
    scrollToItem(elementId: number, index: number, offsetInItem?: number): void
    getListScrollTopJson(elementId: number): string
    getScrollOffsetJson(elementId: number): string
    getAutomationTree(): string
    getElementBoundsJson(elementId: number): string
    getAllTextJson(): string
    getPaintedTextJson(): string
    getPaintedHighlightsJson(): string
    simulateKeystrokes(keystrokes: string): void
    simulateKeyDown(keystroke: string, isHeld?: boolean): void
    simulateKeyUp(keystroke: string): void
    simulateClick(x: number, y: number, button?: number, modifiers?: string): void
    simulateMouseDown(x: number, y: number, button?: number, modifiers?: string): void
    simulateMouseUp(x: number, y: number, button?: number, modifiers?: string): void
    simulateMouseMove(x: number, y: number, pressedButton?: number, modifiers?: string): void
    simulateScrollWheel(
      x: number,
      y: number,
      deltaX: number,
      deltaY: number,
      modifiers?: string,
    ): void
    clockPause(): number
    clockSet(nowMs: number): number
    clockFastForward(deltaMs: number): number
    clockResume(): number
    setDebugFrameOverlay(mode: string): string
    cycleDebugFrameOverlay(): string
    getDebugFrameOverlay(): string
    resetDebugFrameOverlayStats(): void
    timelineGetStateJson(): string
    timelinePlayJson(): string
    timelinePauseJson(): string
    timelineSeekJson(currentTimeMs: number): string
    timelineSetPlaybackRateJson(rate: number): string
    configureAudioJson(sampleRate: number, channels: number, capacityFrames?: number): string
    enqueueAudioFramesJson(samples: Float32Array): string
    dequeueAudioFrames(maxFrames: number): Float32Array
    clearAudioFramesJson(): string
    getAudioBufferStateJson(): string
    drainEventsJson(): string
  }
}
