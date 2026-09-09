import type { GpuiPublicInstance } from "@gpui-native/runtime/nodes"
import {
  createContext,
  createElement,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react"

import { useGpui } from "./context.js"
import {
  FloatingLayer,
  floatingRootStyle,
  renderHostSlot,
  resolveStyle,
  type FloatingContentProps,
  type StateStyle,
} from "./floating.js"
import type { EventPayload, HostProps, InputProps } from "./types.js"
export type ComboboxModelValue = string | string[] | null
export interface ComboboxProps extends HostProps {
  items?: readonly string[]
  value?: ComboboxModelValue
  defaultValue?: ComboboxModelValue
  onValueChange?: (value: ComboboxModelValue) => void
  inputValue?: string
  defaultInputValue?: string
  onInputValueChange?: (value: string) => void
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  multiple?: boolean
  disabled?: boolean
  autoHighlight?: boolean | "always"
  filter?: null | ((item: string, query: string, itemToString: (item: string) => string) => boolean)
  itemToStringValue?: (item: string) => string
}
interface State {
  open: boolean
  value: ComboboxModelValue
  input: string
  disabled: boolean
  active: string | null
  filtered: readonly string[]
  inputRef: { current: GpuiPublicInstance | null }
  label(item: string): string
  setOpen(open: boolean): void
  setInput(input: string): void
  setActive(item: string): void
  select(item: string): void
  key(event: EventPayload): void
  register(value: string, disabled: boolean): () => void
}
const Context = createContext<State | null>(null)
function useCombobox() {
  const state = useContext(Context)
  if (!state) throw new Error("Combobox parts must be used inside Combobox")
  return state
}
export function Combobox({
  items,
  value: controlledValue,
  defaultValue = null,
  onValueChange,
  inputValue: controlledInput,
  defaultInputValue = "",
  onInputValueChange,
  open: controlledOpen,
  defaultOpen = false,
  onOpenChange,
  multiple = false,
  disabled = false,
  autoHighlight = false,
  filter,
  itemToStringValue = String,
  children,
  style,
  ...props
}: ComboboxProps) {
  const { renderer } = useGpui()
  const [internalValue, setValue] = useState(defaultValue)
  const [internalInput, setInternalInput] = useState(defaultInputValue)
  const [internalOpen, setInternalOpen] = useState(defaultOpen)
  const [requested, setActive] = useState<string | null>(null)
  const [, refresh] = useState(0)
  const registered = useRef(new Map<string, boolean>()).current
  const inputRef = useRef<GpuiPublicInstance | null>(null)
  const value = controlledValue === undefined ? internalValue : controlledValue
  const input = controlledInput ?? internalInput
  const open = controlledOpen ?? internalOpen
  const all = items ?? [...registered.keys()]
  const query = input.trim().toLowerCase()
  const filtered =
    filter === null
      ? [...all]
      : filter
        ? all.filter((item) => filter(item, input, itemToStringValue))
        : all
            .filter((item) => itemToStringValue(item).toLowerCase().includes(query))
            .sort(
              (a, b) =>
                Number(itemToStringValue(b).toLowerCase().startsWith(query)) -
                Number(itemToStringValue(a).toLowerCase().startsWith(query)),
            )
  const enabled = filtered.filter((item) => !registered.get(item))
  const active =
    requested !== null && enabled.includes(requested)
      ? requested
      : autoHighlight
        ? (enabled[0] ?? null)
        : null
  const setOpen = (next: boolean) => {
    if (disabled && next) return
    if (controlledOpen === undefined) setInternalOpen(next)
    if (next !== open) onOpenChange?.(next)
  }
  const setInput = (next: string) => {
    if (controlledInput === undefined) setInternalInput(next)
    if (next !== input) onInputValueChange?.(next)
    setActive(null)
  }
  const select = (item: string) => {
    if (disabled || registered.get(item)) return
    const selected = Array.isArray(value) ? value : []
    const next = multiple
      ? selected.includes(item)
        ? selected.filter((value) => value !== item)
        : [...selected, item]
      : item
    if (controlledValue === undefined) setValue(next)
    onValueChange?.(next)
    setInput(multiple ? "" : itemToStringValue(item))
    if (!multiple) setOpen(false)
    if (inputRef.current) renderer?.focusElement?.(inputRef.current.id)
  }
  const key = (event: EventPayload) => {
    if (disabled) return
    const key = event.key?.toLowerCase()
    if (key === "escape" || key === "tab") {
      setOpen(false)
      return
    }
    if (key === "enter") {
      if (open && active !== null) select(active)
      else setOpen(true)
      return
    }
    if (["down", "arrowdown", "up", "arrowup", "home", "end"].includes(key ?? "")) {
      setOpen(true)
      const delta = key === "up" || key === "arrowup" ? -1 : 1
      const index =
        key === "home"
          ? 0
          : key === "end"
            ? enabled.length - 1
            : ((active === null ? (delta > 0 ? -1 : 0) : enabled.indexOf(active)) +
                delta +
                enabled.length) %
              enabled.length
      setActive(enabled[index] ?? null)
    }
  }
  const state: State = {
    open,
    value,
    input,
    disabled,
    active,
    filtered,
    inputRef,
    label: itemToStringValue,
    setOpen,
    setInput,
    setActive,
    select,
    key,
    register(item, disabled) {
      registered.set(item, disabled)
      refresh((n) => n + 1)
      return () => {
        registered.delete(item)
        refresh((n) => n + 1)
      }
    },
  }
  return createElement(
    Context,
    { value: state },
    createElement("div", { ...props, style: floatingRootStyle(style) }, children),
  )
}
export interface ComboboxInputProps extends InputProps {
  disabled?: boolean
}
export function ComboboxInput(props: ComboboxInputProps) {
  const state = useCombobox()
  return createElement("input", {
    ...props,
    ref: state.inputRef,
    value: state.input,
    disabled: props.disabled ?? state.disabled,
    role: "combobox",
    "aria-expanded": state.open,
    onChange: (event: EventPayload) => {
      props.onChange?.(event)
      state.setInput(event.value ?? "")
      state.setOpen(true)
    },
    onFocus: (event: EventPayload) => {
      props.onFocus?.(event)
      state.setOpen(true)
    },
    onKeyDown: (event: EventPayload) => {
      props.onKeyDown?.(event)
      state.key(event)
    },
  })
}
export interface ComboboxTriggerProps extends HostProps {
  asChild?: boolean
  disabled?: boolean
}
export function ComboboxTrigger({ asChild, disabled, ...props }: ComboboxTriggerProps) {
  const state = useCombobox()
  return renderHostSlot(asChild, {
    ...props,
    tabIndex: (disabled ?? state.disabled) ? -1 : (props.tabIndex ?? 0),
    "aria-expanded": state.open,
    onClick: (event) => {
      props.onClick?.(event)
      if (!(disabled ?? state.disabled)) state.setOpen(!state.open)
    },
    onKeyDown: (event) => {
      props.onKeyDown?.(event)
      state.key(event)
    },
  })
}
export interface ComboboxValueProps extends HostProps {
  placeholder?: ReactNode
}
export function ComboboxValue({ placeholder, children, ...props }: ComboboxValueProps) {
  const state = useCombobox()
  const label =
    state.value === null
      ? placeholder
      : Array.isArray(state.value)
        ? state.value.map(state.label).join(", ")
        : state.label(state.value)
  return createElement("text", props, children ?? label)
}
export interface ComboboxContentProps extends FloatingContentProps {}
export function ComboboxContent({ children, ...props }: ComboboxContentProps) {
  const state = useCombobox()
  return createElement(
    "div",
    { style: state.open ? {} : { display: "none" } },
    createElement(
      FloatingLayer,
      {
        ...props,
        onMouseDownOutside: (event) => {
          props.onMouseDownOutside?.(event)
          state.setOpen(false)
        },
      },
      children,
    ),
  )
}
export interface ComboboxListProps extends Omit<HostProps, "children"> {
  children?: ReactNode | ((item: string, index: number) => ReactNode)
}
export function ComboboxList({ children, ...props }: ComboboxListProps) {
  const state = useCombobox()
  return createElement(
    "div",
    {
      ...props,
      role: "listbox",
      onKeyDown: (event: EventPayload) => {
        props.onKeyDown?.(event)
        state.key(event)
      },
    },
    typeof children === "function" ? state.filtered.map(children) : children,
  )
}
export interface ComboboxItemState {
  selected: boolean
  highlighted: boolean
  disabled: boolean
}
export interface ComboboxItemProps extends Omit<HostProps, "children" | "style"> {
  value: string
  disabled?: boolean
  style?: StateStyle<ComboboxItemState>
  children?: ReactNode | ((state: ComboboxItemState) => ReactNode)
}
export function ComboboxItem({
  value,
  disabled = false,
  style,
  children,
  ...props
}: ComboboxItemProps) {
  const state = useCombobox()
  const register = useRef(state.register)
  register.current = state.register
  useLayoutEffect(() => register.current(value, disabled), [value, disabled])
  const itemState = {
    selected: Array.isArray(state.value) ? state.value.includes(value) : state.value === value,
    highlighted: state.active === value,
    disabled,
  }
  if (!state.filtered.includes(value)) return null
  return createElement(
    "div",
    {
      ...props,
      role: "option",
      "aria-selected": itemState.selected,
      style: resolveStyle(style, itemState),
      onMouseEnter: (event: EventPayload) => {
        props.onMouseEnter?.(event)
        if (!disabled) state.setActive(value)
      },
      onClick: (event: EventPayload) => {
        props.onClick?.(event)
        if (!disabled) state.select(value)
      },
    },
    typeof children === "function" ? children(itemState) : (children ?? state.label(value)),
  )
}
export function ComboboxEmpty(props: HostProps) {
  return useCombobox().filtered.length ? null : createElement("text", props)
}
export function ComboboxGroup(props: HostProps) {
  return createElement("div", { ...props, role: "group" })
}
export function ComboboxLabel(props: HostProps) {
  return createElement("text", props)
}
export function ComboboxSeparator(props: HostProps) {
  return createElement("div", { ...props, role: "separator" })
}
