import {
  createContext,
  createElement,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react"

import {
  FloatingLayer,
  floatingRootStyle,
  renderHostSlot,
  type FloatingContentProps,
} from "./floating.js"
import type { HostProps } from "./types.js"
export interface TooltipProviderProps {
  children?: ReactNode
  delayDuration?: number
  skipDelayDuration?: number
  disableHoverableContent?: boolean
}
const Provider = createContext({
  delayDuration: 0,
  skipDelayDuration: 300,
  disableHoverableContent: false,
  lastClosed: { current: -Infinity },
})
export function TooltipProvider({
  children,
  delayDuration = 0,
  skipDelayDuration = 300,
  disableHoverableContent = false,
}: TooltipProviderProps) {
  const lastClosed = useRef(-Infinity)
  return createElement(
    Provider,
    { value: { delayDuration, skipDelayDuration, disableHoverableContent, lastClosed } },
    children,
  )
}
interface TooltipState {
  open: boolean
  openNow(): void
  scheduleOpen(): void
  scheduleClose(): void
  cancelClose(): void
  close(): void
}
const Context = createContext<TooltipState | null>(null)
function useTooltip() {
  const value = useContext(Context)
  if (!value) throw new Error("Tooltip parts must be used inside Tooltip")
  return value
}
export interface TooltipProps extends HostProps {
  open?: boolean
  defaultOpen?: boolean
  delayDuration?: number
  disableHoverableContent?: boolean
  onOpenChange?: (open: boolean) => void
}
export function Tooltip({
  open: controlled,
  defaultOpen = false,
  delayDuration,
  disableHoverableContent,
  onOpenChange,
  children,
  style,
  ...props
}: TooltipProps) {
  const provider = useContext(Provider)
  const [internal, setInternal] = useState(defaultOpen)
  const open = controlled ?? internal
  const timers = useRef<{
    open?: ReturnType<typeof setTimeout>
    close?: ReturnType<typeof setTimeout>
  }>({})
  const cancelClose = () => clearTimeout(timers.current.close)
  const cancel = () => {
    clearTimeout(timers.current.open)
    cancelClose()
  }
  useEffect(() => cancel, [])
  const setOpen = (next: boolean) => {
    cancel()
    if (controlled === undefined) setInternal(next)
    if (next !== open) onOpenChange?.(next)
    if (!next && open) provider.lastClosed.current = Date.now()
  }
  const close = () => setOpen(false)
  const state: TooltipState = {
    open,
    openNow: () => setOpen(true),
    close,
    cancelClose,
    scheduleOpen() {
      cancel()
      const delay =
        Date.now() - provider.lastClosed.current <= provider.skipDelayDuration
          ? 0
          : (delayDuration ?? provider.delayDuration)
      if (delay <= 0) setOpen(true)
      else timers.current.open = setTimeout(() => setOpen(true), delay)
    },
    scheduleClose() {
      cancel()
      if (disableHoverableContent ?? provider.disableHoverableContent) close()
      else timers.current.close = setTimeout(close, 80)
    },
  }
  return createElement(
    Context,
    { value: state },
    createElement("div", { ...props, style: floatingRootStyle(style) }, children),
  )
}
export interface TooltipTriggerProps extends HostProps {
  asChild?: boolean
}
export function TooltipTrigger({ asChild, ...props }: TooltipTriggerProps) {
  const state = useTooltip()
  return renderHostSlot(asChild, {
    ...props,
    tabIndex: props.tabIndex ?? 0,
    onMouseEnter: (event) => {
      props.onMouseEnter?.(event)
      state.scheduleOpen()
    },
    onMouseLeave: (event) => {
      props.onMouseLeave?.(event)
      state.scheduleClose()
    },
    onFocus: (event) => {
      props.onFocus?.(event)
      state.openNow()
    },
    onBlur: (event) => {
      props.onBlur?.(event)
      state.close()
    },
    onMouseDown: (event) => {
      props.onMouseDown?.(event)
      state.close()
    },
    onClick: (event) => {
      props.onClick?.(event)
      state.close()
    },
    onKeyDown: (event) => {
      props.onKeyDown?.(event)
      if (event.key === "escape") state.close()
    },
  })
}
export interface TooltipContentProps extends FloatingContentProps {
  forceMount?: boolean
}
export function TooltipContent({ forceMount, ...props }: TooltipContentProps) {
  const state = useTooltip()
  if (!state.open && !forceMount) return null
  return createElement(FloatingLayer, {
    ...props,
    role: "tooltip",
    onMouseEnter: (event) => {
      props.onMouseEnter?.(event)
      state.cancelClose()
    },
    onMouseLeave: (event) => {
      props.onMouseLeave?.(event)
      state.scheduleClose()
    },
    onMouseDownOutside: (event) => {
      props.onMouseDownOutside?.(event)
      state.close()
    },
  })
}
