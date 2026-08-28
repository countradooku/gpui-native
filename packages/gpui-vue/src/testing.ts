import { nextTick, type App, type Component } from "@vue/runtime-core"

import { handleGpuiEvent } from "./events.js"
import { MemoryNativeRenderer } from "./native.js"
import type { GpuiElement, GpuiNode } from "./nodes.js"
import { createGpuiRenderer, type GpuiRendererHost } from "./renderer.js"
import type { EventPayload } from "./types.js"

export {
  createTestRoot,
  hasNativeTestRenderer,
  TestRenderer,
  type NativeGpuiTestRoot,
  type NativeTestElement,
  type TestWindowOptions,
} from "./native-testing.js"

export class GpuiTestElement {
  constructor(readonly node: GpuiElement) {}

  get id(): number {
    return this.node.id
  }

  get tag(): string {
    return this.node.tag
  }

  get text(): string {
    return nodeText(this.node)
  }

  prop<T = unknown>(name: string): T | undefined {
    return this.node.props[name] as T | undefined
  }

  trigger(eventType: string, payload: Partial<EventPayload> = {}): void {
    handleGpuiEvent(
      {
        ...payload,
        elementId: this.node.id,
        eventType,
      },
      this.node.renderer,
    )
  }

  click(payload: Partial<EventPayload> = {}): void {
    this.trigger("click", { clickCount: 1, ...payload })
  }
}

export interface GpuiTestRoot {
  readonly app: App
  readonly host: GpuiRendererHost
  readonly renderer: MemoryNativeRenderer
  flush(): Promise<void>
  findByTestId(testId: string): GpuiTestElement
  queryByTestId(testId: string): GpuiTestElement | null
  findByText(text: string | RegExp): GpuiTestElement
  findAll(predicate?: (element: GpuiTestElement) => boolean): GpuiTestElement[]
  unmount(): void
}

/** Mounts a component against the exact NodeOps pipeline with no native window. */
export function mountGpui(component: Component, props?: Record<string, unknown>): GpuiTestRoot {
  const renderer = new MemoryNativeRenderer()
  const host = createGpuiRenderer(renderer)
  const app = host.mount(component, props)

  const findAll = (
    predicate: (element: GpuiTestElement) => boolean = () => true,
  ): GpuiTestElement[] =>
    collectElements(host.root.children)
      .map((node) => new GpuiTestElement(node))
      .filter(predicate)

  const queryByTestId = (testId: string): GpuiTestElement | null =>
    findAll((element) => element.prop("testId") === testId)[0] ?? null

  return {
    app,
    host,
    renderer,
    async flush(): Promise<void> {
      await nextTick()
      host.flushMutations()
    },
    findByTestId(testId): GpuiTestElement {
      const element = queryByTestId(testId)
      if (element === null) {
        throw new Error(`No GPUI element has testId ${JSON.stringify(testId)}`)
      }
      return element
    },
    queryByTestId,
    findByText(text): GpuiTestElement {
      const elements = findAll((element) =>
        typeof text === "string" ? element.text === text : text.test(element.text),
      )
      const element = elements.at(-1)
      if (element === undefined) {
        throw new Error(`No GPUI element matches text ${String(text)}`)
      }
      return element
    },
    findAll,
    unmount(): void {
      app.unmount()
      host.flushMutations()
    },
  }
}

function collectElements(nodes: readonly GpuiNode[]): GpuiElement[] {
  const elements: GpuiElement[] = []
  for (const node of nodes) {
    if (node.kind !== "element") continue
    elements.push(node, ...collectElements(node.children))
  }
  return elements
}

function nodeText(node: GpuiNode): string {
  if (node.kind === "text") return node.text
  if (node.kind === "comment") return ""
  return node.children.map(nodeText).join("")
}
