import {
  computed,
  defineComponent,
  h,
  inject,
  onUnmounted,
  provide,
  ref,
  type ComputedRef,
  type InjectionKey,
  type PropType,
  type Ref,
} from "@vue/runtime-core"

import {
  FloatingLayer,
  floatingRootStyle,
  renderHostSlot,
  type FloatingAlign,
  type FloatingSide,
} from "./floating.js"
import type { EventPayload, HostProps, StyleDesc } from "./types.js"

interface TooltipProviderContext {
  delayDuration: Ref<number>
  skipDelayDuration: Ref<number>
  disableHoverableContent: Ref<boolean>
  lastClosedAt: Ref<number>
}

const TooltipProviderKey: InjectionKey<TooltipProviderContext> = Symbol("GpuiVueTooltipProvider")

const fallbackProvider: TooltipProviderContext = {
  delayDuration: ref(0),
  skipDelayDuration: ref(300),
  disableHoverableContent: ref(false),
  lastClosedAt: ref(Number.NEGATIVE_INFINITY),
}

export interface TooltipProviderProps {
  delayDuration?: number
  skipDelayDuration?: number
  disableHoverableContent?: boolean
}

export const TooltipProvider = defineComponent({
  name: "TooltipProvider",
  props: {
    delayDuration: { type: Number, default: 0 },
    skipDelayDuration: { type: Number, default: 300 },
    disableHoverableContent: { type: Boolean, default: false },
  },
  setup(props, { slots }) {
    provide(TooltipProviderKey, {
      delayDuration: computed(() => props.delayDuration),
      skipDelayDuration: computed(() => props.skipDelayDuration),
      disableHoverableContent: computed(() => props.disableHoverableContent),
      lastClosedAt: ref(Number.NEGATIVE_INFINITY),
    })
    return () => slots.default?.()
  },
})

interface TooltipContext {
  open: ComputedRef<boolean>
  disableHoverableContent: ComputedRef<boolean>
  openImmediately(): void
  scheduleOpen(): void
  scheduleClose(): void
  cancelClose(): void
  close(): void
}

const TooltipKey: InjectionKey<TooltipContext> = Symbol("GpuiVueTooltip")

function useTooltip(name: string): TooltipContext {
  const context = inject(TooltipKey)
  if (context === undefined) throw new Error(`${name} must be used inside Tooltip`)
  return context
}

export interface TooltipProps extends HostProps {
  modelValue?: boolean
  open?: boolean
  defaultOpen?: boolean
  delayDuration?: number
  disableHoverableContent?: boolean
  onOpenChange?: (open: boolean) => void
  "onUpdate:open"?: (open: boolean) => void
  "onUpdate:modelValue"?: (open: boolean) => void
}

export const Tooltip = defineComponent({
  name: "Tooltip",
  inheritAttrs: false,
  props: {
    modelValue: { type: Boolean, default: undefined },
    open: { type: Boolean, default: undefined },
    defaultOpen: { type: Boolean, default: false },
    delayDuration: Number,
    disableHoverableContent: { type: Boolean, default: undefined },
  },
  emits: ["update:modelValue", "update:open", "openChange"],
  setup(props, { attrs, emit, slots }) {
    const provider = inject(TooltipProviderKey, fallbackProvider)
    const internalOpen = ref(props.defaultOpen)
    const open = computed(() =>
      props.modelValue !== undefined
        ? props.modelValue
        : props.open !== undefined
          ? props.open
          : internalOpen.value,
    )
    const hoverableDisabled = computed(
      () => props.disableHoverableContent ?? provider.disableHoverableContent.value,
    )
    let openTimer: ReturnType<typeof setTimeout> | undefined
    let closeTimer: ReturnType<typeof setTimeout> | undefined

    const cancelOpen = (): void => {
      if (openTimer !== undefined) clearTimeout(openTimer)
      openTimer = undefined
    }
    const cancelClose = (): void => {
      if (closeTimer !== undefined) clearTimeout(closeTimer)
      closeTimer = undefined
    }
    const setOpen = (next: boolean): void => {
      const previous = open.value
      cancelOpen()
      cancelClose()
      if (props.modelValue === undefined && props.open === undefined) internalOpen.value = next
      if (next !== previous) {
        emit("update:modelValue", next)
        emit("update:open", next)
        emit("openChange", next)
      }
      if (!next && previous) provider.lastClosedAt.value = Date.now()
    }
    const scheduleOpen = (): void => {
      cancelClose()
      const recentlyClosed =
        Date.now() - provider.lastClosedAt.value <= provider.skipDelayDuration.value
      const delay = recentlyClosed ? 0 : (props.delayDuration ?? provider.delayDuration.value)
      if (delay <= 0) {
        setOpen(true)
      } else {
        cancelOpen()
        openTimer = setTimeout(() => setOpen(true), delay)
      }
    }
    const close = (): void => setOpen(false)
    const scheduleClose = (): void => {
      cancelOpen()
      if (hoverableDisabled.value) {
        close()
      } else {
        cancelClose()
        closeTimer = setTimeout(close, 80)
      }
    }

    provide(TooltipKey, {
      open,
      disableHoverableContent: hoverableDisabled,
      openImmediately: () => setOpen(true),
      scheduleOpen,
      scheduleClose,
      cancelClose,
      close,
    })
    onUnmounted(() => {
      cancelOpen()
      cancelClose()
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

export interface TooltipTriggerProps extends HostProps {
  asChild?: boolean
}

export const TooltipTrigger = defineComponent({
  name: "TooltipTrigger",
  inheritAttrs: false,
  props: { asChild: Boolean },
  setup(props, { attrs, slots }) {
    const context = useTooltip("TooltipTrigger")
    return () => {
      const host = attrs as HostProps
      return renderHostSlot(props.asChild, slots, {
        ...attrs,
        tabIndex: props.asChild ? host.tabIndex : (host.tabIndex ?? 0),
        onMouseEnter: (event: EventPayload) => {
          host.onMouseEnter?.(event)
          context.scheduleOpen()
        },
        onMouseLeave: (event: EventPayload) => {
          host.onMouseLeave?.(event)
          context.scheduleClose()
        },
        onMouseDown: (event: EventPayload) => {
          host.onMouseDown?.(event)
          context.close()
        },
        onClick: (event: EventPayload) => {
          host.onClick?.(event)
          context.close()
        },
        onFocus: (event: EventPayload) => {
          host.onFocus?.(event)
          context.openImmediately()
        },
        onBlur: (event: EventPayload) => {
          host.onBlur?.(event)
          context.close()
        },
        onKeyDown: (event: EventPayload) => {
          host.onKeyDown?.(event)
          if (event.key === "escape") context.close()
        },
      })
    }
  },
})

export interface TooltipContentProps extends HostProps {
  side?: FloatingSide
  sideOffset?: number
  align?: FloatingAlign
  alignOffset?: number
  collisionPadding?: number
}

export const TooltipContent = defineComponent({
  name: "TooltipContent",
  inheritAttrs: false,
  props: {
    side: {
      type: String as PropType<FloatingSide>,
      default: "top",
    },
    sideOffset: { type: Number, default: 0 },
    align: {
      type: String as PropType<FloatingAlign>,
      default: "center",
    },
    alignOffset: { type: Number, default: 0 },
    collisionPadding: { type: Number, default: 8 },
  },
  setup(props, { attrs, slots }) {
    const context = useTooltip("TooltipContent")
    return () => {
      if (!context.open.value) return null
      const host = attrs as HostProps
      return h(
        FloatingLayer,
        {
          ...attrs,
          ...props,
          onMouseEnter: (event: EventPayload) => {
            host.onMouseEnter?.(event)
            if (!context.disableHoverableContent.value) context.cancelClose()
          },
          onMouseLeave: (event: EventPayload) => {
            host.onMouseLeave?.(event)
            context.scheduleClose()
          },
        } as TooltipContentProps,
        slots,
      )
    }
  },
})
