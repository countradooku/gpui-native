import type { HighlightSpec, EventPayload } from "./types.js"
export { findRanges, type FindRangesOptions } from "@gpui-native/runtime/text-search"
export interface TextSearchOptions extends Omit<
  HighlightSpec,
  "activeIndex" | "matchIndexOffset" | "ranges"
> {
  query: string
  matches?: { total: number; indexOffset: number }
}
export function useTextSearch(read: () => TextSearchOptions) {
  let reported = $state(0),
    requested = $state(0)
  const options = $derived(read())
  const total = $derived(options.query.length === 0 ? 0 : (options.matches?.total ?? reported))
  const active = $derived(total === 0 ? 0 : Math.min(requested, total - 1))
  const onHighlight = (event: EventPayload) => {
    reported = event.matchCount ?? 0
  }
  const props = $derived.by(() => {
    const { matches, ...highlight } = options
    return {
      highlight: options.query.length
        ? {
            ...highlight,
            activeIndex: active,
            ...(matches ? { matchIndexOffset: matches.indexOffset } : {}),
          }
        : null,
      onHighlight,
    }
  })
  return {
    get props() {
      return props
    },
    get total() {
      return total
    },
    get active() {
      return active
    },
    goTo(index: number) {
      if (index >= 0 && index < total) requested = index
    },
    next() {
      if (total) requested = (active + 1) % total
    },
    previous() {
      if (total) requested = (active + total - 1) % total
    },
  }
}
export type TextSearch = ReturnType<typeof useTextSearch>
