<script setup lang="ts">
import { useWindowSize } from "@gpui-native/vue"
import { computed, ref } from "vue"

import Canvases from "./Canvases.vue"
defineProps<{ gpu: GPU }>()
const visible = ref(true)
const wide = ref(false)
const size = useWindowSize()
const canvasWidth = computed(() =>
  Math.max(1, Math.min(wide.value ? 720 : 480, size.value.width - 48)),
)
</script>
<template>
  <div
    :style="{
      width: '100%',
      height: '100%',
      overflow: 'scroll',
      padding: 24,
      gap: 12,
      display: 'flex',
      flexDirection: 'column',
      backgroundColor: '#070b17',
      color: '#e8efff',
    }"
  >
    <text :style="{ fontSize: 26 }">Vue · shared GPU canvases</text>
    <div
      :style="{ display: 'flex', flexDirection: 'row', flexWrap: 'wrap', flexShrink: 0, gap: 12 }"
    >
      <div
        role="button"
        :style="{ padding: 10, backgroundColor: '#23304a', borderRadius: 6 }"
        @click="visible = !visible"
      >
        {{ visible ? "Unmount canvases" : "Mount canvases" }}
      </div>
      <div
        role="button"
        :style="{ padding: 10, backgroundColor: '#23304a', borderRadius: 6 }"
        @click="wide = !wide"
      >
        Resize canvases
      </div>
    </div>
    <Canvases v-if="visible" :gpu="gpu" :width="canvasWidth" />
  </div>
</template>
