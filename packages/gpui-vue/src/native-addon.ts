import { createRequire } from "node:module"

import type { GpuiRenderer as NativeGpuiRenderer } from "@gpui-vue/native"

import { handleGpuiEvent } from "./events.js"
import type { NativeEventCallback, NativeRenderer } from "./native.js"
import type { EventPayload } from "./types.js"

interface NativeModule {
  GpuiRenderer: new (callback?: NativeEventCallback) => NativeGpuiRenderer
}

let nativeModule: NativeModule | undefined

export function loadNativeModule(): NativeModule {
  if (nativeModule !== undefined) return nativeModule
  const require = createRequire(import.meta.url)
  nativeModule = require("@gpui-vue/native") as NativeModule
  return nativeModule
}

export function createNativeRenderer(onEvent?: (event: EventPayload) => void): NativeRenderer {
  const { GpuiRenderer } = loadNativeModule()
  let renderer: NativeRenderer
  renderer = new GpuiRenderer((error, event) => {
    if (error !== null) {
      console.error("[gpui-vue] native event error", error)
      return
    }
    if (event !== null) {
      handleGpuiEvent(event, renderer)
      onEvent?.(event)
    }
  }) as NativeRenderer
  return renderer
}

/** Lazy constructor-compatible export for users who need the raw native API. */
export const GpuiRenderer = new Proxy(class LazyGpuiRenderer {}, {
  construct(_target, args) {
    return Reflect.construct(loadNativeModule().GpuiRenderer, args)
  },
}) as unknown as NativeModule["GpuiRenderer"]
