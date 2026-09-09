import { createRequire } from "node:module"

import type { GpuiRenderer as NativeGpuiRenderer } from "@gpui-native/core"

import { handleGpuiEvent } from "./events.js"
import type { NativeEventCallback, NativeRenderer } from "./native.js"
import { reportRuntimeError } from "./runtime-errors.js"
import type { EventPayload } from "./types.js"
import { wrapDevice } from "./wgpu-native-objects.js"

interface NativeModule {
  GpuiRenderer: new (callback?: NativeEventCallback) => NativeGpuiRenderer
}

let nativeModule: NativeModule | undefined

function adaptNativeRenderer(native: NativeGpuiRenderer): NativeRenderer {
  const methods = new Map<PropertyKey, unknown>()
  return new Proxy(native, {
    get(target, property): unknown {
      const cached = methods.get(property)
      if (cached !== undefined) return cached

      if (property === "canvasGpuDevice") {
        let shared: GPUDevice | undefined
        const acquire = () => {
          if (shared) return shared
          const native = target.canvasGpuDevice()
          if (!native) return null
          shared = wrapDevice(native, {
            vendor: "",
            architecture: "",
            device: "",
            description: "GPUI shared device",
          } as GPUAdapterInfo)
          void shared.lost.then(() => {
            shared = undefined
          })
          return shared
        }
        methods.set(property, acquire)
        return acquire
      }
      if (property === "setStyle") {
        const setStyle: NativeRenderer["setStyle"] = (id, style) => {
          target.setStyle(id, typeof style === "string" ? style : JSON.stringify(style))
        }
        methods.set(property, setStyle)
        return setStyle
      }
      if (property === "setCustomProp") {
        const setCustomProp: NativeRenderer["setCustomProp"] = (id, key, value) => {
          target.setCustomProp(id, key, typeof value === "string" ? value : JSON.stringify(value))
        }
        methods.set(property, setCustomProp)
        return setCustomProp
      }

      const value: unknown = Reflect.get(target, property, target)
      if (typeof value !== "function") return value
      const bound = value.bind(target) as unknown
      methods.set(property, bound)
      return bound
    },
  }) as unknown as NativeRenderer
}

export function loadNativeModule(): NativeModule {
  if (nativeModule !== undefined) return nativeModule
  const require = createRequire(import.meta.url)
  nativeModule = require("@gpui-native/core") as NativeModule
  return nativeModule
}

export function createNativeRenderer(onEvent?: (event: EventPayload) => void): NativeRenderer {
  const { GpuiRenderer } = loadNativeModule()
  let renderer: NativeRenderer
  renderer = adaptNativeRenderer(
    new GpuiRenderer((error, event) => {
      if (error !== null) {
        console.error("[gpui-vue] native event error", error)
        return
      }
      if (event !== null) {
        try {
          const delivered = handleGpuiEvent(event, renderer)
          if (delivered || event.elementId === 0) onEvent?.(event)
        } catch (error) {
          reportRuntimeError(renderer, error)
        }
      }
    }),
  )
  return renderer
}

/** Lazy constructor-compatible export for users who need the raw native API. */
export const GpuiRenderer = new Proxy(class LazyGpuiRenderer {}, {
  construct(_target, args) {
    return Reflect.construct(loadNativeModule().GpuiRenderer, args)
  },
}) as unknown as NativeModule["GpuiRenderer"]
