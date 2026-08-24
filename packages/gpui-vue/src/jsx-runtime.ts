import type { VNode, VNodeProps } from "@vue/runtime-core"

import type { GpuiIntrinsicElements } from "./types.js"

export { Fragment, h as jsx, h as jsxDEV, h as jsxs } from "@vue/runtime-core"

/** JSX/template intrinsic types for GPUI's native host element set. */
export namespace JSX {
  export interface Element extends VNode {}
  export interface ElementClass {
    $props: Record<string, unknown>
  }
  export interface ElementAttributesProperty {
    $props: Record<string, unknown>
  }
  export interface IntrinsicElements extends GpuiIntrinsicElements {}
  export interface IntrinsicAttributes extends VNodeProps {}
}
