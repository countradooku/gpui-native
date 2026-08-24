import {
  inject,
  onMounted,
  readonly,
  ref,
  shallowRef,
  type InjectionKey,
  type Ref,
  type ShallowRef,
} from "@vue/runtime-core"

import { GpuiAudioFrames } from "./audio.js"
import type { NativeRenderer } from "./native.js"
import type { GpuiPublicInstance } from "./nodes.js"
import { GpuiTimeline } from "./timeline.js"
import type { DebugFrameOverlayMode, WindowSize } from "./types.js"

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

export function useWindowSize(): Readonly<Ref<WindowSize>> {
  const renderer = inject(GpuiRendererKey, null)
  const size = ref<WindowSize>({ width: 800, height: 600 })
  onMounted(() => {
    if (renderer?.getWindowSize === undefined) return
    try {
      size.value = renderer.getWindowSize()
    } catch {
      // The native window may still be opening during component mount.
    }
  })
  return readonly(size)
}

/** A template-ref type for native GPUI host elements. */
export function useElementRef(): ShallowRef<GpuiPublicInstance | null> {
  return shallowRef<GpuiPublicInstance | null>(null)
}

export interface GpuiWindowControls {
  size(): WindowSize
  setTitle(title: string): void
  focus(element: number | GpuiPublicInstance): void
  blur(): void
  selectedText(): string | null
  clearSelection(): void
  scrollTo(element: number | GpuiPublicInstance, x: number, y: number): void
  scrollToItem(element: number | GpuiPublicInstance, index: number): void
  scrollOffset(element: number | GpuiPublicInstance): readonly number[] | null
  setDebugOverlay(mode: DebugFrameOverlayMode): string | undefined
  cycleDebugOverlay(): string | undefined
}

/** Imperative controls for the GPUI window hosting the current Vue app. */
export function useGpuiWindow(): GpuiWindowControls {
  const renderer = useGpuiRequired()
  return {
    size: () => renderer.getWindowSize?.() ?? { width: 800, height: 600 },
    setTitle: (title) => renderer.setWindowTitle?.(title),
    focus: (element) => renderer.focusElement?.(elementId(element)),
    blur: () => renderer.blur?.(),
    selectedText: () => renderer.getSelectedText?.() ?? null,
    clearSelection: () => renderer.clearSelection?.(),
    scrollTo: (element, x, y) => renderer.scrollTo?.(elementId(element), x, y),
    scrollToItem: (element, index) => renderer.scrollToItem?.(elementId(element), index),
    scrollOffset: (element) => renderer.getScrollOffset?.(elementId(element)) ?? null,
    setDebugOverlay: (mode) => renderer.setDebugFrameOverlay?.(mode),
    cycleDebugOverlay: () => renderer.cycleDebugFrameOverlay?.(),
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
