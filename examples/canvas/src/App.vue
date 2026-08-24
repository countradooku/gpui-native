<script setup lang="ts">
import { GpuiCanvas, type CanvasCommand, type EventPayload, type StyleDesc } from "gpui-vue"
import { computed, ref } from "vue"

const colors = {
  background: "#0b0f17",
  panel: "#121925",
  grid: "#253044",
  text: "#eef5ff",
  muted: "#8c9bb3",
  cyan: "#42d3ff",
  green: "#64e6a6",
  pink: "#ff6fae",
}

const rootStyle: StyleDesc = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
  gap: 16,
  padding: 28,
  background: colors.background,
  color: colors.text,
}

const buttonStyle: StyleDesc = {
  padding: 9,
  paddingLeft: 14,
  paddingRight: 14,
  borderRadius: 8,
  background: "#26344c",
  color: colors.text,
  cursor: "pointer",
  hover: { opacity: 0.82 },
  active: { opacity: 0.62 },
}

const phase = ref(0)
const dense = ref(false)
const pointer = ref("Move over the canvas to receive native pointer events")

function chartCommands(currentPhase: number, isDense: boolean): CanvasCommand[] {
  const width = 720
  const height = 320
  const count = isDense ? 4_000 : 240
  const grid: CanvasCommand[] = []

  for (let x = 0; x <= width; x += 60) {
    grid.push({
      type: "line",
      x1: x,
      y1: 0,
      x2: x,
      y2: height,
      stroke: colors.grid,
    })
  }
  for (let y = 0; y <= height; y += 40) {
    grid.push({
      type: "line",
      x1: 0,
      y1: y,
      x2: width,
      y2: y,
      stroke: colors.grid,
    })
  }

  const wave = Array.from({ length: count }, (_, index) => {
    const x = (index / (count - 1)) * width
    const y = height / 2 + Math.sin(index * 0.055 + currentPhase) * 82
    return [x, y] as const
  })
  const secondary = Array.from({ length: 180 }, (_, index) => {
    const x = (index / 179) * width
    const y = height / 2 + Math.cos(index * 0.09 - currentPhase * 0.7) * 46
    return [x, y] as const
  })

  return [
    {
      type: "rect",
      x: 0,
      y: 0,
      width,
      height,
      radius: 12,
      fill: colors.panel,
      stroke: "#34425b",
      strokeWidth: 1,
    },
    ...grid,
    {
      type: "polyline",
      points: secondary,
      stroke: colors.pink,
      strokeWidth: 2,
      dash: [7, 5],
    },
    {
      type: "polyline",
      points: wave,
      stroke: colors.cyan,
      strokeWidth: 3,
    },
    {
      type: "circle",
      cx: 610,
      cy: 82,
      radius: 18,
      fill: colors.green,
      stroke: "#d9fff0",
      strokeWidth: 2,
    },
  ]
}

const commands = computed(() => chartCommands(phase.value, dense.value))

function shiftPhase(): void {
  phase.value += Math.PI / 3
}

function toggleDensity(): void {
  dense.value = !dense.value
}

function handleMouseMove(event: EventPayload): void {
  pointer.value = `window pointer: ${Math.round(event.x ?? 0)}, ${Math.round(event.y ?? 0)}`
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="{ fontSize: 26, fontWeight: 700 }">Retained GPUI canvas</div>
    <div :style="{ color: colors.muted }">
      {{
        dense
          ? "4,000-point path. Geometry stays tessellated until commands change."
          : "Paths are tessellated in Rust; paint frames execute no JavaScript callback."
      }}
    </div>

    <GpuiCanvas
      :commands="commands"
      :style="{ width: 720, height: 320, borderRadius: 12, overflow: 'hidden' }"
      @mouse-move="handleMouseMove"
    />

    <div :style="{ display: 'flex', gap: 10, alignItems: 'center' }">
      <div :style="buttonStyle" @click="shiftPhase">Shift phase</div>
      <div :style="buttonStyle" @click="toggleDensity">
        {{ dense ? "Use 240 points" : "Use 4,000 points" }}
      </div>
      <div :style="{ color: colors.muted, marginLeft: 8 }">{{ pointer }}</div>
    </div>
  </div>
</template>
