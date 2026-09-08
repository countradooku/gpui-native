<script setup lang="ts">
import {
  GpuiCanvas,
  GpuiInput,
  GpuiVirtualList,
  nextTick,
  useElementRef,
  useGpuiWindow,
  type CanvasCommand,
  type EventPayload,
  type StyleDesc,
} from "gpui-vue"
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue"

import { generateMarket, mixEntropy, sectors, venues, type MarketRow } from "./data.js"

const DATASET_SIZE = 100_000
const WINDOW_SIZE = 180
const WINDOW_BUFFER = 54
const TICK_MS = 100

const colors = {
  canvas: "#070a0e",
  panel: "#0c1118",
  raised: "#111821",
  raisedStrong: "#151e29",
  border: "#202b37",
  borderBright: "#2d3c4c",
  text: "#e8edf2",
  muted: "#788797",
  faint: "#4d5b69",
  cyan: "#45d8e7",
  cyanDim: "#18383e",
  lime: "#a8e063",
  limeDim: "#22351d",
  orange: "#ffac5e",
  orangeDim: "#3b291a",
  red: "#ff6b74",
  redDim: "#3b1e23",
  purple: "#a98cff",
} as const

const seriesColors = [colors.cyan, colors.lime, colors.orange, colors.purple, "#66a3ff"] as const
const generationStarted = performance.now()
const rows = generateMarket(DATASET_SIZE)
const generationMs = performance.now() - generationStarted

type SortField = "symbol" | "price" | "change" | "volume" | "latency"
type AlertItem = { id: number; symbol: string; message: string; color: string; time: string }

const tableRef = useElementRef()
const gpuiWindow = useGpuiWindow()
const running = ref(true)
const targetRate = ref(5_000)
const query = ref("")
const sectorFilter = ref("ALL")
const sortField = ref<SortField | null>(null)
const sortDirection = ref<"asc" | "desc">("desc")
const orderedIds = ref(Array.from({ length: DATASET_SIZE }, (_, index) => index))
const windowStart = ref(0)
const visibleStart = ref(0)
const visibleEnd = ref(32)
const updateEpoch = ref(0)
const throughput = ref(0)
const batchDuration = ref(0)
const commitDuration = ref(0)
const indexDuration = ref(0)
const totalEvents = ref(0)
const sessionSeconds = ref(0)
const alerts = ref<AlertItem[]>([
  {
    id: 1,
    symbol: "VECT08F",
    message: "Spread normalized on XNAS",
    color: colors.lime,
    time: "now",
  },
  { id: 2, symbol: "QBIT02A", message: "Burst traffic rerouted", color: colors.cyan, time: "1s" },
  {
    id: 3,
    symbol: "AERO0K4",
    message: "Latency threshold crossed",
    color: colors.orange,
    time: "3s",
  },
])

let entropy = 0x6d2b_79f5
let processedSinceSample = 0
let alertSequence = 4
let tickCount = 0
let updateTimer: ReturnType<typeof setInterval> | undefined
let metricsTimer: ReturnType<typeof setInterval> | undefined
let pendingAlerts: AlertItem[] = []
let batchInFlight = false
let totalEventsProcessed = 0
let latestBatchDuration = 0
let latestCommitDuration = 0
let scrollPriorityUntil = 0
let lastVisibleRangePublish = 0
let latestVisibleRange = { start: 0, end: 32 }
let visibleRangeTimer: ReturnType<typeof setTimeout> | undefined

const chartSeries = ref(
  sectors.slice(0, 5).map((name, seriesIndex) => ({
    name,
    color: seriesColors[seriesIndex] ?? colors.cyan,
    points: Array.from({ length: 72 }, (_, index) => {
      const wave = Math.sin(index * 0.17 + seriesIndex * 1.3) * (9 + seriesIndex * 2)
      return 76 + wave + seriesIndex * 3
    }),
  })),
)

const venueLoad = ref(
  venues.map((venue, index) => ({
    venue,
    load: 42 + ((index * 17) % 44),
    latency: 0.7 + index * 0.43,
  })),
)

const rootStyle: StyleDesc = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
  background: colors.canvas,
  color: colors.text,
  fontFamily: ".SystemUIFont",
}

const headerStyle: StyleDesc = {
  height: 62,
  flexShrink: 0,
  display: "flex",
  flexDirection: "row",
  alignItems: "center",
  gap: 16,
  paddingLeft: 84,
  paddingRight: 16,
  borderBottomWidth: 1,
  borderColor: colors.border,
  background: "#090d12",
  userSelect: "none",
}

const panelStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  minHeight: 0,
  borderWidth: 1,
  borderColor: colors.border,
  borderRadius: 9,
  background: colors.panel,
  overflow: "hidden",
}

const tableStyle: StyleDesc = {
  flexGrow: 1,
  minHeight: 0,
  width: "100%",
}

const filteredCount = computed(() => orderedIds.value.length)
const mountedCount = computed(() => Math.min(WINDOW_SIZE, filteredCount.value - windowStart.value))

const windowRows = computed(() => {
  const revision = updateEpoch.value
  const end = Math.min(windowStart.value + WINDOW_SIZE, orderedIds.value.length)
  const result: { row: MarketRow; logicalIndex: number; revision: number }[] = []
  for (let logicalIndex = windowStart.value; logicalIndex < end; logicalIndex += 1) {
    const rowId = orderedIds.value[logicalIndex]
    const row = rowId === undefined ? undefined : rows[rowId]
    if (row !== undefined) result.push({ row, logicalIndex, revision })
  }
  return result
})

const chartCommands = computed<CanvasCommand[]>(() => {
  const width = 308
  const height = 154
  const commands: CanvasCommand[] = [
    { type: "rect", x: 0, y: 0, width, height, radius: 7, fill: "#0a0f15" },
  ]

  for (let y = 20; y < height; y += 28) {
    commands.push({
      type: "line",
      x1: 0,
      y1: y,
      x2: width,
      y2: y,
      stroke: "#1a2530",
      strokeWidth: 1,
    })
  }
  for (let x = 0; x < width; x += 51) {
    commands.push({
      type: "line",
      x1: x,
      y1: 0,
      x2: x,
      y2: height,
      stroke: "#141e28",
      strokeWidth: 1,
    })
  }

  for (const series of chartSeries.value) {
    commands.push({
      type: "polyline",
      points: series.points.map(
        (point, index) =>
          [
            (index / Math.max(1, series.points.length - 1)) * width,
            height - Math.max(8, Math.min(height - 8, point)),
          ] as const,
      ),
      stroke: series.color,
      strokeWidth: 1.4,
    })
  }
  return commands
})

const statCards = computed(() => [
  {
    label: "EVENT THROUGHPUT",
    value: `${formatCompact(throughput.value)}/s`,
    detail: `${formatCompact(totalEvents.value)} mutations processed`,
    color: colors.cyan,
  },
  {
    label: "LOGICAL DATASET",
    value: formatInteger(filteredCount.value),
    detail: `${formatInteger(mountedCount.value)} Vue rows mounted`,
    color: colors.lime,
  },
  {
    label: "BATCH COMPUTE",
    value: `${batchDuration.value.toFixed(2)} ms`,
    detail: `${commitDuration.value.toFixed(2)} ms through Vue nextTick`,
    color: colors.orange,
  },
  {
    label: "INDEX OPERATION",
    value: `${indexDuration.value.toFixed(1)} ms`,
    detail:
      query.value.length > 0
        ? `query across ${formatInteger(DATASET_SIZE)} rows`
        : "ready for live sort + search",
    color: colors.purple,
  },
])

function formatInteger(value: number): string {
  return Math.max(0, value).toLocaleString("en-US")
}

function formatCompact(value: number): string {
  const absolute = Math.abs(value)
  if (absolute >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)}B`
  if (absolute >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (absolute >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return Math.round(value).toString()
}

function formatPrice(value: number): string {
  return value >= 1_000 ? value.toFixed(1) : value.toFixed(2)
}

function formatChange(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`
}

function formatUptime(): string {
  const minutes = Math.floor(sessionSeconds.value / 60)
  const seconds = sessionSeconds.value % 60
  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`
}

function changeColor(change: number): string {
  if (change > 0.02) return colors.lime
  if (change < -0.02) return colors.red
  return colors.muted
}

function columnStyle(width: number, align: "left" | "right" = "left", grow = false): StyleDesc {
  return {
    width,
    minWidth: width,
    flexGrow: grow ? 1 : 0,
    flexShrink: grow ? 1 : 0,
    textAlign: align,
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  }
}

function headerCellStyle(
  field: SortField | null,
  width: number,
  align: "left" | "right" = "left",
  grow = false,
): StyleDesc {
  return {
    ...columnStyle(width, align, grow),
    color: field !== null && sortField.value === field ? colors.cyan : colors.muted,
    cursor: field === null ? "default" : "pointer",
    hover: field === null ? {} : { color: colors.text },
  }
}

function rowStyle(row: MarketRow, logicalIndex: number): StyleDesc {
  let background = logicalIndex % 2 === 0 ? "#0c1118" : "#0e141c"
  if (row.health === "HOT") background = "#15191a"
  if (row.health === "LAG") background = "#171318"
  return {
    height: 34,
    width: "100%",
    flexShrink: 0,
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    paddingLeft: 12,
    paddingRight: 12,
    borderBottomWidth: 1,
    borderColor: "#17212b",
    background,
    fontSize: 11,
    hover: { background: colors.raisedStrong },
  }
}

function healthStyle(health: MarketRow["health"]): StyleDesc {
  const color = health === "HOT" ? colors.orange : health === "LAG" ? colors.red : colors.lime
  const background =
    health === "HOT" ? colors.orangeDim : health === "LAG" ? colors.redDim : colors.limeDim
  return {
    width: 60,
    height: 20,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 4,
    background,
    color,
    fontSize: 9,
    fontWeight: 700,
  }
}

function sortMark(field: SortField): string {
  if (sortField.value !== field) return ""
  return sortDirection.value === "asc" ? "  ↑" : "  ↓"
}

function rateButtonStyle(rate: number): StyleDesc {
  const active = targetRate.value === rate
  return {
    height: 27,
    paddingLeft: 10,
    paddingRight: 10,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 5,
    background: active ? colors.cyanDim : "#00000000",
    color: active ? colors.cyan : colors.muted,
    borderWidth: 1,
    borderColor: active ? "#28606a" : colors.border,
    cursor: "pointer",
    fontSize: 10,
    fontWeight: 650,
    hover: { background: colors.raisedStrong, color: colors.text },
  }
}

function sectorButtonStyle(sector: string): StyleDesc {
  const active = sectorFilter.value === sector
  return {
    width: 42,
    height: 42,
    borderRadius: 8,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    background: active ? colors.cyanDim : "#00000000",
    color: active ? colors.cyan : colors.muted,
    cursor: "pointer",
    fontSize: 10,
    fontWeight: 750,
    hover: { background: colors.raisedStrong, color: colors.text },
  }
}

function compareRows(leftId: number, rightId: number, field: SortField): number {
  const left = rows[leftId]
  const right = rows[rightId]
  if (left === undefined || right === undefined) return 0
  switch (field) {
    case "symbol":
      return left.symbol.localeCompare(right.symbol)
    case "price":
      return left.price - right.price
    case "change":
      return left.change - right.change
    case "volume":
      return left.volume - right.volume
    case "latency":
      return left.latency - right.latency
  }
}

async function rebuildIndex(): Promise<void> {
  const started = performance.now()
  const needle = query.value.trim().toLocaleLowerCase()
  const requiredSector = sectorFilter.value
  const next: number[] = []

  for (const row of rows) {
    if (requiredSector !== "ALL" && row.sector !== requiredSector) continue
    if (
      needle.length > 0 &&
      !row.symbol.toLocaleLowerCase().includes(needle) &&
      !row.company.toLocaleLowerCase().includes(needle) &&
      !row.venue.toLocaleLowerCase().includes(needle)
    ) {
      continue
    }
    next.push(row.id)
  }

  if (sortField.value !== null) {
    const field = sortField.value
    const direction = sortDirection.value === "asc" ? 1 : -1
    next.sort((left, right) => compareRows(left, right, field) * direction)
  }

  orderedIds.value = next
  windowStart.value = 0
  visibleStart.value = 0
  visibleEnd.value = Math.min(32, next.length)
  indexDuration.value = performance.now() - started
  await nextTick()
  if (tableRef.value !== null && next.length > 0) gpuiWindow.scrollToItem(tableRef.value, 0)
}

function setSort(field: SortField): void {
  if (sortField.value === field) {
    sortDirection.value = sortDirection.value === "asc" ? "desc" : "asc"
  } else {
    sortField.value = field
    sortDirection.value = field === "symbol" ? "asc" : "desc"
  }
  void rebuildIndex()
}

function selectSector(sector: string): void {
  if (sectorFilter.value === sector) return
  sectorFilter.value = sector
  void rebuildIndex()
}

function resetIndex(): void {
  query.value = ""
  sectorFilter.value = "ALL"
  sortField.value = null
  sortDirection.value = "desc"
  void rebuildIndex()
}

function handleVisibleRange(event: EventPayload): void {
  const start = Math.max(0, Math.floor(event.startIndex ?? 0))
  const end = Math.max(start, Math.ceil(event.endIndex ?? start + 1))
  const now = performance.now()
  scrollPriorityUntil = now + 180
  latestVisibleRange = { start, end: Math.min(end, orderedIds.value.length) }
  const publishDelay = 80 - (now - lastVisibleRangePublish)
  if (publishDelay <= 0) {
    if (visibleRangeTimer !== undefined) clearTimeout(visibleRangeTimer)
    visibleRangeTimer = undefined
    visibleStart.value = latestVisibleRange.start
    visibleEnd.value = latestVisibleRange.end
    lastVisibleRangePublish = now
  } else if (visibleRangeTimer === undefined) {
    visibleRangeTimer = setTimeout(() => {
      visibleRangeTimer = undefined
      visibleStart.value = latestVisibleRange.start
      visibleEnd.value = latestVisibleRange.end
      lastVisibleRangePublish = performance.now()
    }, publishDelay)
  }
  const maximum = Math.max(0, orderedIds.value.length - WINDOW_SIZE)
  const desired = Math.max(0, Math.min(maximum, start - WINDOW_BUFFER))
  const currentEnd = windowStart.value + WINDOW_SIZE
  if (start < windowStart.value + 18 || end > currentEnd - 18) windowStart.value = desired
}

function updateChart(): void {
  for (let seriesIndex = 0; seriesIndex < chartSeries.value.length; seriesIndex += 1) {
    const series = chartSeries.value[seriesIndex]
    if (series === undefined) continue
    entropy = mixEntropy(entropy)
    const previous = series.points.at(-1) ?? 72
    const movement = ((entropy & 0xff) / 255 - 0.48) * (4.5 + seriesIndex)
    series.points.shift()
    series.points.push(Math.max(14, Math.min(140, previous + movement)))
  }

  venueLoad.value = venueLoad.value.map((venue) => {
    entropy = mixEntropy(entropy)
    return {
      ...venue,
      load: Math.max(18, Math.min(98, venue.load + ((entropy & 7) - 3))),
      latency: Math.max(0.24, Math.min(9.8, venue.latency + (((entropy >>> 4) & 15) - 7) * 0.025)),
    }
  })
}

async function runBatch(): Promise<void> {
  if (!running.value || batchInFlight) return
  batchInFlight = true
  const started = performance.now()
  const batchSize = Math.max(1, Math.round(targetRate.value / (1_000 / TICK_MS)))
  const activeIds = orderedIds.value
  const mountedIds = new Set<number>()
  const mountedEnd = Math.min(windowStart.value + WINDOW_SIZE, activeIds.length)
  for (let logicalIndex = windowStart.value; logicalIndex < mountedEnd; logicalIndex += 1) {
    const mountedId = activeIds[logicalIndex]
    if (mountedId !== undefined) mountedIds.add(mountedId)
  }
  let visibleChanged = false

  for (let index = 0; index < batchSize; index += 1) {
    entropy = mixEntropy(entropy)
    const rowId = entropy % DATASET_SIZE
    const row = rows[rowId]
    if (row === undefined) continue
    if (mountedIds.has(rowId)) visibleChanged = true

    const priorHealth = row.health
    const signed = ((entropy >>> 8) & 0xffff) / 65_535 - 0.495
    const priceMove = signed * Math.max(0.012, row.price * 0.00038)
    row.price = Math.max(0.05, row.price + priceMove)
    const spread = Math.max(0.01, row.price * (0.00004 + ((entropy >>> 24) & 31) / 92_000))
    row.bid = row.price - spread / 2
    row.ask = row.price + spread / 2
    row.change = Math.max(-12, Math.min(12, row.change + signed * 0.042))
    row.volume += 10 + (entropy & 0x7ff)
    row.trades += 1 + ((entropy >>> 12) & 7)
    row.latency = Math.max(0.12, Math.min(12.8, row.latency + signed * 0.22))
    row.health = Math.abs(row.change) > 7.6 ? "HOT" : row.latency > 7.8 ? "LAG" : "NORMAL"

    if (row.health !== priorHealth && row.health !== "NORMAL" && pendingAlerts.length < 8) {
      pendingAlerts.push({
        id: alertSequence,
        symbol: row.symbol,
        message: row.health === "LAG" ? "Venue latency threshold" : "Price velocity threshold",
        color: row.health === "LAG" ? colors.red : colors.orange,
        time: "now",
      })
      alertSequence += 1
    }
  }

  processedSinceSample += batchSize
  totalEventsProcessed += batchSize
  latestBatchDuration = performance.now() - started
  tickCount += 1
  const scrolling = performance.now() < scrollPriorityUntil
  if (tickCount % 5 === 0 && !scrolling) updateChart()
  if (visibleChanged && !scrolling) {
    const beforeCommit = performance.now()
    updateEpoch.value += 1
    await nextTick()
    latestCommitDuration = performance.now() - beforeCommit
  }
  batchInFlight = false
}

function sampleMetrics(): void {
  throughput.value = processedSinceSample
  processedSinceSample = 0
  totalEvents.value = totalEventsProcessed
  batchDuration.value = latestBatchDuration
  commitDuration.value = latestCommitDuration
  sessionSeconds.value += 1
  if (pendingAlerts.length > 0) {
    alerts.value = [...pendingAlerts.reverse(), ...alerts.value]
      .slice(0, 5)
      .map((alert, index) => ({ ...alert, time: index === 0 ? "now" : `${index + 1}s` }))
    pendingAlerts = []
  }
}

function startTimers(): void {
  updateTimer = setInterval(() => void runBatch(), TICK_MS)
  metricsTimer = setInterval(sampleMetrics, 1_000)
}

function toggleRunning(): void {
  running.value = !running.value
}

watch(query, () => void rebuildIndex())

onMounted(() => {
  startTimers()
})

onBeforeUnmount(() => {
  if (updateTimer !== undefined) clearInterval(updateTimer)
  if (metricsTimer !== undefined) clearInterval(metricsTimer)
  if (visibleRangeTimer !== undefined) clearTimeout(visibleRangeTimer)
})
</script>

<template>
  <div :style="rootStyle">
    <div :style="headerStyle">
      <div :style="{ display: 'flex', flexDirection: 'row', alignItems: 'center', gap: 10 }">
        <div
          :style="{
            width: 30,
            height: 30,
            borderRadius: 7,
            background: colors.cyanDim,
            borderWidth: 1,
            borderColor: '#28606a',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: colors.cyan,
            fontWeight: 800,
            fontSize: 14,
          }"
        >
          V
        </div>
        <div :style="{ display: 'flex', flexDirection: 'column', gap: 2 }">
          <text :style="{ fontSize: 12, fontWeight: 750, color: colors.text }">VECTOR GRID</text>
          <text :style="{ fontSize: 9, color: colors.muted }">MARKET FABRIC / PERFORMANCE LAB</text>
        </div>
      </div>

      <div :style="{ width: 1, height: 26, background: colors.border }" />

      <GpuiInput
        v-model="query"
        testId="market-search"
        placeholder="Search 100,000 symbols, companies, venues…"
        :style="{
          width: 340,
          height: 34,
          borderRadius: 6,
          borderWidth: 1,
          borderColor: colors.borderBright,
          background: colors.raised,
          color: colors.text,
          fontSize: 11,
          paddingLeft: 12,
          paddingRight: 12,
        }"
      />

      <div :style="{ flexGrow: 1 }" />
      <div :style="{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 2 }">
        <text :style="{ fontSize: 9, color: colors.muted }">SESSION</text>
        <text :style="{ fontSize: 11, color: colors.text }">{{ formatUptime() }}</text>
      </div>
      <div
        :style="{
          height: 29,
          paddingLeft: 11,
          paddingRight: 11,
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          gap: 7,
          borderRadius: 5,
          background: running ? colors.limeDim : colors.redDim,
          color: running ? colors.lime : colors.red,
          cursor: 'pointer',
          fontSize: 10,
          fontWeight: 750,
          hover: { opacity: 0.8 },
        }"
        testId="stream-toggle"
        @click="toggleRunning"
      >
        <div
          :style="{
            width: 6,
            height: 6,
            borderRadius: 3,
            background: running ? colors.lime : colors.red,
          }"
        />
        {{ running ? "STREAMING" : "PAUSED" }}
      </div>
    </div>

    <div :style="{ display: 'flex', flexDirection: 'row', flexGrow: 1, minHeight: 0 }">
      <div
        :style="{
          width: 62,
          flexShrink: 0,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: 7,
          paddingTop: 14,
          paddingBottom: 12,
          borderRightWidth: 1,
          borderColor: colors.border,
          background: '#090d12',
          userSelect: 'none',
        }"
      >
        <div
          v-for="sector in ['ALL', ...sectors]"
          :key="sector"
          :style="sectorButtonStyle(sector)"
          :testId="`sector-${sector.toLocaleLowerCase()}`"
          @click="selectSector(sector)"
        >
          {{ sector === "ALL" ? "ALL" : sector.slice(0, 2).toLocaleUpperCase() }}
        </div>
        <div :style="{ flexGrow: 1 }" />
        <div
          :style="{
            width: 38,
            height: 38,
            borderRadius: 7,
            borderWidth: 1,
            borderColor: colors.border,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: colors.muted,
            cursor: 'pointer',
            hover: { background: colors.raisedStrong, color: colors.text },
          }"
          @click="resetIndex"
        >
          ↻
        </div>
      </div>

      <div
        :style="{
          flexGrow: 1,
          minWidth: 0,
          minHeight: 0,
          display: 'flex',
          flexDirection: 'column',
          gap: 10,
          padding: 11,
        }"
      >
        <div :style="{ height: 83, flexShrink: 0, display: 'flex', flexDirection: 'row', gap: 10 }">
          <div
            v-for="stat in statCards"
            :key="stat.label"
            :style="{
              flexGrow: 1,
              minWidth: 0,
              display: 'flex',
              flexDirection: 'column',
              gap: 5,
              padding: 12,
              borderRadius: 8,
              borderWidth: 1,
              borderColor: colors.border,
              background: colors.panel,
            }"
          >
            <div :style="{ display: 'flex', alignItems: 'center', gap: 6 }">
              <div :style="{ width: 5, height: 5, borderRadius: 3, background: stat.color }" />
              <text :style="{ fontSize: 9, color: colors.muted, fontWeight: 650 }">{{
                stat.label
              }}</text>
            </div>
            <text :style="{ fontSize: 20, fontWeight: 720, color: colors.text }">{{
              stat.value
            }}</text>
            <text :style="{ fontSize: 9, color: colors.faint }">{{ stat.detail }}</text>
          </div>
        </div>

        <div
          :style="{
            flexGrow: 1,
            minHeight: 0,
            display: 'flex',
            flexDirection: 'row',
            gap: 10,
          }"
        >
          <div :style="{ ...panelStyle, flexGrow: 1, minWidth: 0 }">
            <div
              :style="{
                height: 47,
                flexShrink: 0,
                paddingLeft: 12,
                paddingRight: 12,
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                gap: 10,
                borderBottomWidth: 1,
                borderColor: colors.border,
                background: '#0d131a',
                userSelect: 'none',
              }"
            >
              <div :style="{ display: 'flex', flexDirection: 'column', gap: 2 }">
                <text :style="{ fontSize: 11, fontWeight: 700 }">LIVE INSTRUMENT MATRIX</text>
                <text :style="{ fontSize: 9, color: colors.muted }">
                  {{ formatInteger(filteredCount) }} rows · visible
                  {{ formatInteger(visibleStart + 1) }}–{{ formatInteger(visibleEnd) }}
                </text>
              </div>
              <div :style="{ flexGrow: 1 }" />
              <text :style="{ fontSize: 9, color: colors.muted }">LOAD</text>
              <div :style="rateButtonStyle(1_000)" @click="targetRate = 1_000">1K/s</div>
              <div :style="rateButtonStyle(5_000)" @click="targetRate = 5_000">5K/s</div>
              <div :style="rateButtonStyle(20_000)" @click="targetRate = 20_000">20K/s</div>
            </div>

            <div
              :style="{
                height: 30,
                flexShrink: 0,
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                paddingLeft: 12,
                paddingRight: 12,
                borderBottomWidth: 1,
                borderColor: colors.border,
                background: '#0a0f15',
                fontSize: 9,
                fontWeight: 650,
                userSelect: 'none',
              }"
            >
              <text
                :style="headerCellStyle('symbol', 142, 'left', true)"
                @click="setSort('symbol')"
              >
                SYMBOL{{ sortMark("symbol") }}
              </text>
              <text :style="headerCellStyle(null, 78)">SECTOR</text>
              <text :style="headerCellStyle(null, 52)">VENUE</text>
              <text :style="headerCellStyle('price', 76, 'right')" @click="setSort('price')">
                LAST{{ sortMark("price") }}
              </text>
              <text :style="headerCellStyle('change', 68, 'right')" @click="setSort('change')">
                CHANGE{{ sortMark("change") }}
              </text>
              <text :style="headerCellStyle(null, 76, 'right')">BID</text>
              <text :style="headerCellStyle(null, 76, 'right')">ASK</text>
              <text :style="headerCellStyle('volume', 76, 'right')" @click="setSort('volume')">
                VOLUME{{ sortMark("volume") }}
              </text>
              <text :style="headerCellStyle(null, 66, 'right')">TRADES</text>
              <text :style="headerCellStyle('latency', 70, 'right')" @click="setSort('latency')">
                LATENCY{{ sortMark("latency") }}
              </text>
              <text :style="columnStyle(68, 'right')">STATE</text>
            </div>

            <GpuiVirtualList
              ref="tableRef"
              :item-count="filteredCount"
              :window-start="windowStart"
              :estimated-item-height="34"
              :overdraw="136"
              :style="tableStyle"
              testId="market-table"
              @visible-range="handleVisibleRange"
            >
              <div
                v-for="item in windowRows"
                :key="`${item.row.id}-${item.logicalIndex}`"
                :style="rowStyle(item.row, item.logicalIndex)"
              >
                <div :style="columnStyle(142, 'left', true)">
                  <text :style="{ color: colors.text, fontWeight: 680 }">{{
                    item.row.symbol
                  }}</text>
                  <text :style="{ color: colors.faint, marginLeft: 7 }">{{
                    item.row.company
                  }}</text>
                </div>
                <text :style="{ ...columnStyle(78), color: colors.muted }">{{
                  item.row.sector
                }}</text>
                <text :style="{ ...columnStyle(52), color: colors.faint }">{{
                  item.row.venue
                }}</text>
                <text :style="{ ...columnStyle(76, 'right'), color: colors.text }">
                  {{ formatPrice(item.row.price) }}
                </text>
                <text
                  :style="{
                    ...columnStyle(68, 'right'),
                    color: changeColor(item.row.change),
                    fontWeight: 650,
                  }"
                >
                  {{ formatChange(item.row.change) }}
                </text>
                <text :style="{ ...columnStyle(76, 'right'), color: colors.muted }">
                  {{ formatPrice(item.row.bid) }}
                </text>
                <text :style="{ ...columnStyle(76, 'right'), color: colors.muted }">
                  {{ formatPrice(item.row.ask) }}
                </text>
                <text :style="{ ...columnStyle(76, 'right'), color: colors.text }">
                  {{ formatCompact(item.row.volume) }}
                </text>
                <text :style="{ ...columnStyle(66, 'right'), color: colors.muted }">
                  {{ formatCompact(item.row.trades) }}
                </text>
                <text
                  :style="{
                    ...columnStyle(70, 'right'),
                    color: item.row.latency > 7.8 ? colors.red : colors.muted,
                  }"
                >
                  {{ item.row.latency.toFixed(2) }} ms
                </text>
                <div
                  :style="{
                    ...columnStyle(68, 'right'),
                    display: 'flex',
                    justifyContent: 'flex-end',
                  }"
                >
                  <div :style="healthStyle(item.row.health)">{{ item.row.health }}</div>
                </div>
              </div>
            </GpuiVirtualList>

            <div
              :style="{
                height: 29,
                flexShrink: 0,
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                paddingLeft: 12,
                paddingRight: 12,
                gap: 14,
                borderTopWidth: 1,
                borderColor: colors.border,
                background: '#0a0f15',
                fontSize: 9,
                color: colors.faint,
              }"
            >
              <text>NATIVE VIRTUAL WINDOW</text>
              <text :style="{ color: colors.cyan }">{{ formatInteger(mountedCount) }} mounted</text>
              <text
                >{{ formatInteger(Math.max(0, filteredCount - mountedCount)) }} retained as logical
                rows</text
              >
              <div :style="{ flexGrow: 1 }" />
              <text>dataset generated in {{ generationMs.toFixed(1) }} ms</text>
            </div>
          </div>

          <div
            :style="{
              width: 326,
              flexShrink: 0,
              minHeight: 0,
              display: 'flex',
              flexDirection: 'column',
              gap: 10,
            }"
          >
            <div :style="{ ...panelStyle, height: 251, flexShrink: 0 }">
              <div
                :style="{
                  height: 42,
                  flexShrink: 0,
                  display: 'flex',
                  alignItems: 'center',
                  paddingLeft: 11,
                  paddingRight: 11,
                  borderBottomWidth: 1,
                  borderColor: colors.border,
                }"
              >
                <text :style="{ fontSize: 10, fontWeight: 700 }">CROSS-SECTOR FLOW</text>
                <div :style="{ flexGrow: 1 }" />
                <text :style="{ fontSize: 9, color: colors.lime }">● LIVE</text>
              </div>
              <GpuiCanvas
                :commands="chartCommands"
                :style="{ width: 308, height: 154, margin: 8 }"
              />
              <div
                :style="{
                  display: 'flex',
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 10,
                  paddingLeft: 10,
                  paddingRight: 10,
                  fontSize: 8,
                  color: colors.muted,
                }"
              >
                <div
                  v-for="series in chartSeries"
                  :key="series.name"
                  :style="{ display: 'flex', alignItems: 'center', gap: 3 }"
                >
                  <div :style="{ width: 8, height: 2, background: series.color }" />
                  <text>{{ series.name.slice(0, 3).toLocaleUpperCase() }}</text>
                </div>
              </div>
            </div>

            <div :style="{ ...panelStyle, flexGrow: 1, minHeight: 195 }">
              <div
                :style="{
                  height: 40,
                  flexShrink: 0,
                  display: 'flex',
                  alignItems: 'center',
                  paddingLeft: 11,
                  paddingRight: 11,
                  borderBottomWidth: 1,
                  borderColor: colors.border,
                }"
              >
                <text :style="{ fontSize: 10, fontWeight: 700 }">VENUE SATURATION</text>
                <div :style="{ flexGrow: 1 }" />
                <text :style="{ fontSize: 8, color: colors.muted }">LOAD / RTT</text>
              </div>
              <div
                v-for="venue in venueLoad"
                :key="venue.venue"
                :style="{
                  height: 31,
                  flexShrink: 0,
                  display: 'flex',
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 8,
                  paddingLeft: 11,
                  paddingRight: 11,
                  borderBottomWidth: 1,
                  borderColor: '#17212b',
                  fontSize: 9,
                }"
              >
                <text :style="{ width: 34, color: colors.text, fontWeight: 650 }">{{
                  venue.venue
                }}</text>
                <div
                  :style="{
                    position: 'relative',
                    height: 5,
                    flexGrow: 1,
                    borderRadius: 3,
                    background: '#18222d',
                    overflow: 'hidden',
                  }"
                >
                  <div
                    :style="{
                      height: 5,
                      width: `${venue.load}%`,
                      borderRadius: 3,
                      background: venue.load > 84 ? colors.orange : colors.cyan,
                    }"
                  />
                </div>
                <text :style="{ width: 28, textAlign: 'right', color: colors.muted }"
                  >{{ venue.load }}%</text
                >
                <text
                  :style="{
                    width: 42,
                    textAlign: 'right',
                    color: venue.latency > 6 ? colors.red : colors.faint,
                  }"
                >
                  {{ venue.latency.toFixed(2) }}ms
                </text>
              </div>
            </div>

            <div :style="{ ...panelStyle, height: 183, flexShrink: 0 }">
              <div
                :style="{
                  height: 40,
                  flexShrink: 0,
                  display: 'flex',
                  alignItems: 'center',
                  paddingLeft: 11,
                  paddingRight: 11,
                  borderBottomWidth: 1,
                  borderColor: colors.border,
                }"
              >
                <text :style="{ fontSize: 10, fontWeight: 700 }">FABRIC EVENTS</text>
                <div :style="{ flexGrow: 1 }" />
                <text :style="{ fontSize: 8, color: colors.muted }">LATEST</text>
              </div>
              <div
                v-for="alert in alerts.slice(0, 4)"
                :key="alert.id"
                :style="{
                  height: 35,
                  flexShrink: 0,
                  display: 'flex',
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 7,
                  paddingLeft: 11,
                  paddingRight: 11,
                  borderBottomWidth: 1,
                  borderColor: '#17212b',
                  fontSize: 9,
                }"
              >
                <div :style="{ width: 5, height: 5, borderRadius: 3, background: alert.color }" />
                <text :style="{ width: 48, color: colors.text, fontWeight: 650 }">{{
                  alert.symbol
                }}</text>
                <text
                  :style="{
                    flexGrow: 1,
                    minWidth: 0,
                    color: colors.muted,
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }"
                >
                  {{ alert.message }}
                </text>
                <text :style="{ color: colors.faint }">{{ alert.time }}</text>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
