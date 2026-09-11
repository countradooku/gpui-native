import { GpuiAudioFrames } from "@gpui-native/runtime/audio"
import { subscribeRendererEvent } from "@gpui-native/runtime/events"
import type { NativeRenderer } from "@gpui-native/runtime/native"
import { GpuiTimeline } from "@gpui-native/runtime/timeline"

import { getContext, onDestroy, onMount } from "./public.js"
import { GpuiRendererKey } from "./renderer.js"
import type {
  DebugFrameOverlayMode,
  DebugFrameOverlayStats,
  HighlightMatch,
  EdgeInsets,
  NativeWindowInsets,
  WindowSize,
} from "./types.js"
import type { ElementRef } from "./types.js"
import { subscribeWindowKey } from "./window-events.js"

export function useGpui() {
  return { renderer: getContext<NativeRenderer | undefined>(GpuiRendererKey) ?? null }
}
export function useGpuiRequired(): NativeRenderer {
  const { renderer } = useGpui()
  if (!renderer)
    throw new Error("GPUI context is only available inside a Svelte native application")
  return renderer
}
export function useGpuiTimeline() {
  return new GpuiTimeline(useGpuiRequired())
}
export function useGpuiAudioFrames() {
  return new GpuiAudioFrames(useGpuiRequired())
}
export function useElementRef() {
  let current = $state.raw<ElementRef | null>(null)
  return {
    get current() {
      return current
    },
    set current(value) {
      current = value
    },
  }
}
export interface WindowPollingOptions {
  intervalMs?: number | false
}
export interface WindowInsets extends NativeWindowInsets {
  keyboardTop: number
  keyboardVisible: boolean
  visibleHeight: number
}
const ZERO: EdgeInsets = { top: 0, right: 0, bottom: 0, left: 0 }
function readSize(renderer: NativeRenderer | null): WindowSize {
  try {
    return renderer?.getWindowSize?.() ?? { width: 800, height: 600 }
  } catch {
    return { width: 800, height: 600 }
  } // Web windows finish opening asynchronously.
}
function readInsets(renderer: NativeRenderer | null): WindowInsets {
  const size = readSize(renderer)
  let insets: NativeWindowInsets = { safeArea: ZERO, ime: ZERO, effective: ZERO }
  try {
    insets = renderer?.getWindowInsets?.() ?? insets
  } catch {
    /* Window is opening. */
  }
  return {
    ...insets,
    keyboardTop: size.height - insets.ime.bottom,
    keyboardVisible: insets.ime.bottom > 0,
    visibleHeight: size.height - insets.effective.top - insets.effective.bottom,
  }
}
function windowValue<T extends object>(
  read: (renderer: NativeRenderer | null) => T,
  options: WindowPollingOptions,
): T {
  const { renderer } = useGpui()
  let value = $state.raw(read(renderer))
  onMount(() => {
    const update = () => {
      const next = read(renderer)
      if (JSON.stringify(next) !== JSON.stringify(value)) value = next
    }
    update()
    if (options.intervalMs === false) return
    if (renderer?.supportsWindowEvents?.())
      return subscribeRendererEvent(renderer, "windowResize", update)
    const timer = setInterval(update, Math.max(16, options.intervalMs ?? 100))
    return () => clearInterval(timer)
  })
  return new Proxy({} as T, {
    get: (_target, key) => value[key as keyof T],
    ownKeys: () => Reflect.ownKeys(value),
    getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
  })
}
export function useWindowSize(options: WindowPollingOptions = {}): WindowSize {
  return windowValue(readSize, options)
}
export function useWindowInsets(options: WindowPollingOptions = {}) {
  return windowValue(readInsets, options)
}
const elementId = (element: number | ElementRef) =>
  typeof element === "number" ? element : element.id
export function useGpuiWindow(): GpuiWindowControls {
  const renderer = useGpuiRequired()
  return {
    size: () => readSize(renderer),
    insets: () => renderer.getWindowInsets?.() ?? { safeArea: ZERO, ime: ZERO, effective: ZERO },
    setTitle: (title: string) => renderer.setWindowTitle?.(title),
    activate: () => renderer.activateWindow?.(),
    focus: (element: number | ElementRef) => renderer.focusElement?.(elementId(element)),
    blur: () => renderer.blur?.(),
    selectedText: () => renderer.getSelectedText?.() ?? null,
    clearSelection: () => renderer.clearSelection?.(),
    scrollTo: (element: number | ElementRef, x: number, y: number) =>
      renderer.scrollTo?.(elementId(element), x, y),
    scrollToItem: (element: number | ElementRef, index: number, offset?: number) =>
      renderer.scrollToItem?.(elementId(element), index, offset),
    scrollOffset: (element: number | ElementRef) =>
      renderer.getScrollOffset?.(elementId(element)) ?? null,
    listScrollTop: (element: number | ElementRef) =>
      renderer.getListScrollTop?.(elementId(element)) ?? null,
    paintedHighlights: () => renderer.getPaintedHighlights?.() ?? [],
    setDebugOverlay: (mode: DebugFrameOverlayMode) => renderer.setDebugFrameOverlay?.(mode),
    cycleDebugOverlay: () => renderer.cycleDebugFrameOverlay?.(),
    debugOverlayStats: () => renderer.getDebugFrameOverlayStats?.(),
  }
}
export interface GpuiWindowControls {
  size(): WindowSize
  insets(): NativeWindowInsets
  setTitle(title: string): void
  activate(): void
  focus(element: number | ElementRef): void
  blur(): void
  selectedText(): string | null
  clearSelection(): void
  scrollTo(element: number | ElementRef, x: number, y: number): void
  scrollToItem(element: number | ElementRef, index: number, offsetInItem?: number): void
  scrollOffset(element: number | ElementRef): readonly number[] | null
  listScrollTop(element: number | ElementRef): readonly number[] | null
  paintedHighlights(): readonly HighlightMatch[]
  setDebugOverlay(mode: DebugFrameOverlayMode): string | undefined
  cycleDebugOverlay(): string | undefined
  debugOverlayStats(): DebugFrameOverlayStats | undefined
}

export function useWindowEvent(
  type: "windowKeyDown" | "windowKeyUp",
  handler: Parameters<typeof subscribeRendererEvent>[2],
) {
  const renderer = useGpuiRequired()
  onDestroy(subscribeWindowKey(renderer, type, handler))
}
