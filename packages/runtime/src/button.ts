import type { EventPayload, HostProps } from "./types.js"

export interface ButtonProps extends HostProps {
  /** Suppress activation and remove the button from keyboard tab navigation. */
  disabled?: boolean
  /** Pointer/accessibility click, Enter down, or Space release while focused. */
  onPress?: (event: EventPayload) => void
}

/** Each framework keeps this state on its own component instance. */
export interface ButtonState {
  spacePressed: boolean
}

/** Lower button behavior to the existing GPUI host event protocol. */
export function buttonHostProps<Props extends Omit<ButtonProps, "key">>(
  props: Props,
  state: ButtonState,
) {
  const { disabled, onPress, onClick, onKeyDown, onKeyUp, onBlur, style, ...host } = props
  if (disabled) state.spacePressed = false
  const plainKey = (event: EventPayload) =>
    !event.modifiers?.ctrl && !event.modifiers?.alt && !event.modifiers?.cmd
  const isSpace = (event: EventPayload) => event.key?.toLowerCase() === "space" || event.key === " "
  return {
    ...host,
    role: props.role ?? "button",
    tabIndex: disabled ? -1 : (props.tabIndex ?? 0),
    autoFocus: disabled ? false : props.autoFocus,
    style: {
      cursor: disabled ? ("default" as const) : ("pointer" as const),
      userSelect: "none" as const,
      ...style,
    },
    onClick: disabled
      ? undefined
      : (event: EventPayload) => {
          onClick?.(event)
          onPress?.(event)
        },
    onKeyDown: (event: EventPayload) => {
      onKeyDown?.(event)
      if (disabled || event.isHeld || !plainKey(event)) return
      if (event.key?.toLowerCase() === "enter") onPress?.(event)
      else if (isSpace(event)) state.spacePressed = true
    },
    onKeyUp: (event: EventPayload) => {
      onKeyUp?.(event)
      if (!isSpace(event)) return
      const pressed = state.spacePressed
      state.spacePressed = false
      if (pressed && !disabled && plainKey(event)) onPress?.(event)
    },
    onBlur: (event: EventPayload) => {
      state.spacePressed = false
      onBlur?.(event)
    },
  }
}
