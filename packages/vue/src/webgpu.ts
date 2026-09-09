import { createGPUCanvas, type GPUCanvasOptions } from "@gpui-native/runtime/webgpu"
import { onScopeDispose } from "@vue/runtime-core"

import { useGpuiRequired } from "./context.js"
export * from "@gpui-native/runtime/webgpu"

/** The setup scope owns this source; use resize() when its dimensions change. */
export function useGPUCanvas(options: Omit<GPUCanvasOptions, "renderer"> = {}) {
  const canvas = createGPUCanvas({ ...options, renderer: useGpuiRequired() })
  onScopeDispose(() => canvas.destroy())
  return canvas
}
