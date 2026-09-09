import { TestRenderer, type TestWindowOptions } from "@gpui-native/runtime/native-testing"
import {
  nextTick,
  defineComponent,
  h,
  isVNode,
  type App,
  type Component,
  type VNode,
} from "@vue/runtime-core"

import { createGpuiRenderer, type GpuiRendererHost } from "./renderer.js"
export * from "@gpui-native/runtime/native-testing"
export interface NativeGpuiTestRoot {
  readonly host: GpuiRendererHost
  readonly renderer: TestRenderer
  render(node: Component | VNode): void
  flush(): Promise<void>
  unmount(): void
}

/** Creates a Vue root backed by GPUI's native GPU test application. */
export function createTestRoot(options: TestWindowOptions = {}): NativeGpuiTestRoot {
  const renderer = new TestRenderer(options)
  const host = createGpuiRenderer(renderer)
  let app: App | null = null

  const unmount = (): void => {
    app?.unmount()
    app = null
    host.flushMutations()
  }

  return {
    host,
    renderer,
    render(node): void {
      unmount()
      const Root = defineComponent({
        name: "GpuiNativeTestRoot",
        setup: () => () => (isVNode(node) ? node : h(node)),
      })
      app = host.mount(Root)
      renderer.flush()
    },
    async flush(): Promise<void> {
      await nextTick()
      host.flushMutations()
      renderer.flush()
    },
    unmount,
  }
}
