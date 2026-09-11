import { createGPUCanvas, type GPUCanvas, type GPUCanvasOptions } from "@gpui-native/runtime/webgpu"

import { useGpuiRequired } from "./context.svelte.js"
export * from "@gpui-native/runtime/webgpu"
/** Getter options keep dimensions reactive. Resources are owned by the Svelte effect. */
export function useGPUCanvas(read: () => Omit<GPUCanvasOptions, "renderer"> = () => ({})) {
  const renderer = useGpuiRequired()
  let current = $state.raw<GPUCanvas | null>(null)
  const maxFramesInFlight = $derived(read().maxFramesInFlight ?? 2)
  const presentation = $derived(read().presentation ?? "direct")
  $effect(() => {
    const canvas = createGPUCanvas({ renderer, maxFramesInFlight, presentation })
    current = canvas
    return () => {
      canvas.destroy()
      current = null
    }
  })
  $effect(() => {
    const { width = 300, height = 150 } = read()
    current?.resize(width, height)
  })
  return {
    get current() {
      return current?.destroyed ? null : current
    },
  }
}
