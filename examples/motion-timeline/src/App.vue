<script setup lang="ts">
import {
  MotionDiv,
  stagger,
  useGpuiTimeline,
  type MotionKeyframe,
  type MotionTransition,
  type StyleDesc,
  type TimelineState,
} from "gpui-vue"
import { computed, ref } from "vue"

const colors = {
  background: "#090d14",
  panel: "#111925",
  card: "#1a2637",
  text: "#f2f6ff",
  muted: "#91a0b8",
  blue: "#61a9ff",
  green: "#59e39d",
  pink: "#ff6da8",
}

const rootStyle: StyleDesc = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
  gap: 18,
  padding: 28,
  background: colors.background,
  color: colors.text,
}

const panelStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  gap: 11,
  padding: 20,
  borderRadius: 14,
  background: colors.panel,
  borderWidth: 1,
  borderColor: "#26344a",
}

const cardStyle: StyleDesc = {
  position: "relative",
  display: "flex",
  alignItems: "center",
  gap: 12,
  height: 52,
  paddingLeft: 16,
  paddingRight: 16,
  borderRadius: 10,
  background: colors.card,
}

const runnerStyle: StyleDesc = {
  position: "relative",
  width: 34,
  height: 10,
  borderRadius: 5,
  background: colors.blue,
}

const buttonStyle: StyleDesc = {
  padding: 9,
  paddingLeft: 13,
  paddingRight: 13,
  borderRadius: 8,
  background: "#27364d",
  color: colors.text,
  cursor: "pointer",
  hover: { opacity: 0.82 },
  active: { opacity: 0.62 },
}

const cards = [
  ["Native keyframes", colors.blue],
  ["Physical springs", colors.green],
  ["Staggered entrance", colors.pink],
  ["Zero JS frame loop", "#c494ff"],
] as const

const runnerKeyframes: readonly MotionKeyframe[] = [
  { at: 0, value: { opacity: 0.4, left: 0 } },
  { at: 0.5, value: { opacity: 1, left: 620 }, ease: "easeInOut" },
  { at: 1, value: { opacity: 0.4, left: 0 }, ease: "easeInOut" },
]

const runnerTransition: MotionTransition = {
  duration: 2.6,
  repeat: 20,
  type: "tween",
}

const timeline = useGpuiTimeline()
const snapshot = ref(timeline.state())
const timelineText = computed(() => {
  const status = snapshot.value.playing ? "playing" : "paused"
  return `${status} · ${snapshot.value.currentTimeMs.toFixed(0)} ms · ${snapshot.value.playbackRate.toFixed(1)}×`
})

function update(next: TimelineState): void {
  snapshot.value = next
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="{ fontSize: 26, fontWeight: 700 }">Native motion timeline</div>
    <div :style="{ color: colors.muted }">
      Animation state is sampled and painted by Rust on GPUI frames.
    </div>

    <div :style="panelStyle">
      <MotionDiv
        v-for="([label, accent], index) in cards"
        :key="label"
        :initial="{ opacity: 0, left: -42, borderRadius: 3 }"
        :animate="{ opacity: 1, left: 0, borderRadius: 10 }"
        :transition="
          stagger(index, 0.09, {
            type: 'spring',
            stiffness: 210,
            damping: 25,
            mass: 0.9,
          })
        "
        :style="cardStyle"
      >
        <div
          :style="{
            width: 9,
            height: 30,
            borderRadius: 5,
            background: accent,
          }"
        />
        <div :style="{ fontWeight: 650 }">{{ label }}</div>
      </MotionDiv>
    </div>

    <MotionDiv
      :initial="{ opacity: 0.4, left: 0 }"
      :animate="runnerKeyframes"
      :transition="runnerTransition"
      :style="runnerStyle"
    />

    <div :style="{ color: colors.muted }">{{ timelineText }}</div>
    <div :style="{ display: 'flex', flexWrap: 'wrap', gap: 9 }">
      <div :style="buttonStyle" @click="update(timeline.play())">Play</div>
      <div :style="buttonStyle" @click="update(timeline.pause())">Pause</div>
      <div :style="buttonStyle" @click="update(timeline.seek(0))">Seek 0 ms</div>
      <div :style="buttonStyle" @click="update(timeline.seek(1_300))">Seek 1,300 ms</div>
      <div :style="buttonStyle" @click="update(timeline.setPlaybackRate(0.5))">0.5×</div>
      <div :style="buttonStyle" @click="update(timeline.setPlaybackRate(1))">1×</div>
      <div :style="buttonStyle" @click="update(timeline.setPlaybackRate(2))">2×</div>
    </div>
  </div>
</template>
