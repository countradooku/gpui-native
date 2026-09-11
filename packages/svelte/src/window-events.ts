import { registerEventHandler, unregisterEventHandlers } from "@gpui-native/runtime/events"
import type { NativeRenderer } from "@gpui-native/runtime/native"
import { allocateRendererId } from "@gpui-native/runtime/ownership"

import type { GpuiEventHandler } from "./types.js"

type KeyType = "windowKeyDown" | "windowKeyUp"
interface Subscription {
  type: KeyType
  handler: GpuiEventHandler
}
const scopes = new WeakMap<NativeRenderer, { id: number; subscriptions: Set<Subscription> }>()
/** Merge window callbacks and component subscriptions into one native listener. */
export function subscribeWindowKey(
  renderer: NativeRenderer,
  type: KeyType,
  handler: GpuiEventHandler,
) {
  const native =
    (renderer as NativeRenderer & { innerRenderer?: NativeRenderer }).innerRenderer ?? renderer
  let scope = scopes.get(native)
  if (!scope) {
    scope = { id: allocateRendererId(native), subscriptions: new Set() }
    scopes.set(native, scope)
    for (const type of ["windowKeyDown", "windowKeyUp"] as const) {
      const subscriptions = scope.subscriptions
      registerEventHandler(
        scope.id,
        type,
        (event) => {
          // Snapshot so callbacks can subscribe/unsubscribe without extending this dispatch.
          // oxlint-disable-next-line unicorn/no-useless-spread
          for (const entry of [...subscriptions]) if (entry.type === type) entry.handler(event)
        },
        native,
      )
    }
  }
  const current = scope
  const update = () =>
    native.setWindowKeyEvents?.(
      [...current.subscriptions].some((entry) => entry.type === "windowKeyDown"),
      [...current.subscriptions].some((entry) => entry.type === "windowKeyUp"),
      current.id,
    )
  const entry = { type, handler }
  current.subscriptions.add(entry)
  update()
  let active = true
  return () => {
    if (!active) return
    active = false
    current.subscriptions.delete(entry)
    update()
    if (!current.subscriptions.size) {
      unregisterEventHandlers(current.id, native)
      scopes.delete(native)
    }
  }
}
