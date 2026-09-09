import { computed, isRef, ref, type ComputedRef, type Ref } from "@vue/runtime-core"

import type { EventPayload, HighlightSpec, HostProps } from "./types.js"

export type MaybeRefOrGetter<T> = T | Ref<T> | (() => T)

function valueOf<T>(value: MaybeRefOrGetter<T>): T {
  if (isRef(value)) return value.value
  return typeof value === "function" ? (value as () => T)() : value
}

export interface TextSearchOptions {
  query: MaybeRefOrGetter<string>
  caseSensitive?: MaybeRefOrGetter<boolean>
  wholeWord?: MaybeRefOrGetter<boolean>
  color?: MaybeRefOrGetter<string>
  activeColor?: MaybeRefOrGetter<string>
  radius?: MaybeRefOrGetter<number>
  /** Match counts for content outside a virtual-list's mounted window. */
  matches?: MaybeRefOrGetter<{ total: number; indexOffset: number }>
}

export interface TextSearch {
  /** Bind to the container to search: `v-bind="search.props.value"`. */
  props: ComputedRef<Pick<HostProps, "highlight" | "onHighlight">>
  total: ComputedRef<number>
  active: ComputedRef<number>
  next(): void
  previous(): void
  goTo(index: number): void
}

/** Reactive find-bar state backed by the native `highlight` prop. */
export function useTextSearch(options: TextSearchOptions): TextSearch {
  const reported = ref(0)
  const requested = ref(0)
  const query = computed(() => valueOf(options.query))
  const supplied = computed(() =>
    options.matches === undefined ? undefined : valueOf(options.matches),
  )
  const total = computed(() =>
    query.value.length === 0 ? 0 : (supplied.value?.total ?? reported.value),
  )
  const active = computed(() =>
    total.value === 0 ? 0 : Math.min(requested.value, total.value - 1),
  )
  const highlight = computed<HighlightSpec | null>(() => {
    if (query.value.length === 0) return null
    const spec: HighlightSpec = {
      query: query.value,
      activeIndex: active.value,
    }
    const caseSensitive =
      options.caseSensitive === undefined ? undefined : valueOf(options.caseSensitive)
    const wholeWord = options.wholeWord === undefined ? undefined : valueOf(options.wholeWord)
    const color = options.color === undefined ? undefined : valueOf(options.color)
    const activeColor = options.activeColor === undefined ? undefined : valueOf(options.activeColor)
    const radius = options.radius === undefined ? undefined : valueOf(options.radius)
    if (caseSensitive !== undefined) spec.caseSensitive = caseSensitive
    if (wholeWord !== undefined) spec.wholeWord = wholeWord
    if (color !== undefined) spec.color = color
    if (activeColor !== undefined) spec.activeColor = activeColor
    if (radius !== undefined) spec.radius = radius
    if (supplied.value !== undefined) spec.matchIndexOffset = supplied.value.indexOffset
    return spec
  })
  const onHighlight = (event: EventPayload): void => {
    reported.value = event.matchCount ?? 0
  }
  const props = computed(() => ({ highlight: highlight.value, onHighlight }))

  return {
    props,
    total,
    active,
    goTo(index) {
      if (index >= 0 && index < total.value) requested.value = index
    },
    next() {
      if (total.value > 0) requested.value = (active.value + 1) % total.value
    },
    previous() {
      if (total.value > 0) requested.value = (active.value + total.value - 1) % total.value
    },
  }
}

export { findRanges, type FindRangesOptions } from "@gpui-native/runtime/text-search"
