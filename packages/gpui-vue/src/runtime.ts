import { createNativeRenderer } from "./native-addon.js"
import { createGpuiRuntime } from "./runtime-core.js"

export { startFrameLoop } from "./runtime-core.js"
export type { FrameLoop, GpuiRoot, GpuiWindowRoot, RenderOptions } from "./runtime-core.js"

const runtime = createGpuiRuntime(createNativeRenderer, "__gpuiVueRenderHost")

export const { render, createWindow, resetRender } = runtime
