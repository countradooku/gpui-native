import { defineComponent, h, type PropType } from "@vue/runtime-core"

import type {
  MotionKeyframe,
  MotionProps,
  MotionStyle,
  MotionTransition,
  StyleDesc,
} from "./types.js"

export interface MotionDivProps extends MotionProps {
  style?: StyleDesc
}

/**
 * A declarative motion element evaluated by Rust on GPUI animation frames.
 * Vue is only involved when props change; playback does not mutate reactive
 * style or cross N-API once per frame.
 */
export const MotionDiv = defineComponent({
  name: "GpuiMotionDiv",
  inheritAttrs: false,
  props: {
    initial: {
      type: [Object, Boolean] as PropType<MotionStyle | false>,
      default: undefined,
    },
    animate: {
      type: [Object, Array] as PropType<MotionStyle | readonly MotionKeyframe[]>,
      required: true,
    },
    transition: {
      type: Object as PropType<MotionTransition>,
      default: undefined,
    },
  },
  setup(props, { attrs, slots }) {
    return () => {
      const nativeMotion: MotionProps = {
        animate: props.animate,
        ...(props.initial === undefined ? {} : { initial: props.initial }),
        ...(props.transition === undefined ? {} : { transition: props.transition }),
      }
      return h(
        "div",
        {
          ...attrs,
          motion: nativeMotion,
        },
        slots.default?.(),
      )
    }
  },
})

/** Return a transition configured for a native stagger index. */
export function stagger(
  index: number,
  each: number,
  transition: MotionTransition = {},
): MotionTransition {
  return { ...transition, stagger: each, staggerIndex: index }
}

export const motion = { div: MotionDiv } as const
