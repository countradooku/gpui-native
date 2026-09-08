export interface FindRangesOptions {
  text: string
  query: string
  caseSensitive?: boolean
  wholeWord?: boolean
}

const WORD_CHAR = /[\p{Alphabetic}\p{N}_]/u

function wordCharBefore(text: string, end: number): boolean {
  if (end <= 0) return false
  const low = text.charCodeAt(end - 1)
  const start = low >= 0xdc00 && low <= 0xdfff && end >= 2 ? end - 2 : end - 1
  return WORD_CHAR.test(text.slice(start, end))
}

function wordCharAt(text: string, start: number): boolean {
  const codePoint = text.codePointAt(start)
  return codePoint === undefined ? false : WORD_CHAR.test(String.fromCodePoint(codePoint))
}

function fold(text: string): { folded: string; map: number[] } {
  let folded = ""
  const map: number[] = []
  for (let index = 0; index < text.length;) {
    const codePoint = text.codePointAt(index)
    const character = String.fromCodePoint(codePoint ?? 0)
    const lower = character.toLowerCase()
    for (let unit = 0; unit < lower.length; unit++) map.push(index)
    folded += lower
    index += character.length
  }
  map.push(text.length)
  return { folded, map }
}

/** Native-compatible non-overlapping text ranges, measured in UTF-16 units. */
export function findRanges(options: FindRangesOptions): Array<[number, number]> {
  const { text, query, caseSensitive = false, wholeWord = false } = options
  if (query.length === 0) return []

  const { folded, map } = caseSensitive ? { folded: text, map: null } : fold(text)
  const needle = caseSensitive ? query : query.toLowerCase()
  const ranges: Array<[number, number]> = []
  let from = 0
  for (;;) {
    const at = folded.indexOf(needle, from)
    if (at === -1) break
    from = at + needle.length
    const start = map?.[at] ?? at
    const end = map?.[from] ?? from
    if (start >= end) continue
    if (wholeWord && (wordCharBefore(text, start) || wordCharAt(text, end))) continue
    ranges.push([start, end])
  }
  return ranges
}
