import {
  defineComponent,
  h,
  isVNode,
  type App,
  type Component,
  type VNode,
} from "@vue/runtime-core"

import { GpuiRendererKey } from "./context.js"
import { clearEventHandlers } from "./events.js"
import type { NativeRenderer } from "./native.js"
import { createGpuiRenderer, type GpuiRendererHost } from "./renderer.js"
import type { WindowOptions } from "./types.js"
import { createWebNativeRenderer } from "./web-native.js"

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
  /** Unmount Vue, destroy this retained tree, and close only this window. */
  close(): void
}

export interface RenderOptions extends WindowOptions {
  renderer?: NativeRenderer
  frameMs?: number
}

const DEFAULT_FRAME_MS = 8
const DEFAULT_LIVENESS_MS = 100
const RENDER_HOST_KEY = "__gpuiVueWebRenderHost"

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
    const wait = Math.max(0, frameMs - (performance.now() - started))
    timer = setTimeout(loop, wait)
  }
  loop()
  return { stop }
}

/** Mounts or hot-remounts a Vue root into one first-party GPUI window. */
export function render(root: Component | VNode, options: RenderOptions = {}): GpuiRoot {
  const slot = renderSlot()
  const { renderer: injected, onEvent, debugFrameOverlay, frameMs, ...windowOptions } = options

  if (slot.renderer === undefined) {
    slot.renderer = injected ?? createWebNativeRenderer(onEvent)
    if (slot.renderer.isInitialized?.() !== true) {
      slot.renderer.init?.(windowOptions)
    }
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
  if (debugFrameOverlay !== undefined) {
    nativeRenderer.setDebugFrameOverlay?.(debugFrameOverlay)
  }

  const RootComponent = defineComponent({
    name: "GpuiVueRoot",
    setup: () => () => (isVNode(root) ? root : h(root)),
  })
  const app = host.createApp(RootComponent)
  app.provide(GpuiRendererKey, host.renderer)
  app.mount(host.root)
  host.flushMutations()

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

/**
 * Open an independently mounted GPUI window. Unlike `render()`, this function
 * never uses the hot-remount singleton, so any number of roots can coexist.
 */
export function createWindow(root: Component | VNode, options: RenderOptions = {}): GpuiWindowRoot {
  const { renderer: injected, onEvent, debugFrameOverlay, frameMs, ...windowOptions } = options
  const nativeRenderer: NativeRenderer = injected ?? createWebNativeRenderer(onEvent)
  if (nativeRenderer.isInitialized?.() !== true) {
    nativeRenderer.init?.(windowOptions)
  }
  if (debugFrameOverlay !== undefined) {
    nativeRenderer.setDebugFrameOverlay?.(debugFrameOverlay)
  }
  const host = createGpuiRenderer(nativeRenderer)
  const RootComponent = defineComponent({
    name: "GpuiVueWindowRoot",
    setup: () => () => (isVNode(root) ? root : h(root)),
  })
  const app = host.createApp(RootComponent)
  app.provide(GpuiRendererKey, host.renderer)
  app.mount(host.root)
  host.flushMutations()

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

export function resetRender(): void {
  const slot = renderSlot()
  slot.root?.unmount()
  slot.loop?.stop()
  slot.host?.destroy()
  slot.renderer?.close?.()
  if (slot.renderer !== undefined) clearEventHandlers(slot.renderer)
  Reflect.deleteProperty(globalThis, RENDER_HOST_KEY)
}

function renderSlot(): RenderSlot {
  const existing = Reflect.get(globalThis, RENDER_HOST_KEY) as RenderSlot | undefined
  if (existing !== undefined) return existing
  const created: RenderSlot = {}
  Reflect.set(globalThis, RENDER_HOST_KEY, created)
  return created
}
