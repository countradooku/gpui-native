import { MemoryNativeRenderer, type NativeRenderer } from "@gpui-native/runtime/native"
import { claimRenderer } from "@gpui-native/runtime/ownership"
import type { Component } from "svelte"

import { api, proxy } from "./engine.js"
import { HostController } from "./host.js"
export const GpuiRendererKey = Symbol("gpui.svelte.renderer")
export function createGpuiRenderer(nativeRenderer: NativeRenderer = new MemoryNativeRenderer()) {
  const release = claimRenderer(nativeRenderer, "Svelte root")
  let controller: HostController
  try {
    controller = new HostController(nativeRenderer)
  } catch (error) {
    release()
    throw error
  }
  let instance: Record<string, unknown> | null = null
  let destroyed = false
  let mountedComponent: unknown
  let liveProps: Record<string, unknown> = {}
  function unmount() {
    mountedComponent = undefined
    if (instance) {
      const previous = instance
      instance = null
      api.flushSync(() => {
        void api.unmount(previous)
      })
    }
    controller.flush()
  }
  return {
    nativeRenderer,
    renderer: controller.renderer,
    root: controller.root,
    render<Props extends Record<string, unknown>, Exports extends Record<string, unknown>>(
      component: Component<Props, Exports> | null,
      props: Props = {} as Props,
    ): Exports | null {
      if (destroyed) throw new Error("Cannot render into a destroyed GPUI Svelte root")
      if (component && component === mountedComponent) {
        api.flushSync(() => {
          for (const key of Object.keys(liveProps))
            if (!Object.hasOwn(props, key)) delete liveProps[key]
          Object.assign(liveProps, props)
        })
        controller.flush()
        return instance as Exports | null
      }
      unmount()
      if (!component) return null
      try {
        liveProps = proxy({ ...props })
        mountedComponent = component
        api.flushSync(() => {
          instance = api.mount(component, {
            target: controller.target as unknown as HTMLElement,
            props: liveProps as Props,
            context: new Map([[GpuiRendererKey, controller.renderer]]),
          })
        })
        controller.flush()
        return instance as Exports | null
      } catch (error) {
        unmount()
        throw error
      }
    },
    flush() {
      api.flushSync()
      controller.flush()
    },
    flushMutations() {
      controller.flush()
    },
    destroy() {
      if (destroyed) return
      try {
        unmount()
      } finally {
        destroyed = true
        try {
          controller.dispose()
        } finally {
          release()
        }
      }
    },
  }
}
export type GpuiRendererHost = ReturnType<typeof createGpuiRenderer>
export type { GpuiElement, GpuiNode, GpuiPublicInstance } from "@gpui-native/runtime/nodes"
