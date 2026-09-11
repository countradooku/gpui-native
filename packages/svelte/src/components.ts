import { buttonHostProps } from "@gpui-native/runtime/button"
import { EVENT_PROPS } from "@gpui-native/runtime/events"
import type { Component } from "svelte"

import { assign_nodes, render_effect, snippet, untrack, snapshot } from "./engine.js"
import { HostElement, HostComment, type HostNode } from "./host.js"
import type {
  AnchoredProps,
  ButtonProps,
  CanvasProps,
  CodeProps,
  DiffProps,
  ElementRef,
  HostProps,
  ImageProps,
  MarkdownProps,
  MotionViewProps,
  ScrollViewProps,
  StyleDesc,
  SvgProps,
  TextAreaProps,
  TextInputProps,
  VirtualListProps,
  EventPayload,
  MotionTransition,
} from "./types.js"

type NativeComponent<Props extends HostProps> = Component<
  Props,
  ElementRef,
  "ref" | ("value" extends keyof Props ? "value" : never)
>
const eventNames = new Map([
  ...EVENT_PROPS.map(([name]) => [name.toLowerCase(), name] as const),
  ["onpress", "onPress"],
])
type Lower = (props: Record<string, unknown>) => Record<string, unknown>

function primitive<Props extends HostProps>(
  tag: string,
  lower?: () => Lower,
  editor = false,
): NativeComponent<Props> {
  return ((anchor: HostNode, props: Record<string, unknown>) => {
    const element = new HostElement(tag)
    const childAnchor = new HostComment()
    element.append(childAnchor)
    assign_nodes(element, element)
    const transform = lower?.()
    const instance: ElementRef = {
      get id() {
        element.rootController()?.flush()
        if (!element.native) throw new Error("Native element is not mounted")
        return element.native.id
      },
    }
    const setRef = (value: ElementRef | null) => {
      const descriptor = Object.getOwnPropertyDescriptor(props, "ref")
      descriptor?.set?.(value)
    }
    render_effect(() => {
      const { children: _children, ref: _ref, ...host } = props
      for (const key of Object.keys(host)) {
        if (key.startsWith("$$")) delete host[key]
        else {
          const canonical = eventNames.get(key)
          if (canonical) {
            if (!(canonical in host)) host[canonical] = host[key]
            delete host[key]
          }
        }
      }
      // Read proxy members while the Svelte effect is active; retain raw snapshots.
      for (const key of Object.keys(host))
        if (host[key] !== null && typeof host[key] === "object") host[key] = snapshot(host[key])
      const next = transform ? transform(host) : host
      if (editor) {
        const change = next.onChange as ((event: EventPayload) => void) | undefined
        next.onChange = (event: EventPayload) => {
          Object.getOwnPropertyDescriptor(props, "value")?.set?.(event.value ?? "")
          change?.(event)
        }
      }
      element.setProps(next)
    })
    snippet(childAnchor, () => props.children as ((anchor: HostNode) => void) | undefined)
    anchor.before(element)
    render_effect(() => {
      untrack(() => setRef(instance))
      return () => untrack(() => setRef(null))
    })
    return instance
  }) as unknown as NativeComponent<Props>
}
export const View = primitive<HostProps>("div")
export const Text = primitive<HostProps>("text")
export const Image = primitive<ImageProps>("img")
export const Svg = primitive<SvgProps>("svg")
export const Canvas = primitive<CanvasProps>("canvas")
export const Anchored = primitive<AnchoredProps>("anchored")
export const Code = primitive<CodeProps>("code")
export const Diff = primitive<DiffProps>("diff")
export const Markdown = primitive<MarkdownProps>("markdown")
export const VirtualList = primitive<VirtualListProps>("virtual-list")
export const TextInput = primitive<TextInputProps>("input", undefined, true)
export const TextArea = primitive<TextAreaProps>("textarea", undefined, true)
const layout = (flexDirection: string) => () => (props: Record<string, unknown>) => ({
  ...props,
  style: { display: "flex", flexDirection, ...(props.style as StyleDesc) },
})
export const Row = primitive<HostProps>("div", layout("row"))
export const Column = primitive<HostProps>("div", layout("column"))
export const ScrollView = primitive<ScrollViewProps>(
  "div",
  () =>
    ({ horizontal, style, ...props }) => ({
      ...props,
      style: {
        display: "flex",
        flexDirection: horizontal ? "row" : "column",
        overflowX: horizontal ? "scroll" : "hidden",
        overflowY: horizontal ? "hidden" : "scroll",
        ...(style as StyleDesc),
      },
    }),
)
export const Button = primitive<ButtonProps>("div", () => {
  const state = { spacePressed: false }
  return (props) => buttonHostProps(props as ButtonProps, state)
})
export const MotionView = primitive<MotionViewProps>(
  "div",
  () =>
    ({ initial, animate, transition, ...props }) => ({
      ...props,
      motion: { initial, animate, transition },
    }),
)
export const motion = { View: MotionView } as const
export function stagger(
  index: number,
  each: number,
  transition: MotionTransition = {},
): MotionTransition {
  return { ...transition, stagger: each, staggerIndex: index }
}
const hosts = {
  div: View,
  text: Text,
  img: Image,
  svg: Svg,
  canvas: Canvas,
  anchored: Anchored,
  code: Code,
  diff: Diff,
  markdown: Markdown,
  input: TextInput,
  textarea: TextArea,
  "virtual-list": VirtualList,
}
/** Compiler target for raw host tags. */
export const Native = ((anchor: HostNode, props: Record<string, unknown>) => {
  const type = props.type
  const host = hosts[type as keyof typeof hosts]
  if (!host) throw new Error(`Unsupported GPUI element: ${String(type)}`)
  // Preserve compiled property getters, including bind:value setters.
  const descriptors = Object.getOwnPropertyDescriptors(props)
  delete descriptors.type
  return (host as unknown as (anchor: HostNode, props: object) => ElementRef)(
    anchor,
    Object.defineProperties({}, descriptors),
  )
}) as unknown as Component<HostProps & { type: string }, ElementRef>
