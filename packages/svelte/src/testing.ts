import { handleGpuiEvent } from "@gpui-native/runtime/events"
import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import { TestRenderer, type TestWindowOptions } from "@gpui-native/runtime/native-testing"
import type { Component } from "svelte"

import { api } from "./engine.js"
import {
  createGpuiRenderer,
  type GpuiRendererHost,
  type GpuiElement,
  type GpuiNode,
} from "./renderer.js"
import type { EventPayload } from "./types.js"
export {
  TestRenderer,
  hasNativeTestRenderer,
  type TestWindowOptions,
  type NativeTestElement,
} from "@gpui-native/runtime/native-testing"
function nodeText(node: GpuiNode): string {
  return node.kind === "text"
    ? node.text
    : node.kind === "comment"
      ? ""
      : node.children.map(nodeText).join("")
}
export class GpuiTestElement {
  constructor(readonly node: GpuiElement) {}
  get id() {
    return this.node.id
  }
  get tag() {
    return this.node.tag
  }
  get text() {
    return nodeText(this.node)
  }
  prop<T = unknown>(name: string): T | undefined {
    return this.node.props[name] as T | undefined
  }
  trigger(eventType: string, payload: Partial<EventPayload> = {}) {
    handleGpuiEvent({ ...payload, elementId: this.id, eventType }, this.node.renderer)
  }
  click(payload: Partial<EventPayload> = {}) {
    this.trigger("click", { clickCount: 1, ...payload })
  }
}
function queries(host: GpuiRendererHost) {
  const findAll = (predicate: (node: GpuiTestElement) => boolean = () => true) => {
    const result: GpuiTestElement[] = []
    const visit = (node: GpuiNode) => {
      if (node.kind !== "element" && node.kind !== "root") return
      if (node.kind === "element") {
        const element = new GpuiTestElement(node)
        if (predicate(element)) result.push(element)
      }
      node.children.forEach(visit)
    }
    host.root.children.forEach(visit)
    return result
  }
  const queryByTestId = (id: string) => findAll((node) => node.prop("testId") === id)[0] ?? null
  return {
    findAll,
    queryByTestId,
    findByTestId(id: string) {
      const node = queryByTestId(id)
      if (!node) throw new Error(`No GPUI element has testId ${JSON.stringify(id)}`)
      return node
    },
    findByText(text: string | RegExp) {
      const node = findAll((node) =>
        typeof text === "string" ? node.text === text : text.test(node.text),
      ).at(-1)
      if (!node) throw new Error(`No GPUI element matches ${String(text)}`)
      return node
    },
  }
}
export function mountGpui<
  Props extends Record<string, unknown>,
  Exports extends Record<string, unknown>,
>(component: Component<Props, Exports>, props: Props = {} as Props) {
  const renderer = new MemoryNativeRenderer(),
    host = createGpuiRenderer(renderer)
  try {
    const instance = host.render(component, props)
    return {
      host,
      renderer,
      instance,
      ...queries(host),
      render: host.render,
      async flush() {
        await api.tick()
        host.flush()
      },
      unmount: host.destroy,
    }
  } catch (error) {
    host.destroy()
    throw error
  }
}
export function createTestRoot(options: TestWindowOptions = {}) {
  const renderer = new TestRenderer(options),
    host = createGpuiRenderer(renderer)
  return {
    host,
    renderer,
    ...queries(host),
    render<Props extends Record<string, unknown>, Exports extends Record<string, unknown>>(
      component: Component<Props, Exports>,
      props: Props = {} as Props,
    ) {
      const instance = host.render(component, props)
      renderer.flush()
      return instance
    },
    async flush() {
      await api.tick()
      renderer.dispatchNativeEvents()
      host.flush()
      renderer.flush()
    },
    unmount() {
      host.destroy()
      renderer.flush()
    },
  }
}
export type GpuiTestRoot = ReturnType<typeof mountGpui>
export type NativeGpuiTestRoot = ReturnType<typeof createTestRoot>
