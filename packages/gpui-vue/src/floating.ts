import {
  cloneVNode,
  h,
  mergeProps,
  type FunctionalComponent,
  type Slots,
  type VNode,
} from "@vue/runtime-core"

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
}

export function resolveStyle<State>(
  style: StateStyle<State> | undefined,
  state: State,
): StyleDesc | undefined {
  return typeof style === "function" ? style(state) : style
}

export function mergeStyles(
  base: StyleDesc | undefined,
  override: StyleDesc | undefined,
): StyleDesc | undefined {
  if (base === undefined) return override
  if (override === undefined) return base
  return { ...base, ...override }
}

export function floatingRootStyle(style?: StyleDesc): StyleDesc {
  return {
    display: "flex",
    position: "relative",
    alignItems: "start",
    ...style,
  }
}

/** Vue equivalent of the `asChild` slot used by headless component APIs. */
export function renderHostSlot(
  asChild: boolean | undefined,
  slots: Slots,
  props: Record<string, unknown>,
): VNode {
  const content = slots.default?.() ?? []
  if (!asChild) return h("div", props, content)
  if (content.length !== 1) {
    throw new Error("asChild requires exactly one Vue vnode")
  }
  const child = content[0]
  if (child === undefined) throw new Error("asChild requires one Vue vnode")
  return cloneVNode(child, mergeProps(child.props ?? {}, props), true)
}

export const FloatingLayer: FunctionalComponent<FloatingContentProps> = (props, { slots }) => {
  const {
    side = "bottom",
    sideOffset = 0,
    align = "start",
    alignOffset = 0,
    collisionPadding = 8,
    style,
    ...contentProps
  } = props
  const offset =
    side === "top" || side === "bottom" ? { x: alignOffset, y: 0 } : { x: 0, y: alignOffset }

  return h(
    "anchored",
    {
      side,
      align,
      gap: sideOffset,
      offset,
      fit: "snap",
      snapMargin: collisionPadding,
      deferred: true,
      priority: 1,
      occlude: true,
    },
    [
      h(
        "div",
        {
          ...contentProps,
          style: mergeStyles({ backgroundColor: "#1a1a1a" }, style),
        },
        slots.default?.(),
      ),
    ],
  )
}
FloatingLayer.displayName = "FloatingLayer"
