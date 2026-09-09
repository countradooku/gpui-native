import { allocateRendererId, claimRenderer } from "@gpui-native/runtime/ownership"
import {
  createRenderer,
  type App,
  type Component,
  type CreateAppFunction,
  type Renderer,
  type VNode,
} from "@vue/runtime-core"

import { wrapWithBatching, type BatchingRenderer } from "./batching.js"
import { GpuiRendererKey } from "./context.js"
import { MemoryNativeRenderer, type NativeRenderer } from "./native.js"
import { createNodeOps, createPatchProp } from "./nodeOps.js"
import { createGpuiRoot, type GpuiContainer, type GpuiNode } from "./nodes.js"
export { allocateRendererId } from "@gpui-native/runtime/ownership"

export interface GpuiRendererHost {
  /** The caller-provided first-party Rust renderer (or memory renderer in tests). */
  readonly nativeRenderer: NativeRenderer
  /** Auto-batched renderer used by Vue and exposed to application composables. */
  readonly renderer: BatchingRenderer
  readonly root: GpuiContainer
  readonly vueRenderer: Renderer<GpuiContainer>
  readonly createApp: CreateAppFunction<GpuiContainer>
  render(vnode: VNode | null, container?: GpuiContainer): void
  mount(component: Component, props?: Record<string, unknown>): App<GpuiContainer>
  flushMutations(): number[]
  destroy(): void
}

/** Creates one Vue renderer bound to a gpui-vue native retained tree. */
export function createGpuiRenderer(
  nativeRenderer: NativeRenderer = new MemoryNativeRenderer(),
): GpuiRendererHost {
  const release = claimRenderer(nativeRenderer, "Vue root")

  try {
    const renderer = wrapWithBatching(nativeRenderer)
    const allocateId = (): number => allocateRendererId(nativeRenderer)

    // A native root div lets Vue fragments have multiple top-level host nodes.
    const rootId = allocateId()
    renderer.createElement(rootId, "div")
    renderer.setStyle(rootId, { width: "100%", height: "100%" })
    renderer.setRoot(rootId)
    const root = createGpuiRoot(renderer, rootId)

    const nodeOps = createNodeOps(renderer, allocateId)
    const vueRenderer = createRenderer<GpuiNode, GpuiContainer>({
      ...nodeOps,
      patchProp: createPatchProp(renderer),
    })

    const render = (vnode: VNode | null, container = root): void => {
      vueRenderer.render(vnode, container)
      renderer.flushMutations()
    }

    const mount = (component: Component, props?: Record<string, unknown>): App<GpuiContainer> => {
      const app = vueRenderer.createApp(component, props)
      app.provide(GpuiRendererKey, renderer)
      app.mount(root)
      renderer.flushMutations()
      return app
    }

    let destroyed = false
    return {
      nativeRenderer,
      renderer,
      root,
      vueRenderer,
      createApp: vueRenderer.createApp,
      render,
      mount,
      flushMutations: () => renderer.flushMutations(),
      destroy(): void {
        if (destroyed) return
        let cleaned = false
        try {
          vueRenderer.render(null, root)
          renderer.flushMutations()
          renderer.destroyElement(root.id)
          renderer.flushMutations()
          cleaned = true
          destroyed = true
        } finally {
          if (cleaned) release()
        }
      },
    }
  } catch (error) {
    release()
    throw error
  }
}
