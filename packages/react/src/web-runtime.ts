import { createWebNativeRenderer } from "@gpui-native/runtime/web-native"

import { createGpuiRuntime } from "./runtime-core.js"
export * from "./runtime-core.js"
export const { render, createWindow, resetRender } = createGpuiRuntime(createWebNativeRenderer)
