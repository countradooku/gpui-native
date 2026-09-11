import { buttonHostProps, type ButtonProps as NativeButtonProps } from "@gpui-native/runtime/button"
import {
  defineComponent,
  h,
  shallowRef,
  type FunctionalComponent,
  type PropType,
  type Slots,
} from "@vue/runtime-core"

import type { GpuiPublicInstance } from "./nodes.js"
import type {
  AnchoredProps,
  CanvasProps,
  CodeProps,
  DiffProps,
  EventPayload,
  HostProps,
  ImgProps,
  InputProps,
  MarkdownProps,
  SvgProps,
  TextareaProps,
  VirtualListProps,
} from "./types.js"

type HostTag =
  | "div"
  | "text"
  | "img"
  | "svg"
  | "canvas"
  | "input"
  | "textarea"
  | "anchored"
  | "code"
  | "diff"
  | "markdown"
  | "virtual-list"

function children(slots: Slots): ReturnType<NonNullable<Slots["default"]>> | undefined {
  return slots.default?.()
}

/** Create a typed Vue component that lowers directly to one native host tag. */
function nativeComponent<Props extends object>(
  tag: HostTag,
  displayName: string,
): FunctionalComponent<Props> {
  const component = ((props: Props, { slots }: { slots: Slots }) =>
    h(
      tag,
      props as unknown as Record<string, unknown>,
      children(slots),
    )) as FunctionalComponent<Props>
  component.inheritAttrs = false
  component.displayName = displayName
  return component
}

export const View = nativeComponent<HostProps>("div", "View")
export const Text = nativeComponent<HostProps>("text", "Text")
export const Image = nativeComponent<ImgProps>("img", "Image")
export const Svg = nativeComponent<SvgProps>("svg", "Svg")
export const Canvas = nativeComponent<CanvasProps>("canvas", "Canvas")
export const Anchored = nativeComponent<AnchoredProps>("anchored", "Anchored")
export const Code = nativeComponent<CodeProps>("code", "Code")
export const Diff = nativeComponent<DiffProps>("diff", "Diff")
export const Markdown = nativeComponent<MarkdownProps>("markdown", "Markdown")
export const VirtualList = nativeComponent<VirtualListProps>("virtual-list", "VirtualList")

export interface TextInputProps extends Omit<InputProps, "value" | "onChange"> {
  /** Vue v-model value. `value` remains available for direct host compatibility. */
  modelValue?: string
  value?: string
  onChange?: (event: EventPayload) => void
  "onUpdate:modelValue"?: (value: string) => void
}

export interface TextAreaProps extends Omit<TextareaProps, "value" | "onChange"> {
  /** Vue v-model value. `value` remains available for direct host compatibility. */
  modelValue?: string
  value?: string
  onChange?: (event: EventPayload) => void
  "onUpdate:modelValue"?: (value: string) => void
}

function editorComponent<Props extends TextInputProps>(
  tag: "input" | "textarea",
  displayName: string,
): FunctionalComponent<Props> {
  const component = ((props: Props) => {
    const nativeProps: Record<string, unknown> = {
      ...(props as unknown as Record<string, unknown>),
    }
    const updateModel = props["onUpdate:modelValue"]
    nativeProps.value = props.modelValue ?? props.value ?? ""
    nativeProps.onChange = (event: EventPayload) => {
      props.onChange?.(event)
      updateModel?.(event.value ?? "")
    }
    delete nativeProps.modelValue
    delete nativeProps["onUpdate:modelValue"]
    return h(tag, nativeProps)
  }) as FunctionalComponent<Props>
  component.inheritAttrs = false
  component.displayName = displayName
  return component
}

/** Native single-line GPUI editor with Vue `v-model` support. */
export const TextInput = editorComponent<TextInputProps>("input", "TextInput")

/** Native multiline GPUI editor with Vue `v-model` support. */
export const TextArea = editorComponent<TextAreaProps>("textarea", "TextArea")

export type ViewProps = HostProps
export type TextProps = HostProps
export type ImageProps = ImgProps
export interface ScrollViewProps extends HostProps {
  /** Scroll horizontally instead of vertically. Constrain the viewport with style. */
  horizontal?: boolean
}
export type ButtonProps = NativeButtonProps & Pick<HostProps, "ref">

function layoutComponent(displayName: string, flexDirection: "row" | "column") {
  const component: FunctionalComponent<HostProps> = (props, { slots }) =>
    h(
      "div",
      { ...props, style: { display: "flex", flexDirection, ...props.style } },
      children(slots),
    )
  component.displayName = displayName
  component.inheritAttrs = false
  return component
}

export const Row = layoutComponent("Row", "row")
export const Column = layoutComponent("Column", "column")
export const ScrollView: FunctionalComponent<ScrollViewProps> = (props, { attrs, slots }) => {
  // Vue DOM augments attrs.style; this renderer forwards native host attributes.
  const hostProps = { ...(attrs as Record<string, unknown>), ...props }
  const { horizontal, style, ...host } = hostProps
  return h(
    "div",
    {
      ...host,
      style: {
        display: "flex",
        flexDirection: horizontal ? "row" : "column",
        overflowX: horizontal ? "scroll" : "hidden",
        overflowY: horizontal ? "hidden" : "scroll",
        ...style,
      },
    },
    children(slots),
  )
}
ScrollView.props = { horizontal: Boolean }
ScrollView.displayName = "ScrollView"
ScrollView.inheritAttrs = false

/** An unstyled button. Use onPress / @press for pointer and keyboard activation. */
export const Button = defineComponent(
  (props: ButtonProps, { attrs, slots, expose }) => {
    const element = shallowRef<GpuiPublicInstance | null>(null)
    expose({
      get id() {
        return element.value?.id
      },
    })
    const state = { spacePressed: false }
    return () =>
      h(
        "div",
        {
          ...buttonHostProps({ ...(attrs as Record<string, unknown>), ...props }, state),
          ref: element,
        },
        children(slots),
      )
  },
  {
    name: "Button",
    inheritAttrs: false,
    props: {
      disabled: Boolean,
      onPress: Function as PropType<NonNullable<ButtonProps["onPress"]>>,
    },
  },
)

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
  type TextInputProps as GpuiInputComponentProps,
  type TextAreaProps as GpuiTextareaComponentProps,
  TextInput as GpuiInput,
  TextArea as GpuiTextarea,
}
