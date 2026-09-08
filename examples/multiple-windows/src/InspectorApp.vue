<script setup lang="ts">
import type { StyleDesc } from "@gpui-native/vue"

import { sharedCount } from "./state.js"

const colors = {
  background: "#0d1119",
  panel: "#171e2b",
  text: "#f1f5ff",
  muted: "#96a2b8",
  green: "#5ee0a0",
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
  padding: 24,
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
    <div :style="{ color: colors.green, fontWeight: 700 }">SECOND WINDOW</div>
    <div :style="{ fontSize: 26, fontWeight: 700 }">Live inspector</div>

    <div :style="panelStyle">
      <div :style="{ color: colors.muted }">Shared Vue ref</div>
      <div :style="{ fontSize: 54, fontWeight: 750 }">{{ sharedCount }}</div>
    </div>

    <div :style="buttonStyle" @click="increment">Increment from inspector</div>
    <div :style="{ color: colors.muted }">
      This window has its own renderer, retained tree, timeline, and event scope.
    </div>
  </div>
</template>
