import { GpuiAudioFrames } from "@gpui-native/runtime/audio"
import { subscribeRendererEvent } from "@gpui-native/runtime/events"
import type { NativeRenderer } from "@gpui-native/runtime/native"
import type { GpuiPublicInstance } from "@gpui-native/runtime/nodes"
import { GpuiTimeline } from "@gpui-native/runtime/timeline"
import type {
  DebugFrameOverlayMode,
  DebugFrameOverlayStats,
  EdgeInsets,
  HighlightMatch,
  NativeWindowInsets,
  WindowSize,
} from "@gpui-native/runtime/types"
import { createContext, useContext, useEffect, useMemo, useRef, useState } from "react"
export const GpuiRendererContext = createContext<NativeRenderer | null>(null)
export function useGpui() {
  return { renderer: useContext(GpuiRendererContext) }
}
export function useGpuiRequired(): NativeRenderer {
  const { renderer } = useGpui()
  if (!renderer) throw new Error("useGpuiRequired must be called inside a GPUI React application")
  return renderer
}
export function useElementRef() {
  return useRef<GpuiPublicInstance | null>(null)
}
export function useGpuiTimeline() {
  const renderer = useGpuiRequired()
  return useMemo(() => new GpuiTimeline(renderer), [renderer])
}
export function useGpuiAudioFrames() {
  const renderer = useGpuiRequired()
  return useMemo(() => new GpuiAudioFrames(renderer), [renderer])
}
const ZERO_EDGES: EdgeInsets = { top: 0, right: 0, bottom: 0, left: 0 }
export interface WindowPollingOptions {
  intervalMs?: number | false
}
export interface WindowInsets extends NativeWindowInsets {
  keyboardTop: number
  keyboardVisible: boolean
  visibleHeight: number
}
function useWindowValue<T>(
  read: (renderer: NativeRenderer | null) => T,
  options: WindowPollingOptions,
): T {
  const { renderer } = useGpui()
  const [value, setValue] = useState(() => read(renderer))
  useEffect(() => {
    const update = () => {
      const next = read(renderer)
      setValue((old) => (JSON.stringify(old) === JSON.stringify(next) ? old : next))
    }
    update()
    if (options.intervalMs === false) return
    if (renderer?.supportsWindowEvents?.())
      return subscribeRendererEvent(renderer, "windowResize", update)
    const timer = setInterval(update, Math.max(16, options.intervalMs ?? 100))
    return () => clearInterval(timer)
  }, [renderer, options.intervalMs, read])
  return value
}
function readSize(renderer: NativeRenderer | null): WindowSize {
  try {
    return renderer?.getWindowSize?.() ?? { width: 800, height: 600 }
  } catch {
    return { width: 800, height: 600 }
  }
}
function readInsets(renderer: NativeRenderer | null): WindowInsets {
  const size = readSize(renderer)
  let insets: NativeWindowInsets = { safeArea: ZERO_EDGES, ime: ZERO_EDGES, effective: ZERO_EDGES }
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
export function useWindowSize(options: WindowPollingOptions = {}): WindowSize {
  return useWindowValue(readSize, options)
}
export function useWindowInsets(options: WindowPollingOptions = {}): WindowInsets {
  return useWindowValue(readInsets, options)
}
function elementId(element: number | GpuiPublicInstance) {
  return typeof element === "number" ? element : element.id
}
export interface GpuiWindowControls {
  size(): WindowSize
  insets(): NativeWindowInsets
  setTitle(title: string): void
  activate(): void
  focus(element: number | GpuiPublicInstance): void
  blur(): void
  selectedText(): string | null
  clearSelection(): void
  scrollTo(element: number | GpuiPublicInstance, x: number, y: number): void
  scrollToItem(element: number | GpuiPublicInstance, index: number, offsetInItem?: number): void
  scrollOffset(element: number | GpuiPublicInstance): readonly number[] | null
  listScrollTop(element: number | GpuiPublicInstance): readonly number[] | null
  paintedHighlights(): readonly HighlightMatch[]
  setDebugOverlay(mode: DebugFrameOverlayMode): string | undefined
  cycleDebugOverlay(): string | undefined
  debugOverlayStats(): DebugFrameOverlayStats | undefined
}

/** Imperative controls for the GPUI window hosting the current React app. */
export function useGpuiWindow(): GpuiWindowControls {
  const renderer = useGpuiRequired()
  return {
    size: () => renderer.getWindowSize?.() ?? { width: 800, height: 600 },
    insets: () =>
      renderer.getWindowInsets?.() ?? {
        safeArea: { ...ZERO_EDGES },
        ime: { ...ZERO_EDGES },
        effective: { ...ZERO_EDGES },
      },
    setTitle: (title) => renderer.setWindowTitle?.(title),
    activate: () => renderer.activateWindow?.(),
    focus: (element) => renderer.focusElement?.(elementId(element)),
    blur: () => renderer.blur?.(),
    selectedText: () => renderer.getSelectedText?.() ?? null,
    clearSelection: () => renderer.clearSelection?.(),
    scrollTo: (element, x, y) => renderer.scrollTo?.(elementId(element), x, y),
    scrollToItem: (element, index, offsetInItem) =>
      renderer.scrollToItem?.(elementId(element), index, offsetInItem),
    scrollOffset: (element) => renderer.getScrollOffset?.(elementId(element)) ?? null,
    listScrollTop: (element) => renderer.getListScrollTop?.(elementId(element)) ?? null,
    paintedHighlights: () => renderer.getPaintedHighlights?.() ?? [],
    setDebugOverlay: (mode) => renderer.setDebugFrameOverlay?.(mode),
    cycleDebugOverlay: () => renderer.cycleDebugFrameOverlay?.(),
    debugOverlayStats: () => renderer.getDebugFrameOverlayStats?.(),
  }
}
