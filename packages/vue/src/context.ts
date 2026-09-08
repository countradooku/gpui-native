import {
  inject,
  onMounted,
  onUnmounted,
  readonly,
  ref,
  shallowRef,
  type InjectionKey,
  type Ref,
  type ShallowRef,
} from "@vue/runtime-core"

import { GpuiAudioFrames } from "./audio.js"
import { subscribeRendererEvent } from "./events.js"
import type { NativeRenderer } from "./native.js"
import type { GpuiPublicInstance } from "./nodes.js"
import { GpuiTimeline } from "./timeline.js"
import type {
  DebugFrameOverlayMode,
  DebugFrameOverlayStats,
  EdgeInsets,
  HighlightMatch,
  NativeWindowInsets,
  WindowSize,
} from "./types.js"

export const GpuiRendererKey: InjectionKey<NativeRenderer> = Symbol("GpuiRenderer")

export interface GpuiContextValue {
  renderer: NativeRenderer | null
}

export function useGpui(): GpuiContextValue {
  return { renderer: inject(GpuiRendererKey, null) }
}

export function useGpuiRequired(): NativeRenderer {
  const renderer = inject(GpuiRendererKey, null)
  if (renderer === null) {
    throw new Error("useGpuiRequired must be called inside a gpui-vue application")
  }
  return renderer
}

export interface WindowPollingOptions {
  /** Fallback poll interval for renderers without window events. Set false for one read. */
  intervalMs?: number | false
}

const DEFAULT_WINDOW_SIZE: WindowSize = { width: 800, height: 600 }

function readWindowSize(renderer: NativeRenderer | null): WindowSize {
  try {
    const next = renderer?.getWindowSize?.()
    if (next !== undefined && next.width > 0 && next.height > 0) return next
  } catch {
    // The native window may still be opening.
  }
  return DEFAULT_WINDOW_SIZE
}

/** Current native window size, driven by events with polling as a compatibility fallback. */
export function useWindowSize(options: WindowPollingOptions = {}): Readonly<Ref<WindowSize>> {
  const renderer = inject(GpuiRendererKey, null)
  const size = ref<WindowSize>(readWindowSize(renderer))
  let timer: ReturnType<typeof setInterval> | undefined
  let unsubscribe: (() => void) | undefined
  onMounted(() => {
    const update = (): void => {
      const next = readWindowSize(renderer)
      if (next.width !== size.value.width || next.height !== size.value.height) size.value = next
    }
    update()
    const intervalMs = options.intervalMs ?? 100
    if (intervalMs === false) return
    if (renderer?.supportsWindowEvents?.() === true) {
      unsubscribe = subscribeRendererEvent(renderer, "windowResize", (event) => {
        if (
          event.width !== undefined &&
          event.height !== undefined &&
          (event.width !== size.value.width || event.height !== size.value.height)
        ) {
          size.value = { width: event.width, height: event.height }
        }
      })
    } else {
      timer = setInterval(update, Math.max(16, intervalMs))
    }
  })
  onUnmounted(() => {
    if (timer !== undefined) clearInterval(timer)
    unsubscribe?.()
  })
  return readonly(size)
}

export interface WindowInsets extends NativeWindowInsets {
  /** Y coordinate where unobscured content ends. */
  keyboardTop: number
  keyboardVisible: boolean
  visibleHeight: number
}

const ZERO_EDGES: EdgeInsets = { top: 0, right: 0, bottom: 0, left: 0 }

function readWindowInsets(renderer: NativeRenderer | null): WindowInsets {
  let size = DEFAULT_WINDOW_SIZE
  let insets: NativeWindowInsets = {
    safeArea: ZERO_EDGES,
    ime: ZERO_EDGES,
    effective: ZERO_EDGES,
  }
  try {
    size = renderer?.getWindowSize?.() ?? size
    insets = renderer?.getWindowInsets?.() ?? insets
  } catch {
    // The native window may still be opening.
  }
  return {
    ...insets,
    keyboardTop: size.height - insets.ime.bottom,
    keyboardVisible: insets.ime.bottom > 0,
    visibleHeight: size.height - insets.effective.top - insets.effective.bottom,
  }
}

function sameWindowInsets(a: WindowInsets, b: WindowInsets): boolean {
  const sameEdges = (left: EdgeInsets, right: EdgeInsets): boolean =>
    left.top === right.top &&
    left.right === right.right &&
    left.bottom === right.bottom &&
    left.left === right.left
  return (
    sameEdges(a.safeArea, b.safeArea) &&
    sameEdges(a.ime, b.ime) &&
    sameEdges(a.effective, b.effective) &&
    a.keyboardTop === b.keyboardTop &&
    a.keyboardVisible === b.keyboardVisible &&
    a.visibleHeight === b.visibleHeight
  )
}

/** Current safe-area and software-keyboard geometry, driven by window events when available. */
export function useWindowInsets(options: WindowPollingOptions = {}): Readonly<Ref<WindowInsets>> {
  const renderer = inject(GpuiRendererKey, null)
  const insets = ref<WindowInsets>(readWindowInsets(renderer))
  let timer: ReturnType<typeof setInterval> | undefined
  let unsubscribe: (() => void) | undefined
  onMounted(() => {
    const update = (): void => {
      const next = readWindowInsets(renderer)
      if (!sameWindowInsets(insets.value, next)) insets.value = next
    }
    update()
    const intervalMs = options.intervalMs ?? 100
    if (intervalMs === false) return
    if (renderer?.supportsWindowEvents?.() === true) {
      unsubscribe = subscribeRendererEvent(renderer, "windowResize", update)
    } else {
      timer = setInterval(update, Math.max(16, intervalMs))
    }
  })
  onUnmounted(() => {
    if (timer !== undefined) clearInterval(timer)
    unsubscribe?.()
  })
  return readonly(insets)
}

/** A template-ref type for native GPUI host elements. */
export function useElementRef(): ShallowRef<GpuiPublicInstance | null> {
  return shallowRef<GpuiPublicInstance | null>(null)
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

/** Imperative controls for the GPUI window hosting the current Vue app. */
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

/** Native playback controls shared by every motion element in this window. */
export function useGpuiTimeline(): GpuiTimeline {
  return new GpuiTimeline(useGpuiRequired())
}

/** Bounded native queue for interleaved decoded f32 PCM chunks. */
export function useGpuiAudioFrames(): GpuiAudioFrames {
  return new GpuiAudioFrames(useGpuiRequired())
}

function elementId(element: number | GpuiPublicInstance): number {
  return typeof element === "number" ? element : element.id
}
