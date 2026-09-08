import type { NativeNodeId, NativeRenderer } from "./native.js"
import type { EventPayload, GpuiEventHandler } from "./types.js"

export type GpuiEventHandlerValue = GpuiEventHandler | GpuiEventHandler[]

type HandlerMap = Map<NativeNodeId, Map<string, GpuiEventHandlerValue>>

const eventStateKey = Symbol.for("gpui-native.vue.events")
const eventState = (Reflect.get(globalThis, eventStateKey) as
  | {
      fallback: HandlerMap
      renderers: WeakMap<object, HandlerMap>
    }
  | undefined) ?? { fallback: new Map(), renderers: new WeakMap() }
Reflect.set(globalThis, eventStateKey, eventState)
const fallbackEventHandlers = eventState.fallback
const rendererEventHandlers = eventState.renderers

export const EVENT_PROPS = [
  ["onToggleFile", "toggleFile"],
  ["onShowMore", "showMore"],
  ["onLineClick", "lineClick"],
  ["onLinkClick", "linkClick"],
  ["onChange", "change"],
  ["onSubmit", "submit"],
  ["onClick", "click"],
  ["onAuxClick", "auxClick"],
  ["onMouseDown", "mouseDown"],
  ["onMouseUp", "mouseUp"],
  ["onMouseEnter", "mouseEnter"],
  ["onMouseLeave", "mouseLeave"],
  ["onMouseMove", "mouseMove"],
  ["onMouseDownOutside", "mouseDownOutside"],
  ["onKeyDown", "keyDown"],
  ["onKeyUp", "keyUp"],
  ["onFocus", "focus"],
  ["onBlur", "blur"],
  ["onScroll", "scroll"],
  ["onVisibleRange", "visibleRange"],
  ["onHighlight", "highlight"],
] as const

const EVENT_TYPES = new Map<string, string>(EVENT_PROPS)

export function eventTypeForProp(prop: string): string | undefined {
  return EVENT_TYPES.get(prop)
}

export function isEventProp(prop: string): boolean {
  return EVENT_TYPES.has(prop)
}

export function patchEvent(
  renderer: NativeRenderer,
  elementId: NativeNodeId,
  prop: string,
  previous: unknown,
  next: unknown,
): void {
  const eventType = eventTypeForProp(prop)
  if (eventType === undefined) return

  const hadHandler = isHandler(previous)
  const hasHandler = isHandler(next)
  if (hasHandler) {
    registerEventHandler(elementId, eventType, next, renderer)
  } else {
    unregisterEventHandler(elementId, eventType, renderer)
  }
  if (hadHandler !== hasHandler) {
    renderer.setEventListener(elementId, eventType, hasHandler)
  }
}

export function handleGpuiEvent(payload: EventPayload, renderer?: NativeRenderer): boolean {
  const handler = handlersFor(renderer).get(payload.elementId)?.get(payload.eventType)
  if (Array.isArray(handler)) {
    for (const callback of handler.slice()) callback(payload)
  } else {
    handler?.(payload)
  }
  return handler !== undefined
}

export function subscribeRendererEvent(
  renderer: NativeRenderer,
  eventType: string,
  handler: GpuiEventHandler,
): () => void {
  const current = handlersFor(renderer).get(0)?.get(eventType)
  registerEventHandler(
    0,
    eventType,
    [...(Array.isArray(current) ? current : current ? [current] : []), handler],
    renderer,
  )
  return () => {
    const current = handlersFor(renderer).get(0)?.get(eventType)
    const remaining = (Array.isArray(current) ? current : current ? [current] : []).filter(
      (callback) => callback !== handler,
    )
    if (remaining.length) registerEventHandler(0, eventType, remaining, renderer)
    else unregisterEventHandler(0, eventType, renderer)
  }
}

export function registerEventHandler(
  elementId: NativeNodeId,
  eventType: string,
  handler: GpuiEventHandlerValue,
  renderer?: NativeRenderer,
): void {
  const eventHandlers = handlersFor(renderer)
  let handlers = eventHandlers.get(elementId)
  if (handlers === undefined) {
    handlers = new Map()
    eventHandlers.set(elementId, handlers)
  }
  handlers.set(eventType, handler)
}

export function unregisterEventHandler(
  elementId: NativeNodeId,
  eventType: string,
  renderer?: NativeRenderer,
): void {
  const eventHandlers = handlersFor(renderer)
  const handlers = eventHandlers.get(elementId)
  if (handlers === undefined) return
  handlers.delete(eventType)
  if (handlers.size === 0) eventHandlers.delete(elementId)
}

export function unregisterEventHandlers(elementId: NativeNodeId, renderer?: NativeRenderer): void {
  handlersFor(renderer).delete(elementId)
}

export function clearEventHandlers(renderer?: NativeRenderer): void {
  if (renderer === undefined) {
    fallbackEventHandlers.clear()
    return
  }
  const key = rendererKey(renderer)
  const handlers = rendererEventHandlers.get(key)
  if (handlers === undefined) return
  handlers.clear()
  rendererEventHandlers.delete(key)
}

function handlersFor(renderer?: NativeRenderer): HandlerMap {
  if (renderer === undefined) return fallbackEventHandlers
  const key = rendererKey(renderer)
  let handlers = rendererEventHandlers.get(key)
  if (handlers === undefined) {
    handlers = new Map()
    rendererEventHandlers.set(key, handlers)
  }
  return handlers
}

function rendererKey(renderer: NativeRenderer): object {
  const candidate = renderer as NativeRenderer & { innerRenderer?: NativeRenderer }
  return (candidate.innerRenderer ?? renderer) as object
}

function isHandler(value: unknown): value is GpuiEventHandlerValue {
  return (
    typeof value === "function" ||
    (Array.isArray(value) && value.every((item) => typeof item === "function"))
  )
}
