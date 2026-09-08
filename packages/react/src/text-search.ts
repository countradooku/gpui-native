import { useState } from "react"

import type { HighlightSpec, EventPayload } from "./types.js"
export { findRanges, type FindRangesOptions } from "@gpui-native/runtime/text-search"
export interface TextSearchOptions extends Omit<
  HighlightSpec,
  "activeIndex" | "matchIndexOffset" | "ranges"
> {
  query: string
  matches?: { total: number; indexOffset: number }
}
export function useTextSearch({ matches, ...options }: TextSearchOptions) {
  const [reported, setReported] = useState(0)
  const [requested, setRequested] = useState(0)
  const total = options.query.length === 0 ? 0 : (matches?.total ?? reported)
  const active = total === 0 ? 0 : Math.min(requested, total - 1)
  return {
    props: {
      highlight: options.query.length
        ? {
            ...options,
            activeIndex: active,
            ...(matches ? { matchIndexOffset: matches.indexOffset } : {}),
          }
        : null,
      onHighlight: (event: EventPayload) => setReported(event.matchCount ?? 0),
    },
    total,
    active,
    goTo: (index: number) => {
      if (index >= 0 && index < total) setRequested(index)
    },
    next: () => {
      if (total) setRequested((active + 1) % total)
    },
    previous: () => {
      if (total) setRequested((active + total - 1) % total)
    },
  }
}
export type TextSearch = ReturnType<typeof useTextSearch>
