import type { RendererOptions } from "@vue/runtime-core"

import { isEventProp, patchEvent, unregisterEventHandlers } from "./events.js"
import type { NativeNodeId, NativeRenderer } from "./native.js"
import type { GpuiComment, GpuiContainer, GpuiElement, GpuiNode, GpuiText } from "./nodes.js"
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
const UNIVERSAL_PROPS = new Set(["autoFocus", "tabIndex", "motion", "testId"])
const RESERVED_PROPS = new Set(["style", "class", "className", "children", "key", "ref"])

export type GpuiNodeOps = Omit<RendererOptions<GpuiNode, GpuiContainer>, "patchProp">

export type AllocateNodeId = () => NativeNodeId

function assertElementType(tag: string): asserts tag is GpuiElementType {
  if (!ELEMENT_TYPE_SET.has(tag)) {
    throw new Error(`Unsupported GPUI element tag: ${tag}`)
  }
}

export function createNodeOps(renderer: NativeRenderer, allocateId: AllocateNodeId): GpuiNodeOps {
  const createText = (text: string): GpuiText => {
    const id = allocateId()
    renderer.createElement(id, "text")
    renderer.setText(id, text)
    return { id, renderer, kind: "text", parent: null, text }
  }

  const remove = (child: GpuiNode): void => {
    const parent = child.parent
    if (parent !== null) {
      renderer.removeChild(parent.id, child.id)
      const index = parent.children.indexOf(child)
      if (index !== -1) parent.children.splice(index, 1)
      child.parent = null
    }
    cleanupHandlers(child)
    renderer.destroyElement(child.id)
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

    if (anchor === null) renderer.appendChild(parent.id, child.id)
    else renderer.insertBefore(parent.id, child.id, anchor.id)

    if (child.parent !== null) {
      const oldIndex = child.parent.children.indexOf(child)
      if (oldIndex !== -1) child.parent.children.splice(oldIndex, 1)
    }
    const index = anchor === null ? parent.children.length : parent.children.indexOf(anchor)
    parent.children.splice(index, 0, child)
    child.parent = parent
  }

  return {
    insert,
    remove,

    createElement(tag): GpuiElement {
      assertElementType(tag)
      const id = allocateId()
      renderer.createElement(id, tag)
      return {
        id,
        renderer,
        kind: "element",
        tag,
        parent: null,
        children: [],
        props: {},
      }
    },

    createText,

    createComment(text): GpuiComment {
      const id = allocateId()
      renderer.createElement(id, "text")
      renderer.setText(id, "")
      return { id, renderer, kind: "comment", parent: null, text }
    },

    setText(node, text): void {
      if (node.kind !== "text" && node.kind !== "comment") {
        throw new Error("setText expects a text or comment host node")
      }
      renderer.setText(node.id, node.kind === "comment" ? "" : text)
      node.text = text
    },

    setElementText(element, text): void {
      for (const child of element.children.slice()) remove(child)
      if (text !== "") insert(createText(text), element)
    },

    parentNode(node): GpuiContainer | null {
      return node.parent
    },

    nextSibling(node): GpuiNode | null {
      if (node.parent === null) return null
      const index = node.parent.children.indexOf(node)
      return node.parent.children[index + 1] ?? null
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
      renderer.setStyle(element.id, style)
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

    if (!BUILT_IN_TYPES.has(element.tag) || UNIVERSAL_PROPS.has(key)) {
      renderer.setCustomProp(element.id, key, serializeCustomProp(next))
    }
    setLocalProp(element, key, next)
  }
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
    for (const child of node.children) cleanupHandlers(child)
  }
}
