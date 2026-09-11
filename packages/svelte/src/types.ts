import type * as Native from "@gpui-native/runtime/types"
import type { Snippet } from "svelte"
export * from "@gpui-native/runtime/types"
export interface ElementRef {
  readonly id: number
}
type LowercaseEvents = {
  [
    K in keyof Native.HostProps as K extends `on${string}` ? Lowercase<K> : never
  ]: Native.HostProps[K]
}
type Props<T> = T & LowercaseEvents & { children?: Snippet; ref?: ElementRef | null }
export type HostProps = Props<Native.HostProps>
export type ViewProps = HostProps
export type TextProps = HostProps
export type ImageProps = Props<Native.ImgProps>
export type SvgProps = Props<Native.SvgProps>
export type CanvasProps = Props<Native.CanvasProps>
export type AnchoredProps = Props<Native.AnchoredProps>
export type CodeProps = Props<Native.CodeProps>
export type DiffProps = Props<Native.DiffProps>
export type MarkdownProps = Props<Native.MarkdownProps>
export type VirtualListProps = Props<Native.VirtualListProps>
export type TextInputProps = Props<Native.InputProps>
export type TextAreaProps = Props<Native.TextareaProps>
export interface ScrollViewProps extends HostProps {
  horizontal?: boolean
}
export interface ButtonProps extends HostProps {
  disabled?: boolean
  onPress?: Native.GpuiEventHandler
  onpress?: Native.GpuiEventHandler
}
export type MotionViewProps = HostProps & Native.MotionProps
