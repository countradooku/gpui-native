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
import type { EventPayload, HostProps } from "./types.js"
interface Item {
  value: string
  label: ReactNode
  text: string
  disabled: boolean
}
interface State {
  open: boolean
  value: string | undefined
  disabled: boolean
  active: string | null
  items: Map<string, Item>
  trigger: { current: GpuiPublicInstance | null }
  content: { current: GpuiPublicInstance | null }
  setOpen(open: boolean): void
  setActive(value: string): void
  select(value: string): void
  key(event: EventPayload): void
  register(item: Item): () => void
}
const Context = createContext<State | null>(null)
function useSelect() {
  const state = useContext(Context)
  if (!state) throw new Error("Select parts must be used inside Select")
  return state
}
export interface SelectProps extends HostProps {
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  disabled?: boolean
}
export function Select({
  value: controlledValue,
  defaultValue,
  onValueChange,
  open: controlledOpen,
  defaultOpen = false,
  onOpenChange,
  disabled = false,
  children,
  style,
  ...props
}: SelectProps) {
  const { renderer } = useGpui()
  const [internalValue, setValue] = useState(defaultValue)
  const [internalOpen, setInternalOpen] = useState(defaultOpen)
  const [active, setActiveValue] = useState<string | null>(null)
  const [, refresh] = useState(0)
  const items = useRef(new Map<string, Item>()).current
  const trigger = useRef<GpuiPublicInstance | null>(null)
  const content = useRef<GpuiPublicInstance | null>(null)
  const typeahead = useRef({ text: "", at: 0 })
  const value = controlledValue ?? internalValue
  const open = controlledOpen ?? internalOpen
  const setOpen = (next: boolean) => {
    if (disabled && next) return
    if (controlledOpen === undefined) setInternalOpen(next)
    if (next !== open) onOpenChange?.(next)
    if (next) setActiveValue(value ?? null)
    else if (trigger.current) renderer?.focusElement?.(trigger.current.id)
  }
  const setActive = (next: string) => {
    setActiveValue(next)
    if (content.current)
      renderer?.scrollToItem?.(content.current.id, [...items.keys()].indexOf(next))
  }
  const select = (next: string) => {
    if (disabled || items.get(next)?.disabled) return
    if (controlledValue === undefined) setValue(next)
    if (next !== value) onValueChange?.(next)
    setOpen(false)
  }
  const key = (event: EventPayload) => {
    if (disabled) return
    const enabled = [...items.values()].filter((item) => !item.disabled)
    const key = event.key?.toLowerCase()
    if (key === "escape" || key === "tab") {
      setOpen(false)
      return
    }
    if (key === "enter" || key === "space" || key === " ") {
      if (!open) setOpen(true)
      else if (active !== null) select(active)
      return
    }
    let index = enabled.findIndex((item) => item.value === active)
    if (["down", "arrowdown", "up", "arrowup", "home", "end"].includes(key ?? "")) {
      if (!open) setOpen(true)
      if (key === "home") index = 0
      else if (key === "end") index = enabled.length - 1
      else {
        const delta = key === "up" || key === "arrowup" ? -1 : 1
        const start = index < 0 ? (delta > 0 ? -1 : 0) : index
        index = (start + delta + enabled.length) % enabled.length
      }
      const item = enabled[index]
      if (item) setActive(item.value)
    } else {
      const char = event.keyChar ?? event.key
      if (
        !char ||
        [...char].length !== 1 ||
        event.modifiers?.ctrl ||
        event.modifiers?.alt ||
        event.modifiers?.cmd
      )
        return
      const previous = Date.now() - typeahead.current.at < 700 ? typeahead.current.text : ""
      const text = previous === char ? char : previous + char
      typeahead.current = { text, at: Date.now() }
      const item = enabled.find((item) => item.text.toLowerCase().startsWith(text.toLowerCase()))
      if (item) {
        if (open) setActive(item.value)
        else select(item.value)
      }
    }
  }
  const state: State = {
    open,
    value,
    disabled,
    active,
    items,
    trigger,
    content,
    setOpen,
    setActive,
    select,
    key,
    register(item) {
      items.set(item.value, item)
      refresh((n) => n + 1)
      return () => {
        if (items.get(item.value) === item) {
          items.delete(item.value)
          refresh((n) => n + 1)
        }
      }
    },
  }
  return createElement(
    Context,
    { value: state },
    createElement("div", { ...props, style: floatingRootStyle(style) }, children),
  )
}
export interface SelectTriggerState {
  open: boolean
  disabled: boolean
  placeholder: boolean
}
export interface SelectTriggerProps extends Omit<HostProps, "style"> {
  asChild?: boolean
  style?: StateStyle<SelectTriggerState>
}
export function SelectTrigger({ asChild, style, ...props }: SelectTriggerProps) {
  const state = useSelect()
  return renderHostSlot(asChild, {
    ...props,
    ref: state.trigger,
    role: "combobox",
    "aria-expanded": state.open,
    tabIndex: state.disabled ? -1 : (props.tabIndex ?? 0),
    style:
      resolveStyle(style, {
        open: state.open,
        disabled: state.disabled,
        placeholder: state.value === undefined,
      }) ?? {},
    onClick: (event) => {
      props.onClick?.(event)
      state.setOpen(!state.open)
    },
    onKeyDown: (event) => {
      props.onKeyDown?.(event)
      state.key(event)
    },
  })
}
export interface SelectValueProps extends HostProps {
  placeholder?: ReactNode
}
export function SelectValue({ placeholder, children, ...props }: SelectValueProps) {
  const state = useSelect()
  return createElement(
    "text",
    props,
    children ??
      (state.value === undefined
        ? placeholder
        : (state.items.get(state.value)?.label ?? state.value)),
  )
}
export interface SelectContentProps extends FloatingContentProps {
  onEscapeKeyDown?: (event: EventPayload) => void
}
export function SelectContent({ onEscapeKeyDown, children, ...props }: SelectContentProps) {
  const state = useSelect()
  // Keep item registrations alive while the native popup is hidden.
  return createElement(
    "div",
    { style: state.open ? {} : { display: "none" } },
    createElement(
      FloatingLayer,
      {
        ...props,
        contentRef: state.content,
        role: "listbox",
        onKeyDown: (event) => {
          props.onKeyDown?.(event)
          if (event.key?.toLowerCase() === "escape") onEscapeKeyDown?.(event)
          state.key(event)
        },
        onMouseDownOutside: (event) => {
          props.onMouseDownOutside?.(event)
          state.setOpen(false)
        },
      },
      children,
    ),
  )
}
export interface SelectItemState {
  selected: boolean
  highlighted: boolean
  disabled: boolean
}
export interface SelectItemProps extends Omit<HostProps, "style" | "children"> {
  value: string
  textValue?: string
  disabled?: boolean
  style?: StateStyle<SelectItemState>
  children?: ReactNode | ((state: SelectItemState) => ReactNode)
}
export function SelectItem({
  value,
  textValue,
  disabled = false,
  style,
  children,
  ...props
}: SelectItemProps) {
  const state = useSelect()
  const itemState = {
    selected: state.value === value,
    highlighted: state.active === value,
    disabled,
  }
  const label = typeof children === "function" ? children(itemState) : children
  const register = useRef(state.register)
  register.current = state.register
  useLayoutEffect(
    () =>
      register.current({
        value,
        disabled,
        label: typeof children === "function" ? value : children,
        text: textValue ?? (typeof children === "string" ? children : value),
      }),
    [value, disabled, textValue, children],
  )
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
    label,
  )
}
export function SelectGroup(props: HostProps) {
  return createElement("div", { ...props, role: "group" })
}
export function SelectLabel(props: HostProps) {
  return createElement("text", props)
}
export function SelectSeparator(props: HostProps) {
  return createElement("div", { ...props, role: "separator" })
}
function ScrollButton({ delta, ...props }: HostProps & { delta: number }) {
  const state = useSelect()
  const { renderer } = useGpui()
  return createElement("div", {
    ...props,
    onClick: (event: EventPayload) => {
      props.onClick?.(event)
      if (state.content.current) {
        const id = state.content.current.id
        const offset = renderer?.getScrollOffset?.(id)
        renderer?.scrollTo?.(id, offset?.[0] ?? 0, (offset?.[1] ?? 0) + delta)
      }
    },
  })
}
export function SelectScrollUpButton(props: HostProps) {
  return createElement(ScrollButton, { ...props, delta: -40 })
}
export function SelectScrollDownButton(props: HostProps) {
  return createElement(ScrollButton, { ...props, delta: 40 })
}
