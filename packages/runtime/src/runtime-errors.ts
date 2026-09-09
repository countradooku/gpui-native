import type { NativeRenderer } from "./native.js"

const key = Symbol.for("gpui-native.vue.errors")
const listeners =
  (Reflect.get(globalThis, key) as WeakMap<NativeRenderer, (error: unknown) => void> | undefined) ??
  new WeakMap<NativeRenderer, (error: unknown) => void>()
Reflect.set(globalThis, key, listeners)

export function reportRuntimeError(renderer: NativeRenderer, error: unknown): void {
  console.error("[gpui-native] runtime error", error)
  listeners.get(renderer)?.(error)
}

export function listenForRuntimeErrors(
  renderer: NativeRenderer,
  listener: (error: unknown) => void,
): () => void {
  listeners.set(renderer, listener)
  return () => {
    if (listeners.get(renderer) === listener) listeners.delete(renderer)
  }
}
