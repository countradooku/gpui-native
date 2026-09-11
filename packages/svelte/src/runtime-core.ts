import { clearEventHandlers } from "@gpui-native/runtime/events"
import { startFrameLoop } from "@gpui-native/runtime/frame-loop"
import type { NativeRenderer } from "@gpui-native/runtime/native"
import { listenForRuntimeErrors } from "@gpui-native/runtime/runtime-errors"
import type { Component } from "svelte"

import RuntimeBoundary from "./controls/RuntimeBoundary.svelte"
import { createGpuiRenderer } from "./renderer.js"
import type { EventPayload, WindowOptions } from "./types.js"
import { subscribeWindowKey } from "./window-events.js"
export { startFrameLoop } from "@gpui-native/runtime/frame-loop"
export type { FrameLoop } from "@gpui-native/runtime/frame-loop"
export interface RenderOptions<
  Props extends Record<string, unknown> = Record<string, unknown>,
> extends WindowOptions {
  props?: Props
  renderer?: NativeRenderer
  frameMs?: number
  onKeyDown?: (event: EventPayload) => void
  onKeyUp?: (event: EventPayload) => void
  onError?: (error: unknown) => void
}
export function createGpuiRuntime(
  factory: (onEvent?: (event: EventPayload) => void) => NativeRenderer,
) {
  let current: GpuiWindowRoot | undefined
  function createWindow<Props extends Record<string, unknown>>(
    component: Component<Props>,
    options: RenderOptions<Props> = {},
  ) {
    const {
      renderer: injected,
      props = {},
      onEvent,
      onKeyDown,
      onKeyUp,
      debugFrameOverlay,
      frameMs,
      onError,
      ...windowOptions
    } = options
    const native = injected ?? factory(onEvent)
    let host: ReturnType<typeof createGpuiRenderer>
    try {
      if (native.isInitialized?.() !== true) native.init?.(windowOptions)
      host = createGpuiRenderer(native)
    } catch (error) {
      if (!injected) native.close?.()
      throw error
    }
    let closed = false,
      stopKeys: (() => void)[] = [],
      stopErrors: () => void = () => {}
    const keys = (down: RenderOptions["onKeyDown"], up: RenderOptions["onKeyUp"]) => {
      stopKeys.splice(0).forEach((stop) => stop())
      if (down) stopKeys.push(subscribeWindowKey(native, "windowKeyDown", down))
      if (up) stopKeys.push(subscribeWindowKey(native, "windowKeyUp", up))
    }
    const renderNode = <P extends Record<string, unknown>>(
      next: Component<P>,
      nextOptions: RenderOptions<P> = {},
    ) => {
      if (closed) throw new Error("Cannot render into a closed Svelte window")
      keys(nextOptions.onKeyDown, nextOptions.onKeyUp)
      if (nextOptions.debugFrameOverlay !== undefined)
        native.setDebugFrameOverlay?.(nextOptions.debugFrameOverlay)
      stopErrors()
      const boundaryProps = {
        component: next as unknown as Component<Record<string, unknown>>,
        props: nextOptions.props ?? {},
        ...(nextOptions.onError ? { onError: nextOptions.onError } : {}),
      }
      host.render(RuntimeBoundary, boundaryProps)
      stopErrors = listenForRuntimeErrors(native, (error) => {
        nextOptions.onError?.(error)
        host.render(RuntimeBoundary, { ...boundaryProps, error })
      })
    }
    try {
      renderNode(component, {
        ...options,
        props: props as Props,
        onKeyDown,
        onKeyUp,
        onError,
        debugFrameOverlay,
      } as RenderOptions<Props>)
    } catch (error) {
      host.destroy()
      stopKeys.splice(0).forEach((stop) => stop())
      if (!injected) native.close?.()
      throw error
    }
    const loop = injected
      ? { stop() {} }
      : startFrameLoop(native, {
          ...(frameMs === undefined ? {} : { frameMs }),
          keepAlive: windowOptions.headless !== true,
        })
    return {
      host,
      renderer: host.renderer,
      render: renderNode,
      get closed() {
        return closed
      },
      unmount() {
        if (closed) return
        keys(undefined, undefined)
        stopErrors()
        host.render(null)
      },
      close() {
        if (closed) return
        closed = true
        loop.stop()
        stopErrors()
        keys(undefined, undefined)
        try {
          host.destroy()
        } finally {
          clearEventHandlers(native)
          native.close?.()
        }
      },
    }
  }
  function render<Props extends Record<string, unknown>>(
    component: Component<Props>,
    options: RenderOptions<Props> = {},
  ) {
    if (current?.closed) current = undefined
    if (current && options.renderer && current.host.nativeRenderer !== options.renderer)
      throw new Error("render() cannot replace an existing window renderer")
    if (current) current.render(component, options)
    else current = createWindow(component, options)
    return current
  }
  function resetRender() {
    current?.close()
    current = undefined
  }
  return { createWindow, render, resetRender }
}
export type GpuiWindowRoot = ReturnType<ReturnType<typeof createGpuiRuntime>["createWindow"]>
export type GpuiRoot = GpuiWindowRoot
