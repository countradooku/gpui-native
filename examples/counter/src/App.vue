<script setup lang="ts">
import {
  GpuiCode,
  GpuiInput,
  MotionDiv,
  type EventPayload,
  type MotionStyle,
  type MotionTransition,
  type StyleDesc,
} from "gpui-vue"
import { ref } from "vue"

const palette = {
  background: "#10131a",
  panel: "#191e29",
  text: "#eef2ff",
  muted: "#98a2b3",
  accent: "#7c9cff",
}

const count = ref(0)
const label = ref("Counter")

const rootStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  width: "100%",
  height: "100%",
  gap: 18,
  padding: 32,
  background: palette.background,
  color: palette.text,
}

const cardStyle: StyleDesc = {
  position: "relative",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 14,
  padding: 26,
  minWidth: 280,
  background: palette.panel,
  borderRadius: 16,
  borderWidth: 1,
  borderColor: "#293044",
}

const inputStyle: StyleDesc = {
  width: "100%",
  padding: 10,
  borderRadius: 8,
  borderWidth: 1,
  borderColor: "#3a435a",
  background: "#11151e",
}

const buttonStyle: StyleDesc = {
  padding: 12,
  paddingLeft: 20,
  paddingRight: 20,
  borderRadius: 10,
  background: palette.accent,
  color: "#0b1020",
  fontWeight: 700,
  cursor: "pointer",
  hover: { opacity: 0.88 },
  active: { opacity: 0.7 },
}

// <code> is deliberately a bare primitive; the app owns its card surface.
const codeStyle: StyleDesc = {
  width: "100%",
  padding: 12,
  background: "#11151e",
  borderWidth: 1,
  borderColor: "#293044",
  borderRadius: 8,
}

const entrance: MotionStyle = { opacity: 1, top: 0 }
const entranceTransition: MotionTransition = {
  duration: 0.35,
  ease: "easeOut",
}

function increment(): void {
  count.value += 1
}

function handleKeyDown(event: EventPayload): void {
  if (event.key === "enter" || event.key === "space") increment()
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="{ fontSize: 30, fontWeight: 700 }">gpui-vue</div>
    <div :style="{ color: palette.muted }">Vue 3 reactivity rendered by Zed GPUI</div>

    <MotionDiv
      :initial="{ opacity: 0, top: 8 }"
      :animate="entrance"
      :transition="entranceTransition"
      :style="cardStyle"
    >
      <GpuiInput v-model="label" placeholder="Counter label" :style="inputStyle" />

      <div testId="count" :style="{ fontSize: 48, fontWeight: 700 }">{{ label }}: {{ count }}</div>

      <div
        testId="increment"
        :tabIndex="0"
        :style="buttonStyle"
        @click="increment"
        @key-down="handleKeyDown"
      >
        Increment
      </div>

      <GpuiCode
        :code="`const count = ${count}`"
        language="typescript"
        :showLineNumbers="false"
        :style="codeStyle"
      />
    </MotionDiv>
  </div>
</template>
