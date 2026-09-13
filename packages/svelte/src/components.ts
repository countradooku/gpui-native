import { buttonHostProps } from "@gpui-native/runtime/button"
import { EVENT_PROPS } from "@gpui-native/runtime/events"
import {
  KIT_COMPONENTS,
  KIT_MODEL_PROPS,
  kitHostProps,
  type KitComponentProps,
} from "@gpui-native/runtime/kit"
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

type NativeComponent<
  Props extends HostProps,
  Binding extends keyof Props & string = "value" extends keyof Props ? "value" : never,
> = Component<Props, ElementRef, "ref" | Binding>
const eventNames = new Map([
  ...EVENT_PROPS.map(([name]) => [name.toLowerCase(), name] as const),
  ["onpress", "onPress"],
])
type Lower = (props: Record<string, unknown>) => Record<string, unknown>

/** Share binding semantics between named Kit components and compiled native tags. */
export function kitBindingProps(
  name: keyof KitComponentProps,
  source: Record<string, unknown>,
): Lower {
  return (props) => {
    const model = KIT_MODEL_PROPS[name as keyof typeof KIT_MODEL_PROPS]
    if (!model) return kitHostProps(props)
    const changed = props.onValueChange
    return kitHostProps({
      ...props,
      onValueChange: (value: unknown) => {
        Object.getOwnPropertyDescriptor(source, model)?.set?.(value)
        if (typeof changed === "function") changed(value)
      },
    })
  }
}

export function nativePrimitive<
  Props extends HostProps,
  Binding extends keyof Props & string = "value" extends keyof Props ? "value" : never,
>(
  tag: string,
  lower?: (props: Record<string, unknown>) => Lower,
  editor = false,
): NativeComponent<Props, Binding> {
  return ((anchor: HostNode, props: Record<string, unknown>) => {
    const element = new HostElement(tag)
    const childAnchor = new HostComment()
    element.append(childAnchor)
    assign_nodes(element, element)
    const transform = lower?.(props)
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
  }) as unknown as NativeComponent<Props, Binding>
}
export const View = nativePrimitive<HostProps>("div")
export const Text = nativePrimitive<HostProps>("text")
export const Image = nativePrimitive<ImageProps>("img")
export const Svg = nativePrimitive<SvgProps>("svg")
export const Canvas = nativePrimitive<CanvasProps>("canvas")
export const Anchored = nativePrimitive<AnchoredProps>("anchored")
export const Code = nativePrimitive<CodeProps>("code")
export const Diff = nativePrimitive<DiffProps>("diff")
export const Markdown = nativePrimitive<MarkdownProps>("markdown")
export const VirtualList = nativePrimitive<VirtualListProps>("virtual-list")
export const TextInput = nativePrimitive<TextInputProps>("input", undefined, true)
export const TextArea = nativePrimitive<TextAreaProps>("textarea", undefined, true)
const layout = (flexDirection: string) => () => (props: Record<string, unknown>) => ({
  ...props,
  style: { display: "flex", flexDirection, ...(props.style as StyleDesc) },
})
export const Row = nativePrimitive<HostProps>("div", layout("row"))
export const Column = nativePrimitive<HostProps>("div", layout("column"))
export const ScrollView = nativePrimitive<ScrollViewProps>(
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
export const Button = nativePrimitive<ButtonProps>("div", () => {
  const state = { spacePressed: false }
  return (props) => buttonHostProps(props as ButtonProps, state)
})
export const MotionView = nativePrimitive<MotionViewProps>(
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
  ...Object.fromEntries(
    Object.entries(KIT_COMPONENTS).map(([name, tag]) => [
      tag,
      nativePrimitive(tag, (source) => kitBindingProps(name as keyof KitComponentProps, source)),
    ]),
  ),
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
