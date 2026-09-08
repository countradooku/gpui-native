import {
  defineComponent,
  h,
  isVNode,
  type App,
  type Component,
  type VNode,
} from "@vue/runtime-core"

import { clearEventHandlers, subscribeRendererEvent } from "./events.js"
import type { NativeRenderer } from "./native.js"
import { createGpuiRenderer, type GpuiRendererHost } from "./renderer.js"
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
}

export type DefaultRendererFactory = (onEvent?: (event: EventPayload) => void) => NativeRenderer

const DEFAULT_FRAME_MS = 8
const DEFAULT_LIVENESS_MS = 100
// Node and Bun clamp timers to a signed 32-bit millisecond delay. One dormant
// interval keeps the JavaScript runtime alive without polling the native UI.
const EVENT_LOOP_KEEP_ALIVE_MS = 2_147_483_647

interface RenderSlot {
  renderer?: NativeRenderer
  host?: GpuiRendererHost
  root?: GpuiRoot
  loop?: FrameLoop
}

export function startFrameLoop(
  renderer: NativeRenderer,
  options: {
    frameMs?: number
    livenessMs?: number
    keepAlive?: boolean
    onTerminated?: () => void
  } = {},
): FrameLoop {
  if (renderer.requiresTick?.() !== true || renderer.tick === undefined) {
    if (options.keepAlive !== true) return { stop() {} }
    if (renderer.supportsWindowEvents?.() === true) {
      let stopped = false
      // N-API event callbacks are deliberately unreferenced, and a native UI
      // thread alone does not retain the JavaScript process. Hold one dormant
      // timer until the authoritative native close event arrives.
      const keepAliveTimer = setInterval(() => undefined, EVENT_LOOP_KEEP_ALIVE_MS)
      let unsubscribe: (() => void) | undefined
      const stop = (): void => {
        if (stopped) return
        stopped = true
        clearInterval(keepAliveTimer)
        unsubscribe?.()
      }
      unsubscribe = subscribeRendererEvent(renderer, "windowClose", () => {
        stop()
        options.onTerminated?.()
      })
      return { stop }
    }

    let timer: ReturnType<typeof setTimeout> | undefined
    let stopped = false
    const stop = (): void => {
      stopped = true
      if (timer !== undefined) clearTimeout(timer)
      timer = undefined
    }
    const poll = (): void => {
      if (stopped) return
      if (renderer.isInitialized?.() === false) {
        stop()
        options.onTerminated?.()
        return
      }
      timer = setTimeout(poll, options.livenessMs ?? DEFAULT_LIVENESS_MS)
    }
    poll()
    return { stop }
  }

  const frameMs = options.frameMs ?? DEFAULT_FRAME_MS
  let timer: ReturnType<typeof setTimeout> | undefined
  let stopped = false
  const stop = (): void => {
    stopped = true
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
  }
  const loop = (): void => {
    if (stopped) return
    const started = performance.now()
    if (renderer.tick?.() === false) {
      stop()
      options.onTerminated?.()
      return
    }
    timer = setTimeout(loop, Math.max(0, frameMs - (performance.now() - started)))
  }
  loop()
  return { stop }
}

export function createGpuiRuntime(createDefaultRenderer: DefaultRendererFactory, hostKey: string) {
  const renderSlot = (): RenderSlot => {
    const existing = Reflect.get(globalThis, hostKey) as RenderSlot | undefined
    if (existing !== undefined) return existing
    const created: RenderSlot = {}
    Reflect.set(globalThis, hostKey, created)
    return created
  }

  const rootComponent = (name: string, root: Component | VNode): Component =>
    defineComponent({
      name,
      setup: () => () => (isVNode(root) ? root : h(root)),
    })

  const render = (root: Component | VNode, options: RenderOptions = {}): GpuiRoot => {
    const slot = renderSlot()
    const { renderer: injected, onEvent, debugFrameOverlay, frameMs, ...windowOptions } = options
    if (slot.renderer === undefined) {
      slot.renderer = injected ?? createDefaultRenderer(onEvent)
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
    if (debugFrameOverlay !== undefined) nativeRenderer.setDebugFrameOverlay?.(debugFrameOverlay)
    const app = host.mount(rootComponent("GpuiVueRoot", root))

    let mounted = true
    const mountedRoot: GpuiRoot = {
      app,
      host,
      renderer: host.renderer,
      unmount(): void {
        if (!mounted) return
        mounted = false
        app.unmount()
        host.flushMutations()
        if (slot.root === mountedRoot) delete slot.root
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
    const { renderer: injected, onEvent, debugFrameOverlay, frameMs, ...windowOptions } = options
    const nativeRenderer = injected ?? createDefaultRenderer(onEvent)
    if (nativeRenderer.isInitialized?.() !== true) nativeRenderer.init?.(windowOptions)
    if (debugFrameOverlay !== undefined) nativeRenderer.setDebugFrameOverlay?.(debugFrameOverlay)
    const host = createGpuiRenderer(nativeRenderer)
    const app = host.mount(rootComponent("GpuiVueWindowRoot", root))
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
