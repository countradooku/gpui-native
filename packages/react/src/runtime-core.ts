import {
  clearEventHandlers,
  registerEventHandler,
  unregisterEventHandlers,
} from "@gpui-native/runtime/events"
import { startFrameLoop } from "@gpui-native/runtime/frame-loop"
import type { NativeRenderer } from "@gpui-native/runtime/native"
import { allocateRendererId } from "@gpui-native/runtime/ownership"
import { listenForRuntimeErrors } from "@gpui-native/runtime/runtime-errors"
import { Component, createElement, type ReactNode } from "react"

import { createGpuiRenderer } from "./renderer.js"
import type { EventPayload, WindowOptions } from "./types.js"
export { startFrameLoop } from "@gpui-native/runtime/frame-loop"
export type { FrameLoop } from "@gpui-native/runtime/frame-loop"
export interface RenderOptions extends WindowOptions {
  renderer?: NativeRenderer
  frameMs?: number
  onKeyDown?: (event: EventPayload) => void
  onKeyUp?: (event: EventPayload) => void
}
class RuntimeBoundary extends Component<
  { renderer: NativeRenderer; children: ReactNode },
  { error: string | null }
> {
  state: { error: string | null } = { error: null }
  private stop?: () => void
  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? (error.stack ?? error.message) : String(error) }
  }
  componentDidMount() {
    this.stop = listenForRuntimeErrors(this.props.renderer, (error) =>
      this.setState(RuntimeBoundary.getDerivedStateFromError(error)),
    )
  }
  componentWillUnmount() {
    this.stop?.()
  }
  render() {
    if (this.state.error === null) return this.props.children
    return createElement(
      "div",
      {
        role: "alert",
        "aria-label": "Application runtime error",
        style: {
          width: "100%",
          height: "100%",
          padding: 20,
          backgroundColor: "#240e16",
          color: "#ffccd5",
          overflow: "scroll",
        },
      },
      createElement("text", null, "Application runtime error"),
      createElement("text", null, this.state.error),
    )
  }
}
export function createGpuiRuntime(
  factory: (onEvent?: (event: EventPayload) => void) => NativeRenderer,
) {
  let current: GpuiWindowRoot | undefined
  function createWindow(node: ReactNode, options: RenderOptions = {}) {
    const {
      renderer: injected,
      onEvent,
      onKeyDown,
      onKeyUp,
      debugFrameOverlay,
      frameMs,
      ...windowOptions
    } = options
    const native = injected ?? factory(onEvent)
    if (native.isInitialized?.() !== true) native.init?.(windowOptions)
    const host = createGpuiRenderer(native)
    let keyId = allocateRendererId(native)
    if (onKeyDown) registerEventHandler(keyId, "windowKeyDown", onKeyDown, native)
    if (onKeyUp) registerEventHandler(keyId, "windowKeyUp", onKeyUp, native)
    native.setWindowKeyEvents?.(!!onKeyDown, !!onKeyUp, keyId)
    if (debugFrameOverlay !== undefined) native.setDebugFrameOverlay?.(debugFrameOverlay)
    const unbindKeys = () => {
      unregisterEventHandlers(keyId, native)
      native.setWindowKeyEvents?.(false, false, keyId)
    }
    const renderNode = (node: ReactNode, nextOptions?: RenderOptions) => {
      if (nextOptions) {
        unbindKeys()
        keyId = allocateRendererId(native)
        if (nextOptions.onKeyDown)
          registerEventHandler(keyId, "windowKeyDown", nextOptions.onKeyDown, native)
        if (nextOptions.onKeyUp)
          registerEventHandler(keyId, "windowKeyUp", nextOptions.onKeyUp, native)
        native.setWindowKeyEvents?.(!!nextOptions.onKeyDown, !!nextOptions.onKeyUp, keyId)
        if (nextOptions.debugFrameOverlay !== undefined)
          native.setDebugFrameOverlay?.(nextOptions.debugFrameOverlay)
      }
      host.render(createElement(RuntimeBoundary, { renderer: native, children: node }))
    }
    try {
      renderNode(node)
    } catch (error) {
      host.destroy()
      unregisterEventHandlers(keyId, native)
      if (!injected) native.close?.()
      throw error
    }
    const loop = injected
      ? { stop() {} }
      : startFrameLoop(native, {
          ...(frameMs === undefined ? {} : { frameMs }),
          keepAlive: windowOptions.headless !== true,
        })
    let closed = false
    return {
      host,
      renderer: host.renderer,
      render: renderNode,
      get closed() {
        return closed
      },
      unmount() {
        if (!closed) {
          unbindKeys()
          host.render(null)
        }
      },
      close() {
        if (closed) return
        closed = true
        loop.stop()
        unbindKeys()
        host.destroy()
        clearEventHandlers(native)
        native.close?.()
      },
    }
  }
  function render(node: ReactNode, options: RenderOptions = {}) {
    if (current?.closed) current = undefined
    if (current && options.renderer && current.host.nativeRenderer !== options.renderer)
      throw new Error("render() cannot replace an existing native window renderer")
    if (current) current.render(node, options)
    else current = createWindow(node, options)
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
