<script setup lang="ts">
import {
  GpuiCanvas,
  useGpuiAudioFrames,
  type AudioBufferState,
  type CanvasCommand,
  type StyleDesc,
} from "gpui-vue"
import { computed, ref } from "vue"

const SAMPLE_RATE = 48_000
const CHANNELS = 2
const CAPACITY_FRAMES = 4_096
const CHUNK_FRAMES = 1_024

const colors = {
  background: "#090e15",
  panel: "#121b28",
  grid: "#27364a",
  text: "#eef5ff",
  muted: "#91a1b9",
  cyan: "#45d5ff",
  green: "#63e7a5",
  orange: "#ffb65c",
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
  background: "#26364e",
  color: colors.text,
  cursor: "pointer",
  hover: { opacity: 0.82 },
  active: { opacity: 0.62 },
}

function waveformCommands(samples: readonly number[]): CanvasCommand[] {
  const width = 700
  const height = 190
  const nextCommands: CanvasCommand[] = [
    { type: "rect", x: 0, y: 0, width, height, radius: 12, fill: colors.panel },
  ]
  for (let y = 38; y < height; y += 38) {
    nextCommands.push({
      type: "line",
      x1: 0,
      y1: y,
      x2: width,
      y2: y,
      stroke: colors.grid,
      strokeWidth: 1,
    })
  }
  nextCommands.push({
    type: "polyline",
    points: samples.map(
      (sample, index) =>
        [(index / Math.max(1, samples.length - 1)) * width, height / 2 - sample * 72] as const,
    ),
    stroke: colors.cyan,
    strokeWidth: 2,
  })
  return nextCommands
}

function stateText(currentState: AudioBufferState): string {
  return `${currentState.queuedFrames.toLocaleString()} / ${currentState.capacityFrames.toLocaleString()} frames queued`
}

const audio = useGpuiAudioFrames()
const state = ref(audio.configure(SAMPLE_RATE, CHANNELS, CAPACITY_FRAMES))
const waveform = ref<number[]>(Array.from({ length: 160 }, () => 0))
const phase = ref(0)
const commands = computed(() => waveformCommands(waveform.value))

function enqueueTone(): void {
  const interleaved = new Float32Array(CHUNK_FRAMES * CHANNELS)
  const preview: number[] = []
  let nextPhase = phase.value
  for (let frame = 0; frame < CHUNK_FRAMES; frame += 1) {
    const sample = Math.sin(nextPhase) * 0.72
    nextPhase += (Math.PI * 2 * 440) / SAMPLE_RATE
    interleaved[frame * CHANNELS] = sample
    interleaved[frame * CHANNELS + 1] = sample
    if (frame % 6 === 0) preview.push(sample)
  }
  phase.value = nextPhase % (Math.PI * 2)
  waveform.value = preview
  state.value = audio.enqueue(interleaved)
}

function dequeue(): void {
  const samples = audio.dequeue(CHUNK_FRAMES)
  const preview: number[] = []
  for (let index = 0; index < samples.length; index += CHANNELS * 6) {
    preview.push(samples[index] ?? 0)
  }
  waveform.value = preview.length > 0 ? preview : Array.from({ length: 160 }, () => 0)
  state.value = audio.state()
}

function overflow(): void {
  for (let index = 0; index < 7; index += 1) enqueueTone()
}

function clear(): void {
  state.value = audio.clear()
  waveform.value = Array.from({ length: 160 }, () => 0)
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="{ fontSize: 26, fontWeight: 700 }">Decoded PCM bridge</div>
    <div :style="{ color: colors.muted }">
      Chunked Float32Array transfer into a bounded native ring buffer—no JSON or per-sample N-API
      calls.
    </div>

    <GpuiCanvas
      :commands="commands"
      :style="{ width: 700, height: 190, borderRadius: 12, overflow: 'hidden' }"
    />

    <div :style="{ display: 'flex', gap: 22 }">
      <div>
        <div :style="{ color: colors.muted }">Queue</div>
        <div :style="{ color: colors.green, fontWeight: 700 }">{{ stateText(state) }}</div>
      </div>
      <div>
        <div :style="{ color: colors.muted }">Dropped</div>
        <div :style="{ color: colors.orange, fontWeight: 700 }">
          {{ state.droppedFrames.toLocaleString() }} frames
        </div>
      </div>
      <div>
        <div :style="{ color: colors.muted }">Format</div>
        <div :style="{ fontWeight: 700 }">{{ SAMPLE_RATE / 1_000 }} kHz stereo f32</div>
      </div>
    </div>

    <div :style="{ display: 'flex', gap: 9, flexWrap: 'wrap' }">
      <div :style="buttonStyle" @click="enqueueTone">Enqueue 1,024 frames</div>
      <div :style="buttonStyle" @click="dequeue">Dequeue 1,024 frames</div>
      <div :style="buttonStyle" @click="overflow">Force overflow</div>
      <div :style="buttonStyle" @click="clear">Clear</div>
    </div>

    <div :style="{ color: colors.muted }">
      This example exercises decoded-frame ingestion and backpressure; connecting the queue to a
      speaker backend is intentionally host-specific.
    </div>
  </div>
</template>
