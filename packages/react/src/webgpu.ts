import { createGPUCanvas, type GPUCanvas, type GPUCanvasOptions } from "@gpui-native/runtime/webgpu"
import { useEffect, useState } from "react"

import { useGpuiRequired } from "./context.js"
export * from "@gpui-native/runtime/webgpu"

/** Created after commit; disposed on unmount, including React StrictMode replay. */
export function useGPUCanvas(options: Omit<GPUCanvasOptions, "renderer"> = {}): GPUCanvas | null {
  const renderer = useGpuiRequired()
  const [canvas, setCanvas] = useState<GPUCanvas | null>(null)
  const { width = 300, height = 150, maxFramesInFlight = 2, presentation = "direct" } = options
  useEffect(() => {
    const next = createGPUCanvas({ renderer, maxFramesInFlight, presentation })
    setCanvas(next)
    return () => next.destroy()
  }, [renderer, maxFramesInFlight, presentation])
  useEffect(() => {
    if (canvas && !canvas.destroyed) canvas.resize(width, height)
  }, [canvas, width, height])
  return canvas?.destroyed ? null : canvas
}
