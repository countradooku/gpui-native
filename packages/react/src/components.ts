import { createElement, type ComponentType } from "react"

import type { GpuiIntrinsicElements, HostProps, MotionProps, MotionTransition } from "./types.js"
function primitive<K extends keyof GpuiIntrinsicElements>(
  tag: K,
): ComponentType<GpuiIntrinsicElements[K]> {
  return (props) => createElement(tag, props)
}
export const GpuiDiv = primitive("div")
export const GpuiTextElement = primitive("text")
export const GpuiImage = primitive("img")
export const GpuiSvg = primitive("svg")
export const GpuiCanvas = primitive("canvas")
export const GpuiAnchored = primitive("anchored")
export const GpuiCode = primitive("code")
export const GpuiDiff = primitive("diff")
export const GpuiMarkdown = primitive("markdown")
export const GpuiInput = primitive("input")
export const GpuiTextarea = primitive("textarea")
export const GpuiVirtualList = primitive("virtual-list")
export type MotionDivProps = HostProps & MotionProps
export function MotionDiv({ initial, animate, transition, ...props }: MotionDivProps) {
  return createElement("div", { ...props, motion: { initial, animate, transition } })
}
export const motion = { div: MotionDiv }
export function stagger(
  index: number,
  each: number,
  transition: MotionTransition = {},
): MotionTransition {
  return { ...transition, stagger: each, staggerIndex: index }
}
