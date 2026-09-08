import type { NativeNodeId } from "./native.js"
import type { StyleDesc } from "./types.js"

/** Shared mutation protocol for adapters whose backend accepts atomic tuples. */
export abstract class MutationRenderer {
  protected abstract applyMutations(mutations: unknown[][]): NativeNodeId[]

  createElement(id: NativeNodeId, elementType: string): void {
    this.applyMutations([["createElement", id, elementType]])
  }

  destroyElement(id: NativeNodeId): NativeNodeId[] {
    return this.applyMutations([["destroyElement", id]])
  }

  appendChild(parentId: NativeNodeId, childId: NativeNodeId): void {
    this.applyMutations([["appendChild", parentId, childId]])
  }

  removeChild(parentId: NativeNodeId, childId: NativeNodeId): void {
    this.applyMutations([["removeChild", parentId, childId]])
  }

  insertBefore(parentId: NativeNodeId, childId: NativeNodeId, beforeId: NativeNodeId): void {
    this.applyMutations([["insertBefore", parentId, childId, beforeId]])
  }

  setStyle(id: NativeNodeId, style: string | StyleDesc | Record<string, unknown>): void {
    this.applyMutations([["setStyle", id, style]])
  }

  setText(id: NativeNodeId, content: string): void {
    this.applyMutations([["setText", id, content]])
  }

  setEventListener(id: NativeNodeId, eventType: string, hasHandler: boolean): void {
    this.applyMutations([["setEventListener", id, eventType, hasHandler]])
  }

  setRoot(id: NativeNodeId): void {
    this.applyMutations([["setRoot", id]])
  }

  setCustomProp(
    id: NativeNodeId,
    key: string,
    value: string | object | number | boolean | null,
  ): void {
    this.applyMutations([["setCustomPropValue", id, key, value]])
  }
}
