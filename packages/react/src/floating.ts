import type { GpuiPublicInstance } from "@gpui-native/runtime/nodes"
import {
  Children,
  cloneElement,
  createElement,
  isValidElement,
  type ReactElement,
  type Ref,
} from "react"

import type { HostProps, StyleDesc } from "./types.js"
export type FloatingSide = "top" | "right" | "bottom" | "left"
export type FloatingAlign = "start" | "center" | "end"
export type StateStyle<State> = StyleDesc | ((state: State) => StyleDesc)
export interface FloatingContentProps extends HostProps {
  side?: FloatingSide
  sideOffset?: number
  align?: FloatingAlign
  alignOffset?: number
  collisionPadding?: number
  contentRef?: Ref<GpuiPublicInstance>
}
export function resolveStyle<S>(style: StateStyle<S> | undefined, state: S) {
  return typeof style === "function" ? style(state) : style
}
export function mergeStyles(base?: StyleDesc, override?: StyleDesc): StyleDesc {
  return { ...base, ...override }
}
export function floatingRootStyle(style?: StyleDesc): StyleDesc {
  return { display: "flex", position: "relative", alignItems: "start", ...style }
}
export function renderHostSlot(
  asChild: boolean | undefined,
  { children, ...props }: HostProps,
): ReactElement {
  if (!asChild) return createElement("div", props, children)
  const child = Children.only(children)
  if (!isValidElement<HostProps>(child))
    throw new Error("asChild requires exactly one React element")
  const merged: Record<string, unknown> = { ...child.props, ...props }
  for (const key of Object.keys(props)) {
    const own = (props as Record<string, unknown>)[key]
    const original = (child.props as Record<string, unknown>)[key]
    if (key.startsWith("on") && typeof own === "function" && typeof original === "function")
      merged[key] = (...args: unknown[]) => {
        original(...args)
        own(...args)
      }
  }
  merged.style = mergeStyles(child.props.style, props.style)
  if (child.props.ref && props.ref) {
    merged.ref = (node: GpuiPublicInstance | null) => {
      const cleanups: (() => void)[] = []
      for (const ref of [child.props.ref, props.ref]) {
        if (typeof ref === "function") {
          const cleanup = ref(node)
          cleanups.push(
            typeof cleanup === "function"
              ? cleanup
              : () => {
                  ref(null)
                },
          )
        } else if (ref) {
          ref.current = node
          cleanups.push(() => {
            ref.current = null
          })
        }
      }
      return () => cleanups.forEach((cleanup) => cleanup())
    }
  }
  return cloneElement(child, merged)
}
export function FloatingLayer({
  side = "bottom",
  sideOffset = 0,
  align = "start",
  alignOffset = 0,
  collisionPadding = 8,
  contentRef,
  style,
  children,
  ...props
}: FloatingContentProps) {
  return createElement(
    "anchored",
    {
      side,
      align,
      gap: sideOffset,
      offset:
        side === "top" || side === "bottom" ? { x: alignOffset, y: 0 } : { x: 0, y: alignOffset },
      fit: "snap",
      snapMargin: collisionPadding,
      deferred: true,
      priority: 1,
      occlude: true,
    },
    createElement(
      "div",
      { ...props, ref: contentRef, style: mergeStyles({ backgroundColor: "#1a1a1a" }, style) },
      children,
    ),
  )
}
