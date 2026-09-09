import { handleGpuiEvent } from "@gpui-native/runtime/events"
import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import { TestRenderer, type TestWindowOptions } from "@gpui-native/runtime/native-testing"
import { act, type ReactNode } from "react"

import { createGpuiRenderer, type GpuiRendererHost, type GpuiElement } from "./renderer.js"
import type { EventPayload } from "./types.js"
export { act } from "react"
export {
  TestRenderer,
  hasNativeTestRenderer,
  type TestWindowOptions,
  type NativeTestElement,
} from "@gpui-native/runtime/native-testing"
export class GpuiTestElement {
  constructor(readonly node: GpuiElement) {}
  get id() {
    return this.node.id
  }
  get tag() {
    return this.node.tag
  }
  get text(): string {
    return String(
      this.node.props.__text ??
        this.node.children
          .map((node) =>
            node.kind === "element"
              ? new GpuiTestElement(node).text
              : node.kind === "text"
                ? node.text
                : "",
          )
          .join(""),
    )
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
  const findAll = (
    predicate: (element: GpuiTestElement) => boolean = () => true,
  ): GpuiTestElement[] => {
    const result: GpuiTestElement[] = []
    const visit = (node: GpuiElement) => {
      const element = new GpuiTestElement(node)
      if (predicate(element)) result.push(element)
      node.children.forEach((child) => {
        if (child.kind === "element") visit(child)
      })
    }
    host.root.children.forEach((child) => {
      if (child.kind === "element") visit(child)
    })
    return result
  }
  const queryByTestId = (id: string) =>
    findAll((element) => element.prop("testId") === id)[0] ?? null
  return {
    findAll,
    queryByTestId,
    findByTestId(id: string) {
      const element = queryByTestId(id)
      if (!element) throw new Error(`No GPUI element has testId ${JSON.stringify(id)}`)
      return element
    },
    findByText(text: string | RegExp) {
      const element = findAll((element) =>
        typeof text === "string" ? element.text === text : text.test(element.text),
      ).at(-1)
      if (!element) throw new Error(`No GPUI element matches text ${String(text)}`)
      return element
    },
  }
}
export function mountGpui(node: ReactNode) {
  const renderer = new MemoryNativeRenderer()
  const host = createGpuiRenderer(renderer)
  host.render(node)
  return {
    host,
    renderer,
    ...queries(host),
    render: host.render,
    async flush() {
      await act(async () => {})
      host.flush()
    },
    unmount: host.destroy,
  }
}
export function createTestRoot(options: TestWindowOptions = {}) {
  const renderer = new TestRenderer(options)
  const host = createGpuiRenderer(renderer)
  return {
    host,
    renderer,
    ...queries(host),
    render(node: ReactNode) {
      host.render(node)
      renderer.flush()
    },
    async flush() {
      await act(async () => {})
      await act(async () => renderer.dispatchNativeEvents())
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
