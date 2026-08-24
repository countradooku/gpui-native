import {
  computed,
  defineComponent,
  h,
  inject,
  provide,
  ref,
  shallowReactive,
  shallowRef,
  type ComputedRef,
  type InjectionKey,
  type PropType,
  type ShallowRef,
  type VNodeChild,
} from "@vue/runtime-core"

import { useGpui } from "./context.js"
import {
  FloatingLayer,
  floatingRootStyle,
  renderHostSlot,
  resolveStyle,
  type FloatingAlign,
  type FloatingSide,
  type StateStyle,
} from "./floating.js"
import type { GpuiPublicInstance } from "./nodes.js"
import type { EventPayload, HostProps, StyleDesc } from "./types.js"

interface SelectItemRecord {
  value: string
  label: () => VNodeChild
  textValue: string
  disabled: boolean
}

interface SelectContext {
  open: ComputedRef<boolean>
  value: ComputedRef<string | undefined>
  disabled: ComputedRef<boolean>
  activeValue: ShallowRef<string | null>
  items: Map<string, SelectItemRecord>
  triggerRef: ShallowRef<GpuiPublicInstance | null>
  triggerPressedWhileOpen: ShallowRef<boolean>
  dismissedByOutsidePress: ShallowRef<boolean>
  setOpen(open: boolean): void
  setActiveValue(value: string | null): void
  moveActive(delta: number): void
  selectValue(value: string): void
  registerItem(item: SelectItemRecord): void
  unregisterItem(value: string): void
}

const SelectKey: InjectionKey<SelectContext> = Symbol("GpuiVueSelect")

function useSelect(name: string): SelectContext {
  const context = inject(SelectKey)
  if (context === undefined) throw new Error(`${name} must be used inside Select`)
  return context
}

export interface SelectProps extends HostProps {
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
  "onUpdate:value"?: (value: string) => void
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  "onUpdate:open"?: (open: boolean) => void
  disabled?: boolean
}

export const Select = defineComponent({
  name: "Select",
  inheritAttrs: false,
  props: {
    value: String,
    defaultValue: String,
    open: { type: Boolean, default: undefined },
    defaultOpen: { type: Boolean, default: false },
    disabled: { type: Boolean, default: false },
  },
  emits: ["update:value", "valueChange", "update:open", "openChange"],
  setup(props, { attrs, emit, slots }) {
    const { renderer } = useGpui()
    const internalValue = ref<string | undefined>(props.defaultValue)
    const internalOpen = ref(props.defaultOpen)
    const value = computed(() => props.value ?? internalValue.value)
    const open = computed(() => props.open ?? internalOpen.value)
    const disabled = computed(() => props.disabled)
    const activeValue = shallowRef<string | null>(null)
    const items = shallowReactive(new Map<string, SelectItemRecord>())
    const triggerRef = shallowRef<GpuiPublicInstance | null>(null)
    const triggerPressedWhileOpen = shallowRef(false)
    const dismissedByOutsidePress = shallowRef(false)

    const setOpen = (next: boolean): void => {
      const previous = open.value
      if (props.open === undefined) internalOpen.value = next
      if (next !== previous) {
        emit("update:open", next)
        emit("openChange", next)
      }
      if (next) {
        const selected = value.value === undefined ? undefined : items.get(value.value)
        activeValue.value = selected?.disabled === false ? selected.value : null
      } else if (triggerRef.value !== null) {
        renderer?.focusElement?.(triggerRef.value.id)
      }
    }

    const setValue = (next: string): void => {
      const previous = value.value
      if (props.value === undefined) internalValue.value = next
      if (next !== previous) {
        emit("update:value", next)
        emit("valueChange", next)
      }
    }

    const moveActive = (delta: number): void => {
      const enabled = [...items.values()].filter((item) => !item.disabled)
      if (enabled.length === 0) return
      const current = enabled.findIndex((item) => item.value === activeValue.value)
      const start = current < 0 ? (delta > 0 ? -1 : 0) : current
      const index = (start + delta + enabled.length) % enabled.length
      activeValue.value = enabled[index]?.value ?? null
    }

    const selectValue = (next: string): void => {
      const item = items.get(next)
      if (item === undefined || item.disabled) return
      setValue(next)
      setOpen(false)
    }

    provide(SelectKey, {
      open,
      value,
      disabled,
      activeValue,
      items,
      triggerRef,
      triggerPressedWhileOpen,
      dismissedByOutsidePress,
      setOpen,
      setActiveValue: (next) => {
        activeValue.value = next
      },
      moveActive,
      selectValue,
      registerItem: (item) => items.set(item.value, item),
      unregisterItem: (itemValue) => items.delete(itemValue),
    })

    return () =>
      h(
        "div",
        {
          ...attrs,
          style: floatingRootStyle(attrs.style as StyleDesc | undefined),
        },
        slots.default?.(),
      )
  },
})

export interface SelectTriggerState {
  open: boolean
  disabled: boolean
  placeholder: boolean
}

export interface SelectTriggerProps extends Omit<HostProps, "style"> {
  asChild?: boolean
  disabled?: boolean
  style?: StateStyle<SelectTriggerState>
}

export const SelectTrigger = defineComponent({
  name: "SelectTrigger",
  inheritAttrs: false,
  props: {
    asChild: Boolean,
    disabled: { type: Boolean, default: undefined },
    style: [Object, Function] as PropType<StateStyle<SelectTriggerState>>,
  },
  setup(props, { attrs, slots }) {
    const context = useSelect("SelectTrigger")
    return () => {
      const host = attrs as HostProps
      const isDisabled = props.disabled ?? context.disabled.value
      const state: SelectTriggerState = {
        open: context.open.value,
        disabled: isDisabled,
        placeholder: context.value.value === undefined,
      }
      return renderHostSlot(props.asChild, slots, {
        ...attrs,
        ref: (element: GpuiPublicInstance | null) => {
          context.triggerRef.value = element
        },
        tabIndex: isDisabled ? -1 : (host.tabIndex ?? 0),
        style: resolveStyle(props.style, state),
        onMouseDown: (event: EventPayload) => {
          host.onMouseDown?.(event)
          context.triggerPressedWhileOpen.value = context.open.value
        },
        onClick: (event: EventPayload) => {
          host.onClick?.(event)
          if (isDisabled) return
          if (context.dismissedByOutsidePress.value) {
            context.dismissedByOutsidePress.value = false
          } else if (context.triggerPressedWhileOpen.value) {
            context.triggerPressedWhileOpen.value = false
            context.setOpen(false)
          } else {
            context.setOpen(!context.open.value)
          }
        },
        onKeyDown: (event: EventPayload) => {
          host.onKeyDown?.(event)
          if (isDisabled) return
          if (event.key === "escape") {
            context.setOpen(false)
          } else if (event.key === "down" || (event.key === "n" && event.modifiers?.ctrl)) {
            if (!context.open.value) context.setOpen(true)
            context.moveActive(1)
          } else if (event.key === "up" || (event.key === "p" && event.modifiers?.ctrl)) {
            if (!context.open.value) context.setOpen(true)
            context.moveActive(-1)
          } else if (event.key === "enter" || event.key === "space") {
            context.setOpen(!context.open.value)
          }
        },
      })
    }
  },
})

export interface SelectValueProps extends HostProps {
  placeholder?: VNodeChild
}

export const SelectValue = defineComponent({
  name: "SelectValue",
  inheritAttrs: false,
  props: {
    placeholder: null as unknown as PropType<VNodeChild>,
  },
  setup(props, { attrs, slots }) {
    const context = useSelect("SelectValue")
    return () => {
      const item =
        context.value.value === undefined ? undefined : context.items.get(context.value.value)
      const content = slots.default?.() ?? item?.label() ?? props.placeholder
      return h("div", attrs, content == null ? undefined : [content])
    }
  },
})

export interface SelectContentProps extends HostProps {
  side?: FloatingSide
  sideOffset?: number
  align?: FloatingAlign
  alignOffset?: number
  collisionPadding?: number
  onEscapeKeyDown?: (event: EventPayload) => void
}

export const SelectContent = defineComponent({
  name: "SelectContent",
  inheritAttrs: false,
  props: {
    side: { type: String as PropType<FloatingSide>, default: "bottom" },
    sideOffset: { type: Number, default: 0 },
    align: { type: String as PropType<FloatingAlign>, default: "start" },
    alignOffset: { type: Number, default: 0 },
    collisionPadding: { type: Number, default: 8 },
    onEscapeKeyDown: Function as PropType<(event: EventPayload) => void>,
  },
  setup(props, { attrs, slots }) {
    const context = useSelect("SelectContent")
    return () => {
      if (!context.open.value) return null
      const host = attrs as HostProps
      return h(
        FloatingLayer,
        {
          ...attrs,
          side: props.side,
          sideOffset: props.sideOffset,
          align: props.align,
          alignOffset: props.alignOffset,
          collisionPadding: props.collisionPadding,
          tabIndex: host.tabIndex ?? 0,
          autoFocus: true,
          onMouseDownOutside: (event: EventPayload) => {
            host.onMouseDownOutside?.(event)
            context.dismissedByOutsidePress.value = true
            queueMicrotask(() => {
              context.dismissedByOutsidePress.value = false
            })
            context.setOpen(false)
          },
          onKeyDown: (event: EventPayload) => {
            host.onKeyDown?.(event)
            if (event.key === "escape") {
              props.onEscapeKeyDown?.(event)
              context.setOpen(false)
            } else if (event.key === "down" || (event.key === "n" && event.modifiers?.ctrl)) {
              context.moveActive(1)
            } else if (event.key === "up" || (event.key === "p" && event.modifiers?.ctrl)) {
              context.moveActive(-1)
            } else if (
              (event.key === "enter" || event.key === "space") &&
              context.activeValue.value !== null
            ) {
              context.selectValue(context.activeValue.value)
            }
          },
        },
        slots,
      )
    }
  },
})

export interface SelectItemState {
  selected: boolean
  highlighted: boolean
  disabled: boolean
}

export interface SelectItemProps extends Omit<HostProps, "style"> {
  value: string
  disabled?: boolean
  textValue?: string
  style?: StateStyle<SelectItemState>
}

export const SelectItem = defineComponent({
  name: "SelectItem",
  inheritAttrs: false,
  props: {
    value: { type: String, required: true },
    disabled: { type: Boolean, default: false },
    textValue: String,
    style: [Object, Function] as PropType<StateStyle<SelectItemState>>,
  },
  setup(props, { attrs, slots }) {
    const context = useSelect("SelectItem")
    const record = (): SelectItemRecord => ({
      value: props.value,
      disabled: props.disabled,
      textValue: props.textValue ?? props.value,
      label: () => slots.default?.() ?? props.textValue ?? props.value,
    })
    context.registerItem(record())
    return () => {
      context.registerItem(record())
      const state: SelectItemState = {
        selected: context.value.value === props.value,
        highlighted: context.activeValue.value === props.value,
        disabled: props.disabled,
      }
      const host = attrs as HostProps
      return h(
        "div",
        {
          ...attrs,
          style: resolveStyle(props.style, state),
          onMouseEnter: (event: EventPayload) => {
            host.onMouseEnter?.(event)
            if (!props.disabled) context.setActiveValue(props.value)
          },
          onClick: (event: EventPayload) => {
            host.onClick?.(event)
            if (!props.disabled) context.selectValue(props.value)
          },
        },
        slots.default?.(state),
      )
    }
  },
})

function passthrough(name: string) {
  return defineComponent({
    name,
    inheritAttrs: false,
    setup(_props, { attrs, slots }) {
      return () => h("div", attrs, slots.default?.())
    },
  })
}

export const SelectGroup = passthrough("SelectGroup")
export const SelectLabel = passthrough("SelectLabel")
export const SelectSeparator = passthrough("SelectSeparator")
export const SelectScrollUpButton = passthrough("SelectScrollUpButton")
export const SelectScrollDownButton = passthrough("SelectScrollDownButton")
