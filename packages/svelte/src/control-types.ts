import type { Snippet } from "svelte"

import type { HostProps, TextInputProps, ElementRef, StyleDesc, EventPayload } from "./types.js"
export interface FloatingContentProps extends HostProps {
  side?: "top" | "right" | "bottom" | "left"
  align?: "start" | "center" | "end"
  sideOffset?: number
  alignOffset?: number
  collisionPadding?: number
  contentRef?: ElementRef | null
}
export type StateStyle<State> = StyleDesc | ((state: State) => StyleDesc)
export interface SelectProps extends HostProps {
  value?: string
  defaultValue?: string
  open?: boolean
  defaultOpen?: boolean
  disabled?: boolean
  onValueChange?: (value: string) => void
  onOpenChange?: (open: boolean) => void
}
export interface SelectTriggerProps extends HostProps {
  child?: Snippet<[HostProps]>
}
export interface SelectValueProps extends HostProps {
  placeholder?: string
}
export interface SelectionItemState {
  selected: boolean
  highlighted: boolean
  disabled: boolean
}
export interface SelectItemProps extends Omit<HostProps, "style"> {
  style?: StateStyle<SelectionItemState>
  content?: Snippet<[SelectionItemState]>

  value: string
  textValue?: string
  disabled?: boolean
}
export type SelectContentProps = FloatingContentProps
export interface ComboboxProps extends Omit<
  SelectProps,
  "value" | "defaultValue" | "onValueChange"
> {
  value?: string | string[]
  defaultValue?: string | string[]
  multiple?: boolean
  inputValue?: string
  defaultInputValue?: string
  onValueChange?: (value: string | string[]) => void
  onInputValueChange?: (value: string) => void
  items?: readonly string[]
  itemToStringValue?: (value: string) => string
  filter?:
    | null
    | ((value: string, query: string, itemToString: (value: string) => string) => boolean)
  autoHighlight?: boolean | "always"
}
export type ComboboxInputProps = TextInputProps
export type ComboboxContentProps = FloatingContentProps
export type ComboboxItemProps = SelectItemProps
export type ComboboxTriggerProps = SelectTriggerProps
export type ComboboxValueProps = SelectValueProps
export interface ComboboxListProps extends HostProps {
  item?: Snippet<[string, number]>
}
export interface TooltipProviderProps extends HostProps {
  delayDuration?: number
  skipDelayDuration?: number
}
export interface TooltipProps extends HostProps {
  open?: boolean
  defaultOpen?: boolean
  delayDuration?: number
  onOpenChange?: (open: boolean) => void
}
export interface TooltipTriggerProps extends HostProps {
  child?: Snippet<[HostProps]>
}
export interface TooltipContentProps extends FloatingContentProps {}
export interface SelectionItem {
  value: string
  label: string
  disabled: boolean
  ref: ElementRef | null
}
export interface SelectionContext {
  readonly value: string | string[]
  readonly open: boolean
  readonly disabled: boolean
  readonly inputValue: string
  readonly active: string | null
  readonly items: Map<string, SelectionItem>
  readonly visible: SelectionItem[]
  readonly trigger: ElementRef | null
  setTrigger(ref: ElementRef | null): void
  setContent(ref: ElementRef | null): void
  setOpen(open: boolean): void
  setInput(value: string): void
  setActive(value: string): void
  select(value: string): void
  key(event: EventPayload): void
  register(item: SelectionItem): () => void
  selected(value: string): boolean
}
