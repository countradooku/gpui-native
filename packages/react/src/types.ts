import type { GpuiPublicInstance } from "@gpui-native/runtime/nodes"
import type * as Native from "@gpui-native/runtime/types"
import type { ReactNode, Ref, Key } from "react"
export * from "@gpui-native/runtime/types"
type ReactProps<T> = T extends unknown
  ? Omit<T, "key"> & { key?: Key; ref?: Ref<GpuiPublicInstance>; children?: ReactNode }
  : never
export type HostProps = ReactProps<Native.HostProps>
export type ImgProps = ReactProps<Native.ImgProps>
export type SvgProps = ReactProps<Native.SvgProps>
export type CanvasProps = ReactProps<Native.CanvasProps>
export type AnchoredProps = ReactProps<Native.AnchoredProps>
export type CodeProps = ReactProps<Native.CodeProps>
export type DiffProps = ReactProps<Native.DiffProps>
export type MarkdownProps = ReactProps<Native.MarkdownProps>
export type InputProps = ReactProps<Native.InputProps>
export type TextareaProps = ReactProps<Native.TextareaProps>
export type VirtualListProps = ReactProps<Native.VirtualListProps>
export interface GpuiIntrinsicElements {
  div: HostProps
  text: HostProps
  img: ImgProps
  svg: SvgProps
  canvas: CanvasProps
  anchored: AnchoredProps
  code: CodeProps
  diff: DiffProps
  markdown: MarkdownProps
  input: InputProps
  textarea: TextareaProps
  "virtual-list": VirtualListProps
}
