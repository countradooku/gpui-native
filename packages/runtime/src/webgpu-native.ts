/// <reference types="@webgpu/types" preserve="true" />

import { createRequire } from "node:module"

import { globals, wrapAdapter } from "./wgpu-native-objects.js"

/** Create the Rust/wgpu Node-API implementation used by both Bun and Node. */
export async function createNativeGPU(options: string[] = []): Promise<GPU> {
  if (options.length)
    throw new TypeError(
      "Dawn option strings are unsupported by wgpu; use requestAdapter/requestDevice descriptors",
    )
  const { requestWgpuAdapter } = createRequire(import.meta.url)(
    "@gpui-native/core",
  ) as typeof import("@gpui-native/core")
  return {
    requestAdapter: async (options: GPURequestAdapterOptions = {}) =>
      wrapAdapter(await requestWgpuAdapter(options)),
    getPreferredCanvasFormat: () => "bgra8unorm",
    wgslLanguageFeatures: new Set<string>(),
  } as unknown as GPU
}

/**
 * Install the native wgpu binding's WebGPU constructors/constants and navigator.gpu for libraries
 * such as Three.js. Returns an idempotent restoration function. Explicit opt-in:
 * importing this module never changes globals or loads the native library.
 */
export async function installWebGPU(options: string[] = []): Promise<() => void> {
  const gpu = await createNativeGPU(options)
  const restores: (() => void)[] = []
  const define = (target: object, key: string, value: unknown) => {
    const previous = Object.getOwnPropertyDescriptor(target, key)
    Object.defineProperty(target, key, { configurable: true, writable: true, value })
    restores.push(() => {
      // Do not overwrite another owner's later installation.
      if (Reflect.get(target, key) !== value) return
      if (previous) Object.defineProperty(target, key, previous)
      else Reflect.deleteProperty(target, key)
    })
  }
  try {
    if (typeof globalThis.requestAnimationFrame !== "function") {
      const timers = new Map<number, ReturnType<typeof setTimeout>>()
      let nextId = 0
      restores.push(() => {
        for (const timer of timers.values()) clearTimeout(timer)
        timers.clear()
      })
      define(globalThis, "requestAnimationFrame", (callback: FrameRequestCallback) => {
        const id = ++nextId
        const timer = setTimeout(() => {
          timers.delete(id)
          callback(performance.now())
        }, 16)
        timer.unref()
        timers.set(id, timer)
        return id
      })
      define(globalThis, "cancelAnimationFrame", (id: number) => {
        const timer = timers.get(id)
        if (timer) clearTimeout(timer)
        timers.delete(id)
      })
    }
    if (typeof globalThis.self === "undefined") define(globalThis, "self", globalThis)
    for (const [key, value] of Object.entries(globals)) define(globalThis, key, value)
    if (!globalThis.navigator) define(globalThis, "navigator", {})
    define(globalThis.navigator, "gpu", gpu)
  } catch (error) {
    for (const restore of restores.splice(0).reverse()) restore()
    throw error
  }
  let restored = false
  return () => {
    if (restored) return
    restored = true
    for (const restore of restores.splice(0).reverse()) restore()
  }
}
