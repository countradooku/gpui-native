import { buttonHostProps, type ButtonProps as NativeButtonProps } from "@gpui-native/runtime/button"
import { createElement, useRef, type ComponentType } from "react"

import type { GpuiIntrinsicElements, HostProps, MotionProps, MotionTransition } from "./types.js"
function primitive<K extends keyof GpuiIntrinsicElements>(
  tag: K,
): ComponentType<GpuiIntrinsicElements[K]> {
  return (props) => createElement(tag, props)
}
export const View = primitive("div")
export const Text = primitive("text")
export const Image = primitive("img")
export const Svg = primitive("svg")
export const Canvas = primitive("canvas")
export const Anchored = primitive("anchored")
export const Code = primitive("code")
export const Diff = primitive("diff")
export const Markdown = primitive("markdown")
export const TextInput = primitive("input")
export const TextArea = primitive("textarea")
export const VirtualList = primitive("virtual-list")
export type MotionViewProps = HostProps & MotionProps
export function MotionView({ initial, animate, transition, ...props }: MotionViewProps) {
  return createElement("div", { ...props, motion: { initial, animate, transition } })
}
export const motion = { View: MotionView, div: MotionView }
export function stagger(
  index: number,
  each: number,
  transition: MotionTransition = {},
): MotionTransition {
  return { ...transition, stagger: each, staggerIndex: index }
}

export type ViewProps = HostProps
export type TextProps = HostProps
export type ImageProps = GpuiIntrinsicElements["img"]
export type TextInputProps = GpuiIntrinsicElements["input"]
export type TextAreaProps = GpuiIntrinsicElements["textarea"]
export interface ScrollViewProps extends HostProps {
  /** Scroll horizontally instead of vertically. Constrain the viewport with style. */
  horizontal?: boolean
}
export type ButtonProps = HostProps & Pick<NativeButtonProps, "disabled" | "onPress">

export function Row({ style, ...props }: HostProps) {
  return createElement("div", {
    ...props,
    style: { display: "flex", flexDirection: "row", ...style },
  })
}
export function Column({ style, ...props }: HostProps) {
  return createElement("div", {
    ...props,
    style: { display: "flex", flexDirection: "column", ...style },
  })
}
export function ScrollView({ horizontal, style, ...props }: ScrollViewProps) {
  return createElement("div", {
    ...props,
    style: {
      display: "flex",
      flexDirection: horizontal ? "row" : "column",
      overflowX: horizontal ? "scroll" : "hidden",
      overflowY: horizontal ? "hidden" : "scroll",
      ...style,
    },
  })
}

/** An unstyled button. Use onPress for pointer and keyboard activation. */
export function Button(props: ButtonProps) {
  const state = useRef({ spacePressed: false })
  return createElement("div", buttonHostProps(props, state.current))
}

// Compatibility aliases; new code should use the names above.
export {
  View as GpuiDiv,
  Text as GpuiTextElement,
  Image as GpuiImage,
  Svg as GpuiSvg,
  Canvas as GpuiCanvas,
  Anchored as GpuiAnchored,
  Code as GpuiCode,
  Diff as GpuiDiff,
  Markdown as GpuiMarkdown,
  VirtualList as GpuiVirtualList,
  TextInput as GpuiInput,
  TextArea as GpuiTextarea,
  MotionView as MotionDiv,
  type MotionViewProps as MotionDivProps,
}
