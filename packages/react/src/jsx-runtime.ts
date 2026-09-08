export { jsx, jsxs, Fragment } from "react/jsx-runtime"
import type * as React from "react"

import type { GpuiIntrinsicElements } from "./types.js"
export namespace JSX {
  export type Element = React.JSX.Element
  export type ElementType = keyof GpuiIntrinsicElements | React.JSXElementConstructor<any>
  export interface ElementChildrenAttribute {
    children: {}
  }
  export interface ElementAttributesProperty {
    props: {}
  }
  export type LibraryManagedAttributes<C, P> = React.JSX.LibraryManagedAttributes<C, P>
  export interface IntrinsicAttributes extends React.Attributes {}
  export interface IntrinsicClassAttributes<T> extends React.ClassAttributes<T> {}
  export interface IntrinsicElements extends GpuiIntrinsicElements {}
}
