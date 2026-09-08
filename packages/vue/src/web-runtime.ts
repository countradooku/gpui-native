import { createGpuiRuntime } from "./runtime-core.js"
import { createWebNativeRenderer } from "./web-native.js"

export { startFrameLoop } from "./runtime-core.js"
export type { FrameLoop, GpuiRoot, GpuiWindowRoot, RenderOptions } from "./runtime-core.js"

const runtime = createGpuiRuntime(createWebNativeRenderer, "__gpuiVueWebRenderHost")

export const { render, createWindow, resetRender } = runtime
