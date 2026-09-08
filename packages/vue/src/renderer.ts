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

const rendererStateKey = Symbol.for("gpui-native.vue.renderers")
const rendererState = (Reflect.get(globalThis, rendererStateKey) as
  | {
      owners: WeakMap<NativeRenderer, symbol>
      ids: WeakMap<NativeRenderer, number>
    }
  | undefined) ?? { owners: new WeakMap(), ids: new WeakMap() }
Reflect.set(globalThis, rendererStateKey, rendererState)
const rendererOwners = rendererState.owners

export function allocateRendererId(renderer: NativeRenderer): number {
  const id = (rendererState.ids.get(renderer) ?? 0) + 1
  rendererState.ids.set(renderer, id)
  return id
}

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
  if (rendererOwners.has(nativeRenderer)) {
    throw new Error("A native GPUI renderer can only own one live Vue root")
  }
  const owner = Symbol("gpui-vue-root")
  rendererOwners.set(nativeRenderer, owner)

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
          if (cleaned && rendererOwners.get(nativeRenderer) === owner) {
            rendererOwners.delete(nativeRenderer)
          }
        }
      },
    }
  } catch (error) {
    if (rendererOwners.get(nativeRenderer) === owner) rendererOwners.delete(nativeRenderer)
    throw error
  }
}
