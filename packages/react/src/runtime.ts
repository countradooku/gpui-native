import { createNativeRenderer } from "@gpui-native/runtime/native-addon"

import { createGpuiRuntime } from "./runtime-core.js"
export * from "./runtime-core.js"
export const { render, createWindow, resetRender } = createGpuiRuntime(createNativeRenderer)
