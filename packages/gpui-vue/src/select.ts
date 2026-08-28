import {
  computed,
  defineComponent,
  h,
  inject,
  onBeforeUnmount,
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
  selectedItem: ShallowRef<SelectItemRecord | null>
  items: Map<string, SelectItemRecord>
  triggerRef: ShallowRef<GpuiPublicInstance | null>
  contentRef: ShallowRef<GpuiPublicInstance | null>
  triggerPressedWhileOpen: ShallowRef<boolean>
  dismissedByOutsidePress: ShallowRef<boolean>
  setOpen(open: boolean): void
  setActiveValue(value: string | null): void
  moveActive(delta: number): void
  handleTypeahead(event: EventPayload): boolean
  selectValue(value: string): void
  registerItem(item: SelectItemRecord): void
  unregisterItem(item: SelectItemRecord): void
}

const SelectKey: InjectionKey<SelectContext> = Symbol("GpuiVueSelect")

function useSelect(name: string): SelectContext {
  const context = inject(SelectKey)
  if (context === undefined) throw new Error(`${name} must be used inside Select`)
  return context
}

export interface SelectProps extends HostProps {
  modelValue?: string
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
  "onUpdate:value"?: (value: string) => void
  "onUpdate:modelValue"?: (value: string) => void
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
    modelValue: String,
    value: String,
    defaultValue: String,
    open: { type: Boolean, default: undefined },
    defaultOpen: { type: Boolean, default: false },
    disabled: { type: Boolean, default: false },
  },
  emits: ["update:modelValue", "update:value", "valueChange", "update:open", "openChange"],
  setup(props, { attrs, emit, slots }) {
    const { renderer } = useGpui()
    const internalValue = ref<string | undefined>(props.defaultValue)
    const internalOpen = ref(props.defaultOpen)
    const value = computed(() =>
      props.modelValue !== undefined
        ? props.modelValue
        : props.value !== undefined
          ? props.value
          : internalValue.value,
    )
    const open = computed(() => props.open ?? internalOpen.value)
    const disabled = computed(() => props.disabled)
    const activeValue = shallowRef<string | null>(null)
    const selectedItem = shallowRef<SelectItemRecord | null>(null)
    const items = shallowReactive(new Map<string, SelectItemRecord>())
    const triggerRef = shallowRef<GpuiPublicInstance | null>(null)
    const contentRef = shallowRef<GpuiPublicInstance | null>(null)
    const triggerPressedWhileOpen = shallowRef(false)
    const dismissedByOutsidePress = shallowRef(false)
    let typeaheadBuffer = ""
    let typeaheadTimer: ReturnType<typeof setTimeout> | undefined

    const scrollActiveIntoView = (itemValue: string): void => {
      const index = [...items.keys()].indexOf(itemValue)
      if (index < 0) return
      queueMicrotask(() => {
        if (contentRef.value !== null) renderer?.scrollToItem?.(contentRef.value.id, index)
      })
    }

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
      if (props.modelValue === undefined && props.value === undefined) internalValue.value = next
      if (next !== previous) {
        emit("update:modelValue", next)
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
      if (activeValue.value !== null) scrollActiveIntoView(activeValue.value)
    }

    const handleTypeahead = (event: EventPayload): boolean => {
      if (event.modifiers?.ctrl || event.modifiers?.alt || event.modifiers?.cmd) return false
      const key = event.keyChar ?? event.key
      if (key === undefined || [...key].length !== 1 || key === " ") return false

      const character = key.toLocaleLowerCase()
      if (typeaheadTimer !== undefined) clearTimeout(typeaheadTimer)
      const repeated =
        typeaheadBuffer !== "" && [...typeaheadBuffer].every((part) => part === character)
      typeaheadBuffer = repeated ? character : typeaheadBuffer + character
      typeaheadTimer = setTimeout(() => {
        typeaheadBuffer = ""
        typeaheadTimer = undefined
      }, 700)

      const enabled = [...items.values()].filter((item) => !item.disabled)
      if (enabled.length === 0) return true
      const current = enabled.findIndex((item) => item.value === activeValue.value)
      for (let offset = 1; offset <= enabled.length; offset += 1) {
        const item = enabled[(current + offset + enabled.length) % enabled.length]
        if (item?.textValue.toLocaleLowerCase().startsWith(typeaheadBuffer)) {
          if (!open.value) setOpen(true)
          activeValue.value = item.value
          scrollActiveIntoView(item.value)
          break
        }
      }
      return true
    }

    const selectValue = (next: string): void => {
      const item = items.get(next)
      if (item === undefined || item.disabled) return
      selectedItem.value = item
      setValue(next)
      setOpen(false)
    }

    provide(SelectKey, {
      open,
      value,
      disabled,
      activeValue,
      selectedItem,
      items,
      triggerRef,
      contentRef,
      triggerPressedWhileOpen,
      dismissedByOutsidePress,
      setOpen,
      setActiveValue: (next) => {
        activeValue.value = next
      },
      moveActive,
      handleTypeahead,
      selectValue,
      registerItem: (item) => {
        items.set(item.value, item)
        if (item.value === value.value) selectedItem.value = item
      },
      unregisterItem: (item) => {
        if (items.get(item.value) === item) items.delete(item.value)
      },
    })
    onBeforeUnmount(() => {
      if (typeaheadTimer !== undefined) clearTimeout(typeaheadTimer)
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
          } else {
            context.handleTypeahead(event)
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
      const content =
        slots.default?.() ??
        item?.label() ??
        context.selectedItem.value?.label() ??
        props.placeholder
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
          contentRef: (element: GpuiPublicInstance | null) => {
            context.contentRef.value = element
          },
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
            } else {
              context.handleTypeahead(event)
            }
          },
        } as SelectContentProps & {
          contentRef: (element: GpuiPublicInstance | null) => void
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
    const item: SelectItemRecord = {
      value: props.value,
      disabled: props.disabled,
      textValue: props.textValue ?? props.value,
      label: () => slots.default?.() ?? props.textValue ?? props.value,
    }
    context.registerItem(item)
    onBeforeUnmount(() => context.unregisterItem(item))
    return () => {
      if (item.value !== props.value) {
        context.unregisterItem(item)
        item.value = props.value
      }
      item.disabled = props.disabled
      item.textValue = props.textValue ?? props.value
      context.registerItem(item)
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

function selectScrollButton(name: string, delta: number) {
  return defineComponent({
    name,
    inheritAttrs: false,
    setup(_props, { attrs, slots }) {
      const context = useSelect(name)
      return () => {
        const host = attrs as HostProps
        return h(
          "div",
          {
            ...attrs,
            onClick: (event: EventPayload) => {
              host.onClick?.(event)
              context.moveActive(delta)
            },
          },
          slots.default?.(),
        )
      }
    },
  })
}

export const SelectScrollUpButton = selectScrollButton("SelectScrollUpButton", -1)
export const SelectScrollDownButton = selectScrollButton("SelectScrollDownButton", 1)
