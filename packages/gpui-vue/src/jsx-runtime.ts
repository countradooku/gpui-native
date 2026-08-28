import {
  Fragment,
  h,
  type Component,
  type VNode,
  type VNodeChild,
  type VNodeProps,
} from "@vue/runtime-core"

import type { GpuiIntrinsicElements } from "./types.js"

export { Fragment }

type AutomaticJsxProps = VNodeProps &
  Record<string, unknown> & {
    children?: VNodeChild
  }

/** Adapt the automatic JSX runtime ABI to Vue's `h(type, props, children)`. */
export function jsx(
  type: string | Component,
  rawProps: AutomaticJsxProps | null,
  key?: string | number,
): VNode {
  const props: Record<string, unknown> = { ...rawProps }
  const children = props.children as VNodeChild | undefined
  delete props.children
  if (key !== undefined) props.key = key
  return h(type, props, (children === null ? undefined : children) as never)
}

export const jsxs = jsx
export const jsxDEV = jsx

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
