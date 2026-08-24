import { h, type FunctionalComponent, type Slots } from "@vue/runtime-core"

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
  component.displayName = displayName
  return component
}

export const GpuiDiv = nativeComponent<HostProps>("div", "GpuiDiv")
export const GpuiTextElement = nativeComponent<HostProps>("text", "GpuiTextElement")
export const GpuiImage = nativeComponent<ImgProps>("img", "GpuiImage")
export const GpuiSvg = nativeComponent<SvgProps>("svg", "GpuiSvg")
export const GpuiCanvas = nativeComponent<CanvasProps>("canvas", "GpuiCanvas")
export const GpuiAnchored = nativeComponent<AnchoredProps>("anchored", "GpuiAnchored")
export const GpuiCode = nativeComponent<CodeProps>("code", "GpuiCode")
export const GpuiDiff = nativeComponent<DiffProps>("diff", "GpuiDiff")
export const GpuiMarkdown = nativeComponent<MarkdownProps>("markdown", "GpuiMarkdown")
export const GpuiVirtualList = nativeComponent<VirtualListProps>("virtual-list", "GpuiVirtualList")

export interface GpuiInputComponentProps extends Omit<InputProps, "value" | "onChange"> {
  /** Vue v-model value. `value` remains available for direct host compatibility. */
  modelValue?: string
  value?: string
  onChange?: (event: EventPayload) => void
  "onUpdate:modelValue"?: (value: string) => void
}

export interface GpuiTextareaComponentProps extends Omit<TextareaProps, "value" | "onChange"> {
  /** Vue v-model value. `value` remains available for direct host compatibility. */
  modelValue?: string
  value?: string
  onChange?: (event: EventPayload) => void
  "onUpdate:modelValue"?: (value: string) => void
}

function editorComponent<Props extends GpuiInputComponentProps>(
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
  component.displayName = displayName
  return component
}

/** Native single-line GPUI editor with Vue `v-model` support. */
export const GpuiInput = editorComponent<GpuiInputComponentProps>("input", "GpuiInput")

/** Native multiline GPUI editor with Vue `v-model` support. */
export const GpuiTextarea = editorComponent<GpuiTextareaComponentProps>("textarea", "GpuiTextarea")
