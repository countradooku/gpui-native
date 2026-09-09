import { startFrameLoop } from "@gpui-native/runtime/frame-loop"
export { startFrameLoop } from "@gpui-native/runtime/frame-loop"
import {
  defineComponent,
  h,
  isVNode,
  shallowRef,
  onErrorCaptured,
  onUnmounted,
  type App,
  type Component,
  type VNode,
} from "@vue/runtime-core"

import { clearEventHandlers, registerEventHandler, unregisterEventHandlers } from "./events.js"
import type { NativeRenderer } from "./native.js"
import { createGpuiRenderer, allocateRendererId, type GpuiRendererHost } from "./renderer.js"
import { listenForRuntimeErrors } from "./runtime-errors.js"
import type { EventPayload, WindowOptions } from "./types.js"

export interface FrameLoop {
  stop(): void
}

export interface GpuiRoot {
  readonly app: App
  readonly host: GpuiRendererHost
  readonly renderer: NativeRenderer
  unmount(): void
}

export interface GpuiWindowRoot extends GpuiRoot {
  close(): void
}

export interface RenderOptions extends WindowOptions {
  renderer?: NativeRenderer
  frameMs?: number
  onKeyDown?: (event: EventPayload) => void
  onKeyUp?: (event: EventPayload) => void
}

export type DefaultRendererFactory = (onEvent?: (event: EventPayload) => void) => NativeRenderer

interface RenderSlot {
  renderer?: NativeRenderer
  host?: GpuiRendererHost
  root?: GpuiRoot
  loop?: FrameLoop
  onEvent?: (event: EventPayload) => void
}

export function createGpuiRuntime(createDefaultRenderer: DefaultRendererFactory, hostKey: string) {
  const renderSlot = (): RenderSlot => {
    const existing = Reflect.get(globalThis, hostKey) as RenderSlot | undefined
    if (existing !== undefined) return existing
    const created: RenderSlot = {}
    Reflect.set(globalThis, hostKey, created)
    return created
  }

  const rootComponent = (
    name: string,
    root: Component | VNode,
    renderer: NativeRenderer,
  ): Component =>
    defineComponent({
      name,
      setup() {
        const failure = shallowRef<string>()
        const showError = (error: unknown): void => {
          failure.value = error instanceof Error ? (error.stack ?? error.message) : String(error)
        }
        const stop = listenForRuntimeErrors(renderer, showError)
        onUnmounted(stop)
        onErrorCaptured((error) => {
          showError(error)
          return false
        })
        return () =>
          failure.value === undefined
            ? isVNode(root)
              ? root
              : h(root)
            : h(
                "div",
                {
                  style: {
                    width: "100%",
                    height: "100%",
                    padding: 20,
                    background: "#240e16",
                    color: "#ffccd5",
                    overflow: "scroll",
                  },
                  role: "alert",
                  "aria-label": "Application runtime error",
                },
                [
                  h("text", { style: { fontWeight: 700 } }, "Application runtime error"),
                  h("text", { style: { fontFamily: "monospace" } }, failure.value),
                ],
              )
      },
    })

  const bindKeys = (
    renderer: NativeRenderer,
    down: RenderOptions["onKeyDown"],
    up: RenderOptions["onKeyUp"],
  ): (() => void) => {
    const id = allocateRendererId(renderer)
    if (down !== undefined) registerEventHandler(id, "windowKeyDown", down, renderer)
    if (up !== undefined) registerEventHandler(id, "windowKeyUp", up, renderer)
    renderer.setWindowKeyEvents?.(down !== undefined, up !== undefined, id)
    return () => {
      unregisterEventHandlers(id, renderer)
      renderer.setWindowKeyEvents?.(false, false, id)
    }
  }

  const render = (root: Component | VNode, options: RenderOptions = {}): GpuiRoot => {
    const slot = renderSlot()
    const {
      renderer: injected,
      onEvent,
      onKeyDown,
      onKeyUp,
      debugFrameOverlay,
      frameMs,
      ...windowOptions
    } = options
    if (slot.renderer === undefined) {
      slot.renderer = injected ?? createDefaultRenderer((event) => slot.onEvent?.(event))
      if (slot.renderer.isInitialized?.() !== true) slot.renderer.init?.(windowOptions)
      slot.host = createGpuiRenderer(slot.renderer)
    } else if (injected !== undefined && injected !== slot.renderer) {
      throw new Error("gpui-vue render() cannot replace an existing native window renderer")
    }

    const nativeRenderer = slot.renderer
    const host = slot.host
    if (nativeRenderer === undefined || host === undefined) {
      throw new Error("gpui-vue native host failed to initialize")
    }
    slot.root?.unmount()
    if (onEvent === undefined) delete slot.onEvent
    else slot.onEvent = onEvent
    const unbindKeys = bindKeys(nativeRenderer, onKeyDown, onKeyUp)
    if (debugFrameOverlay !== undefined) nativeRenderer.setDebugFrameOverlay?.(debugFrameOverlay)
    const app = host.mount(rootComponent("GpuiVueRoot", root, nativeRenderer))

    let mounted = true
    const mountedRoot: GpuiRoot = {
      app,
      host,
      renderer: host.renderer,
      unmount(): void {
        if (!mounted) return
        mounted = false
        unbindKeys()
        app.unmount()
        host.flushMutations()
        if (slot.root === mountedRoot) {
          delete slot.root
          delete slot.onEvent
        }
      },
    }
    slot.root = mountedRoot
    if (injected === undefined) {
      slot.loop?.stop()
      slot.loop = startFrameLoop(nativeRenderer, {
        ...(frameMs === undefined ? {} : { frameMs }),
        keepAlive: windowOptions.headless !== true,
      })
    }
    return mountedRoot
  }

  const createWindow = (root: Component | VNode, options: RenderOptions = {}): GpuiWindowRoot => {
    const {
      renderer: injected,
      onEvent,
      onKeyDown,
      onKeyUp,
      debugFrameOverlay,
      frameMs,
      ...windowOptions
    } = options
    const nativeRenderer = injected ?? createDefaultRenderer(onEvent)
    if (nativeRenderer.isInitialized?.() !== true) nativeRenderer.init?.(windowOptions)
    if (debugFrameOverlay !== undefined) nativeRenderer.setDebugFrameOverlay?.(debugFrameOverlay)
    const host = createGpuiRenderer(nativeRenderer)
    const unbindKeys = bindKeys(nativeRenderer, onKeyDown, onKeyUp)
    const app = host.mount(rootComponent("GpuiVueWindowRoot", root, nativeRenderer))
    const loop =
      injected === undefined
        ? startFrameLoop(nativeRenderer, {
            ...(frameMs === undefined ? {} : { frameMs }),
            keepAlive: windowOptions.headless !== true,
          })
        : { stop() {} }
    let mounted = true
    let closed = false
    const windowRoot: GpuiWindowRoot = {
      app,
      host,
      renderer: host.renderer,
      unmount(): void {
        if (!mounted) return
        mounted = false
        unbindKeys()
        app.unmount()
        host.flushMutations()
      },
      close(): void {
        if (closed) return
        closed = true
        windowRoot.unmount()
        loop.stop()
        host.destroy()
        clearEventHandlers(nativeRenderer)
        nativeRenderer.close?.()
      },
    }
    return windowRoot
  }

  const resetRender = (): void => {
    const slot = renderSlot()
    slot.root?.unmount()
    slot.loop?.stop()
    slot.host?.destroy()
    slot.renderer?.close?.()
    if (slot.renderer !== undefined) clearEventHandlers(slot.renderer)
    Reflect.deleteProperty(globalThis, hostKey)
  }

  return { render, createWindow, resetRender }
}
