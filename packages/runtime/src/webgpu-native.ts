/// <reference types="@webgpu/types" preserve="true" />

function assertSupportedHost(): void {
  if (process.versions.bun) {
    throw new Error(
      "Dawn WebGPU requires Node.js: Bun 1.4.0 crashes during asynchronous pipeline creation. Run the built application with node, or supply your own compatible GPU to GPUCanvas.",
    )
  }
}

/** Load Dawn only from a desktop entry point. Keep the returned GPU alive while using its devices. */
export async function createNativeGPU(options: string[] = []): Promise<GPU> {
  assertSupportedHost()
  const { create } = await import("webgpu")
  return create(options)
}

/**
 * Install Dawn's WebGPU constructors/constants and navigator.gpu for libraries
 * such as Three.js. Returns an idempotent restoration function. Explicit opt-in:
 * importing this module never changes globals or loads the native library.
 */
export async function installWebGPU(options: string[] = []): Promise<() => void> {
  assertSupportedHost()
  const { create, globals } = await import("webgpu")
  const gpu = create(options)
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
