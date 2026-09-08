<script setup lang="ts">
import type { StyleDesc } from "@gpui-native/vue"

import { inspectorOpen, sharedCount } from "./state.js"
import { closeInspector, openInspector } from "./windows.js"

const colors = {
  background: "#0d1119",
  panel: "#171e2b",
  text: "#f1f5ff",
  muted: "#96a2b8",
  accent: "#6ea8ff",
}

const rootStyle: StyleDesc = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
  justifyContent: "center",
  gap: 16,
  padding: 28,
  background: colors.background,
  color: colors.text,
}

const panelStyle: StyleDesc = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: 22,
  borderRadius: 14,
  background: colors.panel,
  borderWidth: 1,
  borderColor: "#2b3a51",
}

const buttonStyle: StyleDesc = {
  padding: 10,
  paddingLeft: 15,
  paddingRight: 15,
  borderRadius: 9,
  background: "#2a3953",
  color: colors.text,
  cursor: "pointer",
  hover: { opacity: 0.84 },
  active: { opacity: 0.64 },
}

function increment(): void {
  sharedCount.value += 1
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="{ color: colors.accent, fontWeight: 700 }">PRIMARY WINDOW</div>
    <div :style="{ fontSize: 27, fontWeight: 700 }">Independent Vue roots</div>
    <div :style="{ color: colors.muted }">
      Both native windows react to the same module-level ref without a browser DOM.
    </div>

    <div :style="panelStyle">
      <div :style="{ color: colors.muted }">Shared count</div>
      <div :style="{ fontSize: 44, fontWeight: 750 }">{{ sharedCount }}</div>
    </div>

    <div :style="{ display: 'flex', gap: 10, flexWrap: 'wrap' }">
      <div :style="buttonStyle" @click="increment">Increment</div>
      <div v-if="inspectorOpen" :style="buttonStyle" @click="closeInspector">Close inspector</div>
      <div v-else :style="buttonStyle" @click="openInspector">Open inspector</div>
    </div>
  </div>
</template>
