<script setup lang="ts">
import { GpuiCanvas, useGPUCanvas } from "@gpui-native/vue"
import { onMounted, onScopeDispose, ref } from "vue"

import { startWebGPUScene } from "../../shared/webgpu-scene.js"
const props = defineProps<{ gpu: GPU }>()
const canvas = useGPUCanvas({ width: 640, height: 400 })
const error = ref("")
let cancelled = false
let stop: (() => void) | undefined
onMounted(() => {
  void startWebGPUScene(canvas, props.gpu, (value) => {
    error.value = String(value)
  })
    .then((dispose) => {
      if (cancelled) dispose()
      else stop = dispose
    })
    .catch((value) => {
      if (!cancelled) error.value = String(value)
    })
})
onScopeDispose(() => {
  cancelled = true
  stop?.()
})
</script>
<template>
  <div
    :style="{
      width: '100%',
      height: '100%',
      padding: 24,
      gap: 16,
      flexDirection: 'column',
      backgroundColor: '#070b17',
      color: '#e8efff',
    }"
  >
    <text :style="{ fontSize: 26 }">Vue · WebGPU canvas</text>
    <text>WGSL shaders · 4× antialiasing · bounded asynchronous presentation</text>
    <GpuiCanvas
      :source="canvas.id"
      testId="webgpu-canvas"
      :style="{ width: 640, height: 400 }"
      :commands="[
        {
          type: 'rect',
          x: 12,
          y: 12,
          width: 616,
          height: 376,
          radius: 12,
          stroke: '#6686b0',
          strokeWidth: 1,
        },
      ]"
    />
    <text v-if="error" :style="{ color: '#ff8888' }">{{ error }}</text>
  </div>
</template>
