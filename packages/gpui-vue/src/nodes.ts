import type { NativeNodeId, NativeRenderer } from "./native.js"
import type { GpuiElementType } from "./types.js"

interface GpuiNodeBase {
  readonly id: NativeNodeId
  readonly renderer: NativeRenderer
  parent: GpuiContainer | null
}

/** Synthetic full-window div used to support Vue root fragments. */
export interface GpuiRoot extends GpuiNodeBase {
  readonly kind: "root"
  readonly children: GpuiNode[]
}

export interface GpuiElement extends GpuiNodeBase {
  readonly kind: "element"
  readonly tag: GpuiElementType
  readonly children: GpuiNode[]
  readonly props: Record<string, unknown>
}

export interface GpuiText extends GpuiNodeBase {
  readonly kind: "text"
  text: string
}

export interface GpuiComment extends GpuiNodeBase {
  readonly kind: "comment"
  text: string
}

export type GpuiContainer = GpuiRoot | GpuiElement
export type GpuiNode = GpuiContainer | GpuiText | GpuiComment
export type GpuiPublicInstance = GpuiElement

export function createGpuiRoot(renderer: NativeRenderer, id: NativeNodeId): GpuiRoot {
  return {
    id,
    renderer,
    kind: "root",
    parent: null,
    children: [],
  }
}
