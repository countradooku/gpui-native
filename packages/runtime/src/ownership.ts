import type { NativeRenderer } from "./native.js"
const stateKey = Symbol.for("gpui-native.runtime.ownership")
const state = (Reflect.get(globalThis, stateKey) as
  | { owners: WeakSet<NativeRenderer>; ids: WeakMap<NativeRenderer, number> }
  | undefined) ?? { owners: new WeakSet(), ids: new WeakMap() }
Reflect.set(globalThis, stateKey, state)
const { owners, ids } = state
export function allocateRendererId(renderer: NativeRenderer): number {
  const id = (ids.get(renderer) ?? 0) + 1
  ids.set(renderer, id)
  return id
}
export function claimRenderer(renderer: NativeRenderer, label = "root"): () => void {
  if (owners.has(renderer)) throw new Error(`A native GPUI renderer can only own one live ${label}`)
  owners.add(renderer)
  return () => {
    owners.delete(renderer)
  }
}
