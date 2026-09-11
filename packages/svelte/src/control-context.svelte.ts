import type { SelectionContext } from "./control-types.js"
import { getContext } from "./public.js"
export const SELECT = Symbol("gpui.select"),
  COMBOBOX = Symbol("gpui.combobox"),
  TOOLTIP = Symbol("gpui.tooltip"),
  TOOLTIP_PROVIDER = Symbol("gpui.tooltip.provider")
export function selection(key: symbol): SelectionContext {
  const value = getContext<SelectionContext | undefined>(key)
  if (!value) throw new Error("Selection parts must be inside their Select or Combobox root")
  return value
}
export interface TooltipState {
  readonly open: boolean
  show(immediate?: boolean): void
  hide(): void
}
export function tooltip(): TooltipState {
  const value = getContext<TooltipState | undefined>(TOOLTIP)
  if (!value) throw new Error("Tooltip parts must be inside Tooltip")
  return value
}
