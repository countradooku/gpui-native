import type { NativeNodeId, NativeRenderer } from "./native.js"
import type { GpuiElementType } from "./types.js"

interface GpuiNodeBase {
  readonly id: NativeNodeId
  readonly renderer: NativeRenderer
  parent: GpuiContainer | null
  previousSibling: GpuiNode | null
  nextSibling: GpuiNode | null
}

/** Synthetic full-window div used to support Vue root fragments. */
export interface GpuiRoot extends GpuiNodeBase {
  readonly kind: "root"
  firstChild: GpuiNode | null
  lastChild: GpuiNode | null
  readonly children: readonly GpuiNode[]
}

export interface GpuiElement extends GpuiNodeBase {
  readonly kind: "element"
  readonly tag: GpuiElementType
  firstChild: GpuiNode | null
  lastChild: GpuiNode | null
  readonly children: readonly GpuiNode[]
  readonly props: Record<string, unknown>
}

export interface GpuiText extends GpuiNodeBase {
  readonly kind: "text"
  /** Empty Vue fragment sentinels stay JS-only until they gain content. */
  native: boolean
  text: string
}

export interface GpuiComment extends GpuiNodeBase {
  readonly kind: "comment"
  text: string
}

export type GpuiContainer = GpuiRoot | GpuiElement
export type GpuiNode = GpuiContainer | GpuiText | GpuiComment
export type GpuiPublicInstance = GpuiElement

export function materializeChildren(container: GpuiContainer): GpuiNode[] {
  const children: GpuiNode[] = []
  let child = container.firstChild
  while (child !== null) {
    children.push(child)
    child = child.nextSibling
  }
  return children
}

export function createGpuiRoot(renderer: NativeRenderer, id: NativeNodeId): GpuiRoot {
  const root: GpuiRoot = {
    id,
    renderer,
    kind: "root",
    parent: null,
    previousSibling: null,
    nextSibling: null,
    firstChild: null,
    lastChild: null,
    get children() {
      return materializeChildren(root)
    },
  }
  return root
}
