<script setup lang="ts">
import { ref } from "vue"

import Canvases from "./Canvases.vue"
defineProps<{ gpu: GPU }>()
const visible = ref(true)
const wide = ref(false)
</script>
<template>
  <div
    :style="{
      width: '100%',
      height: '100%',
      padding: 24,
      gap: 12,
      display: 'flex',
      flexDirection: 'column',
      backgroundColor: '#070b17',
      color: '#e8efff',
    }"
  >
    <text :style="{ fontSize: 26 }">Vue · shared GPU canvases</text>
    <div :style="{ display: 'flex', flexDirection: 'row', gap: 12 }">
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
    <Canvases v-if="visible" :gpu="gpu" :width="wide ? 720 : 480" />
  </div>
</template>
