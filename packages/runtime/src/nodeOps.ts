import { isEventProp, patchEvent, unregisterEventHandlers } from "./events.js"
import type { NativeNodeId, NativeRenderer } from "./native.js"
import {
  materializeChildren,
  type GpuiComment,
  type GpuiContainer,
  type GpuiElement,
  type GpuiNode,
  type GpuiText,
} from "./nodes.js"
import type { GpuiElementType, StyleDesc } from "./types.js"

export const GPUI_ELEMENT_TYPES = [
  "div",
  "text",
  "img",
  "svg",
  "canvas",
  "input",
  "textarea",
  "anchored",
  "code",
  "diff",
  "markdown",
  "virtual-list",
] as const satisfies readonly GpuiElementType[]

const ELEMENT_TYPE_SET = new Set<string>(GPUI_ELEMENT_TYPES)
const BUILT_IN_TYPES = new Set<GpuiElementType>(["div", "text"])
const UNIVERSAL_PROPS = new Set(["autoFocus", "tabIndex", "motion", "testId", "highlight"])
const RESERVED_PROPS = new Set(["style", "class", "className", "children", "key", "ref"])

export interface GpuiNodeOps {
  insert(child: GpuiNode, parent: GpuiContainer, anchor?: GpuiNode | null): void
  remove(child: GpuiNode): void
  createElement(tag: string, ...args: unknown[]): GpuiElement
  createText(text: string): GpuiText
  createComment(text: string): GpuiComment
  setText(node: GpuiNode, text: string): void
  setElementText(element: GpuiContainer, text: string): void
  parentNode(node: GpuiNode): GpuiContainer | null
  nextSibling(node: GpuiNode): GpuiNode | null
  setScopeId(element: GpuiElement, id: string): void
}

export type AllocateNodeId = () => NativeNodeId

function assertElementType(tag: string): asserts tag is GpuiElementType {
  if (!ELEMENT_TYPE_SET.has(tag)) {
    throw new Error(`Unsupported GPUI element tag: ${tag}`)
  }
}

export function createNodeOps(renderer: NativeRenderer, allocateId: AllocateNodeId): GpuiNodeOps {
  const createText = (text: string): GpuiText => {
    const id = allocateId()
    const native = text !== ""
    if (native) {
      renderer.createElement(id, "text")
      renderer.setText(id, text)
    }
    return {
      id,
      renderer,
      kind: "text",
      native,
      parent: null,
      previousSibling: null,
      nextSibling: null,
      text,
    }
  }

  const detachLocal = (child: GpuiNode): void => {
    const parent = child.parent
    if (parent === null) return
    if (child.previousSibling !== null) child.previousSibling.nextSibling = child.nextSibling
    else parent.firstChild = child.nextSibling
    if (child.nextSibling !== null) child.nextSibling.previousSibling = child.previousSibling
    else parent.lastChild = child.previousSibling
    child.parent = null
    child.previousSibling = null
    child.nextSibling = null
  }

  const attachLocal = (child: GpuiNode, parent: GpuiContainer, anchor: GpuiNode | null): void => {
    const previous = anchor === null ? parent.lastChild : anchor.previousSibling
    child.parent = parent
    child.previousSibling = previous
    child.nextSibling = anchor
    if (previous !== null) previous.nextSibling = child
    else parent.firstChild = child
    if (anchor !== null) anchor.previousSibling = child
    else parent.lastChild = child
  }

  const nativeAnchorFor = (anchor: GpuiNode | null): GpuiNode | null => {
    let candidate = anchor
    while (candidate?.kind === "comment" || (candidate?.kind === "text" && !candidate.native)) {
      candidate = candidate.nextSibling
    }
    return candidate
  }

  const hasNativeNode = (node: GpuiNode): boolean =>
    node.kind !== "comment" && (node.kind !== "text" || node.native)

  const updateText = (node: GpuiText | GpuiComment, text: string): void => {
    if (node.kind === "text") {
      if (node.native && text === "") {
        renderer.destroyElement(node.id)
        node.native = false
      } else if (!node.native && text !== "") {
        renderer.createElement(node.id, "text")
        renderer.setText(node.id, text)
        node.native = true
        if (node.parent !== null) {
          const anchor = nativeAnchorFor(node.nextSibling)
          if (anchor === null) renderer.appendChild(node.parent.id, node.id)
          else renderer.insertBefore(node.parent.id, node.id, anchor.id)
        }
      } else if (node.native) {
        renderer.setText(node.id, text)
      }
    }
    node.text = text
  }

  const remove = (child: GpuiNode): void => {
    const parent = child.parent
    if (parent !== null) detachLocal(child)
    cleanupHandlers(child)
    if (hasNativeNode(child)) renderer.destroyElement(child.id)
  }

  const insert = (child: GpuiNode, parent: GpuiContainer, anchor: GpuiNode | null = null): void => {
    if (
      child.renderer !== renderer ||
      parent.renderer !== renderer ||
      (anchor !== null && anchor.renderer !== renderer)
    ) {
      throw new Error("Cannot move GPUI nodes between native renderers")
    }
    if (anchor !== null && anchor.parent !== parent) {
      throw new Error("The insert anchor must be a child of the target parent")
    }
    if (anchor === child && child.parent === parent) return

    if (hasNativeNode(child)) {
      const nativeAnchor = nativeAnchorFor(anchor)
      if (nativeAnchor === child) {
        // Moving across virtual comment anchors can leave the native order
        // unchanged even though the JS sibling links change.
      } else if (nativeAnchor === null) renderer.appendChild(parent.id, child.id)
      else renderer.insertBefore(parent.id, child.id, nativeAnchor.id)
    }
    detachLocal(child)
    attachLocal(child, parent, anchor)
  }

  return {
    insert,
    remove,

    createElement(tag): GpuiElement {
      assertElementType(tag)
      const id = allocateId()
      renderer.createElement(id, tag)
      const element: GpuiElement = {
        id,
        renderer,
        kind: "element",
        tag,
        parent: null,
        previousSibling: null,
        nextSibling: null,
        firstChild: null,
        lastChild: null,
        get children() {
          return materializeChildren(element)
        },
        props: {},
      }
      return element
    },

    createText,

    createComment(text): GpuiComment {
      const id = allocateId()
      return {
        id,
        renderer,
        kind: "comment",
        parent: null,
        previousSibling: null,
        nextSibling: null,
        text,
      }
    },

    setText(node, text): void {
      if (node.kind !== "text" && node.kind !== "comment") {
        throw new Error("setText expects a text or comment host node")
      }
      updateText(node, text)
    },

    setElementText(element, text): void {
      const onlyChild = element.firstChild === element.lastChild ? element.firstChild : null
      if (onlyChild?.kind === "text" && text !== "") {
        if (onlyChild.text !== text) updateText(onlyChild, text)
        return
      }
      let child = element.firstChild
      while (child !== null) {
        const next = child.nextSibling
        remove(child)
        child = next
      }
      if (text !== "") insert(createText(text), element)
    },

    parentNode(node): GpuiContainer | null {
      return node.parent
    },

    nextSibling(node): GpuiNode | null {
      return node.nextSibling
    },

    setScopeId(element, id): void {
      if (element.kind === "element") element.props[id] = true
    },
  }
}

export function createPatchProp(renderer: NativeRenderer) {
  return (element: GpuiContainer, key: string, previous: unknown, next: unknown): void => {
    if (element.kind !== "element") {
      throw new Error("Properties can only be patched on GPUI elements")
    }

    if (key === "style") {
      const style = normalizeStyle(next)
      if (!shallowEqualStyle(normalizeStyle(previous), style)) renderer.setStyle(element.id, style)
      setLocalProp(element, key, next)
      return
    }

    if (key === "motion" && equalJsonValue(previous, next)) {
      setLocalProp(element, key, next)
      return
    }

    if (isEventProp(key)) {
      patchEvent(renderer, element.id, key, previous, next)
      setLocalProp(element, key, next)
      return
    }

    if (RESERVED_PROPS.has(key)) {
      setLocalProp(element, key, next)
      return
    }

    if (
      !BUILT_IN_TYPES.has(element.tag) ||
      UNIVERSAL_PROPS.has(key) ||
      key === "role" ||
      key.startsWith("aria-")
    ) {
      renderer.setCustomProp(element.id, key, serializeCustomProp(next))
    }
    setLocalProp(element, key, next)
  }
}

function shallowEqualStyle(left: StyleDesc, right: StyleDesc): boolean {
  if (left === right) return true
  const leftEntries = Object.entries(left)
  const rightKeys = Object.keys(right)
  if (leftEntries.length !== rightKeys.length) return false
  return leftEntries.every(([key, value]) => Object.is(value, right[key as keyof StyleDesc]))
}

function equalJsonValue(left: unknown, right: unknown, depth = 0): boolean {
  if (Object.is(left, right)) return true
  if (depth >= 16 || left === null || right === null) return false
  if (Array.isArray(left)) {
    if (!Array.isArray(right) || left.length !== right.length) return false
    return left.every((value, index) => equalJsonValue(value, right[index], depth + 1))
  }
  if (typeof left !== "object" || typeof right !== "object" || Array.isArray(right)) return false
  const leftRecord = left as Record<string, unknown>
  const rightRecord = right as Record<string, unknown>
  const keys = Object.keys(leftRecord)
  if (keys.length !== Object.keys(rightRecord).length) return false
  return keys.every(
    (key) =>
      Object.hasOwn(rightRecord, key) &&
      equalJsonValue(leftRecord[key], rightRecord[key], depth + 1),
  )
}

function normalizeStyle(value: unknown): StyleDesc {
  if (value === null || value === undefined || value === false) return {}
  if (Array.isArray(value)) {
    return Object.assign({}, ...value.map(normalizeStyle)) as StyleDesc
  }
  if (typeof value !== "object") {
    throw new TypeError("GPUI styles must be objects or arrays of objects")
  }
  return value as StyleDesc
}

function serializeCustomProp(value: unknown): string | object | number | boolean | null {
  if (
    value === null ||
    value === undefined ||
    typeof value === "function" ||
    typeof value === "symbol" ||
    typeof value === "bigint"
  ) {
    return null
  }
  if (
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean" ||
    typeof value === "object"
  ) {
    return value
  }
  return null
}

function setLocalProp(element: GpuiElement, key: string, value: unknown): void {
  if (value === null || value === undefined) delete element.props[key]
  else element.props[key] = value
}

function cleanupHandlers(node: GpuiNode): void {
  unregisterEventHandlers(node.id, node.renderer)
  if (node.kind === "root" || node.kind === "element") {
    let child = node.firstChild
    while (child !== null) {
      cleanupHandlers(child)
      child = child.nextSibling
    }
  }
}
