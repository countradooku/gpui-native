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
  type Ref,
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
import type { EventPayload, HostProps, InputProps, StyleDesc } from "./types.js"

export type ComboboxModelValue = string | string[] | null

interface ComboboxContext {
  open: ComputedRef<boolean>
  disabled: ComputedRef<boolean>
  multiple: ComputedRef<boolean>
  value: ComputedRef<ComboboxModelValue>
  inputValue: ComputedRef<string>
  filteredItems: ComputedRef<readonly string[]>
  filteredIndex: ComputedRef<ReadonlyMap<string, number>>
  activeIndex: Ref<number | null>
  inputRef: ShallowRef<GpuiPublicInstance | null>
  itemToString(item: string): string
  setOpen(open: boolean): void
  setInputValue(value: string): void
  setActiveIndex(index: number | null): void
  moveActive(delta: number): void
  selectItem(item: string): void
  registerItem(value: string, disabled: boolean): void
  unregisterItem(value: string): void
}

const ComboboxKey: InjectionKey<ComboboxContext> = Symbol("GpuiVueCombobox")

function useCombobox(name: string): ComboboxContext {
  const context = inject(ComboboxKey)
  if (context === undefined) throw new Error(`${name} must be used inside Combobox`)
  return context
}

function defaultFilter(
  items: readonly { item: string; foldedLabel: string }[],
  query: string,
): string[] {
  const normalized = query.trim().toLowerCase()
  if (normalized === "") return items.map(({ item }) => item)
  const prefix: string[] = []
  const substring: string[] = []
  for (const { item, foldedLabel } of items) {
    if (foldedLabel.startsWith(normalized)) prefix.push(item)
    else if (foldedLabel.includes(normalized)) substring.push(item)
  }
  return prefix.concat(substring)
}

export interface ComboboxProps extends HostProps {
  items?: readonly string[]
  modelValue?: ComboboxModelValue
  value?: ComboboxModelValue
  defaultValue?: ComboboxModelValue
  onValueChange?: (value: ComboboxModelValue) => void
  "onUpdate:value"?: (value: ComboboxModelValue) => void
  "onUpdate:modelValue"?: (value: ComboboxModelValue) => void
  inputValue?: string
  defaultInputValue?: string
  onInputValueChange?: (value: string) => void
  "onUpdate:inputValue"?: (value: string) => void
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  "onUpdate:open"?: (open: boolean) => void
  multiple?: boolean
  disabled?: boolean
  autoHighlight?: boolean | "always"
  filter?: null | ((item: string, query: string, itemToString: (item: string) => string) => boolean)
  itemToStringValue?: (item: string) => string
}

export const Combobox = defineComponent({
  name: "Combobox",
  inheritAttrs: false,
  props: {
    items: { type: Array as PropType<readonly string[]>, default: () => [] },
    modelValue: [String, Array] as PropType<ComboboxModelValue | undefined>,
    value: [String, Array] as PropType<ComboboxModelValue | undefined>,
    defaultValue: {
      type: [String, Array] as PropType<ComboboxModelValue>,
      default: null,
    },
    inputValue: String,
    defaultInputValue: { type: String, default: "" },
    open: { type: Boolean, default: undefined },
    defaultOpen: { type: Boolean, default: false },
    multiple: { type: Boolean, default: false },
    disabled: { type: Boolean, default: false },
    autoHighlight: {
      type: [Boolean, String] as PropType<boolean | "always">,
      default: false,
    },
    filter: Function as unknown as PropType<
      null | ((item: string, query: string, itemToString: (item: string) => string) => boolean)
    >,
    itemToStringValue: Function as PropType<(item: string) => string>,
  },
  emits: [
    "update:modelValue",
    "update:value",
    "valueChange",
    "update:inputValue",
    "inputValueChange",
    "update:open",
    "openChange",
  ],
  setup(props, { attrs, emit, slots }) {
    const { renderer } = useGpui()
    const internalValue = ref<ComboboxModelValue>(props.defaultValue)
    const internalInput = ref(props.defaultInputValue)
    const internalOpen = ref(props.defaultOpen)
    const value = computed(() =>
      props.modelValue !== undefined
        ? props.modelValue
        : props.value !== undefined
          ? props.value
          : internalValue.value,
    )
    const inputValue = computed(() => props.inputValue ?? internalInput.value)
    const open = computed(() => props.open ?? internalOpen.value)
    const disabled = computed(() => props.disabled)
    const multiple = computed(() => props.multiple)
    const activeIndex = ref<number | null>(null)
    const inputRef = shallowRef<GpuiPublicInstance | null>(null)
    const disabledItems = shallowReactive(new Set<string>())
    const itemToString = (item: string): string => props.itemToStringValue?.(item) ?? item
    const searchIndex = computed(() =>
      props.items.map((item) => ({ item, foldedLabel: itemToString(item).toLowerCase() })),
    )

    const filterItems = (query: string): readonly string[] => {
      if (props.filter === null) return props.items
      if (props.filter !== undefined) {
        return props.items.filter((item) => props.filter?.(item, query, itemToString))
      }
      return defaultFilter(searchIndex.value, query)
    }

    const filteredItems = computed<readonly string[]>(() => filterItems(inputValue.value))
    const filteredIndex = computed<ReadonlyMap<string, number>>(
      () => new Map(filteredItems.value.map((item, index) => [item, index])),
    )

    const setOpen = (next: boolean): void => {
      const previous = open.value
      if (props.open === undefined) internalOpen.value = next
      if (next !== previous) {
        emit("update:open", next)
        emit("openChange", next)
      }
      if (next) {
        queueMicrotask(() => {
          if (inputRef.value !== null) renderer?.focusElement?.(inputRef.value.id)
        })
      }
    }

    const setInputValue = (next: string): void => {
      const previous = inputValue.value
      if (props.inputValue === undefined) internalInput.value = next
      if (next !== previous) {
        emit("update:inputValue", next)
        emit("inputValueChange", next)
      }
      const nextItems = inputValue.value === next ? filteredItems.value : filterItems(next)
      const firstEnabled = nextItems.findIndex((item) => !disabledItems.has(item))
      activeIndex.value = props.autoHighlight && firstEnabled >= 0 ? firstEnabled : null
    }

    const setValue = (next: ComboboxModelValue): void => {
      const previous = value.value
      if (props.modelValue === undefined && props.value === undefined) internalValue.value = next
      if (!Object.is(next, previous)) {
        emit("update:modelValue", next)
        emit("update:value", next)
        emit("valueChange", next)
      }
    }

    const moveActive = (delta: number): void => {
      const items = filteredItems.value
      if (items.length === 0) return
      let next = activeIndex.value ?? (delta > 0 ? -1 : 0)
      for (let checked = 0; checked < items.length; checked += 1) {
        next = (next + delta + items.length) % items.length
        const item = items[next]
        if (item !== undefined && !disabledItems.has(item)) {
          activeIndex.value = next
          return
        }
      }
    }

    const selectItem = (item: string): void => {
      if (disabled.value || disabledItems.has(item)) return
      if (multiple.value) {
        const selected = Array.isArray(value.value) ? value.value : []
        setValue(
          selected.includes(item)
            ? selected.filter((candidate) => candidate !== item)
            : [...selected, item],
        )
        setInputValue("")
        activeIndex.value = null
      } else {
        setValue(item)
        setInputValue(itemToString(item))
        setOpen(false)
        activeIndex.value = null
      }
    }

    provide(ComboboxKey, {
      open,
      disabled,
      multiple,
      value,
      inputValue,
      filteredItems,
      filteredIndex,
      activeIndex,
      inputRef,
      itemToString,
      setOpen,
      setInputValue,
      setActiveIndex: (index) => {
        activeIndex.value = index
      },
      moveActive,
      selectItem,
      registerItem: (item, itemDisabled) => {
        if (itemDisabled) disabledItems.add(item)
        else disabledItems.delete(item)
      },
      unregisterItem: (item) => disabledItems.delete(item),
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

export interface ComboboxInputProps extends InputProps {
  disabled?: boolean
}

export const ComboboxInput = defineComponent({
  name: "ComboboxInput",
  inheritAttrs: false,
  props: { disabled: { type: Boolean, default: undefined } },
  setup(props, { attrs }) {
    const context = useCombobox("ComboboxInput")
    return () => {
      const host = attrs as InputProps
      const isDisabled = props.disabled ?? context.disabled.value
      return h("input", {
        ...attrs,
        ref: (element: GpuiPublicInstance | null) => {
          context.inputRef.value = element
        },
        value: context.inputValue.value,
        readOnly: isDisabled || host.readOnly,
        autoFocus: context.open.value,
        onClick: (event: EventPayload) => {
          host.onClick?.(event)
          if (!isDisabled) context.setOpen(true)
        },
        onFocus: (event: EventPayload) => {
          host.onFocus?.(event)
          if (!isDisabled) context.setOpen(true)
        },
        onChange: (event: EventPayload) => {
          host.onChange?.(event)
          context.setInputValue(event.value ?? "")
          if (!isDisabled) context.setOpen(true)
        },
        onKeyDown: (event: EventPayload) => {
          host.onKeyDown?.(event)
          if (isDisabled) return
          if (event.key === "escape") context.setOpen(false)
          else if (event.key === "down" || (event.key === "n" && event.modifiers?.ctrl)) {
            context.moveActive(1)
          } else if (event.key === "up" || (event.key === "p" && event.modifiers?.ctrl)) {
            context.moveActive(-1)
          }
        },
        onSubmit: (event: EventPayload) => {
          host.onSubmit?.(event)
          const index = context.activeIndex.value
          const item = index === null ? undefined : context.filteredItems.value[index]
          if (!isDisabled && item !== undefined) context.selectItem(item)
        },
      } as Record<string, unknown>)
    }
  },
})

export interface ComboboxTriggerProps extends HostProps {
  asChild?: boolean
  disabled?: boolean
}

export const ComboboxTrigger = defineComponent({
  name: "ComboboxTrigger",
  inheritAttrs: false,
  props: {
    asChild: Boolean,
    disabled: { type: Boolean, default: undefined },
  },
  setup(props, { attrs, slots }) {
    const context = useCombobox("ComboboxTrigger")
    return () => {
      const host = attrs as HostProps
      const isDisabled = props.disabled ?? context.disabled.value
      return renderHostSlot(props.asChild, slots, {
        ...attrs,
        tabIndex: isDisabled ? -1 : (host.tabIndex ?? 0),
        onClick: (event: EventPayload) => {
          host.onClick?.(event)
          if (!isDisabled) context.setOpen(!context.open.value)
        },
        onKeyDown: (event: EventPayload) => {
          host.onKeyDown?.(event)
          if (isDisabled) return
          if (event.key === "down" || event.key === "up") context.setOpen(true)
          if (event.key === "escape") context.setOpen(false)
        },
      })
    }
  },
})

export interface ComboboxValueProps extends HostProps {
  placeholder?: VNodeChild
}

export const ComboboxValue = defineComponent({
  name: "ComboboxValue",
  inheritAttrs: false,
  props: { placeholder: null as unknown as PropType<VNodeChild> },
  setup(props, { attrs, slots }) {
    const context = useCombobox("ComboboxValue")
    return () => {
      const model = context.value.value
      const text = Array.isArray(model)
        ? model.map(context.itemToString).join(", ")
        : model === null
          ? ""
          : context.itemToString(model)
      const content = slots.default?.({ value: model }) ?? (text || props.placeholder)
      return h("div", attrs, content == null ? undefined : [content])
    }
  },
})

export interface ComboboxContentProps extends HostProps {
  side?: FloatingSide
  sideOffset?: number
  align?: FloatingAlign
  alignOffset?: number
  collisionPadding?: number
}

export const ComboboxContent = defineComponent({
  name: "ComboboxContent",
  inheritAttrs: false,
  props: {
    side: { type: String as PropType<FloatingSide>, default: "bottom" },
    sideOffset: { type: Number, default: 0 },
    align: { type: String as PropType<FloatingAlign>, default: "start" },
    alignOffset: { type: Number, default: 0 },
    collisionPadding: { type: Number, default: 8 },
  },
  setup(props, { attrs, slots }) {
    const context = useCombobox("ComboboxContent")
    return () => {
      if (!context.open.value) return null
      const host = attrs as HostProps
      return h(
        FloatingLayer,
        {
          ...attrs,
          ...props,
          onMouseDownOutside: (event: EventPayload) => {
            host.onMouseDownOutside?.(event)
            context.setOpen(false)
          },
        } as ComboboxContentProps,
        slots,
      )
    }
  },
})

export interface ComboboxListProps extends HostProps {}

export const ComboboxList = defineComponent({
  name: "ComboboxList",
  setup(_props, { attrs, slots }) {
    const context = useCombobox("ComboboxList")
    return () =>
      h(
        "div",
        attrs,
        context.filteredItems.value.flatMap(
          (item, index) => slots.default?.({ item, index }) ?? [],
        ),
      )
  },
})

export interface ComboboxItemState {
  selected: boolean
  highlighted: boolean
  disabled: boolean
}

export interface ComboboxItemProps extends Omit<HostProps, "style"> {
  value: string
  disabled?: boolean
  style?: StateStyle<ComboboxItemState>
}

export const ComboboxItem = defineComponent({
  name: "ComboboxItem",
  inheritAttrs: false,
  props: {
    value: { type: String, required: true },
    disabled: { type: Boolean, default: false },
    style: [Object, Function] as PropType<StateStyle<ComboboxItemState>>,
  },
  setup(props, { attrs, slots }) {
    const context = useCombobox("ComboboxItem")
    context.registerItem(props.value, props.disabled)
    onBeforeUnmount(() => context.unregisterItem(props.value))
    return () => {
      context.registerItem(props.value, props.disabled)
      const index = context.filteredIndex.value.get(props.value) ?? -1
      const model = context.value.value
      const state: ComboboxItemState = {
        selected: Array.isArray(model) ? model.includes(props.value) : model === props.value,
        highlighted: context.activeIndex.value === index,
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
            if (!props.disabled && index >= 0) context.setActiveIndex(index)
          },
          onClick: (event: EventPayload) => {
            host.onClick?.(event)
            if (!props.disabled) context.selectItem(props.value)
          },
        },
        slots.default?.(state),
      )
    }
  },
})

export const ComboboxEmpty = defineComponent({
  name: "ComboboxEmpty",
  setup(_props, { attrs, slots }) {
    const context = useCombobox("ComboboxEmpty")
    return () =>
      context.filteredItems.value.length === 0 ? h("div", attrs, slots.default?.()) : null
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

export const ComboboxGroup = passthrough("ComboboxGroup")
export const ComboboxLabel = passthrough("ComboboxLabel")
export const ComboboxSeparator = passthrough("ComboboxSeparator")
