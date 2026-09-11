<script setup lang="ts">
import { Canvas, useGPUCanvas } from "@gpui-native/vue"
import { onMounted, onScopeDispose, ref, watch } from "vue"

import { startThreeScene } from "../../shared/three-scene.js"
import { startWebGPUScene } from "../../shared/webgpu-scene.js"
const props = defineProps<{ gpu: GPU; width: number }>()
const compute = useGPUCanvas({ width: props.width, height: 300 })
const three = useGPUCanvas({ width: props.width, height: 300 })
watch(
  () => props.width,
  (width) => {
    compute.resize(width, 300)
    three.resize(width, 300)
  },
)
const error = ref("")
let cancelled = false
const stops: (() => void)[] = []
onMounted(() => {
  for (const [canvas, start] of [
    [compute, startWebGPUScene],
    [three, startThreeScene],
  ] as const) {
    void start(canvas, props.gpu, (value) => {
      error.value = String(value)
    })
      .then((stop) => {
        if (cancelled) stop()
        else stops.push(stop)
      })
      .catch((value) => {
        if (!cancelled) error.value = String(value)
      })
  }
})
onScopeDispose(() => {
  cancelled = true
  for (const stop of stops) stop()
})
</script>
<template>
  <div :style="{ display: 'flex', flexDirection: 'column', gap: 12 }">
    <text>Compute + WGSL · 4× MSAA</text>
    <Canvas :source="compute.id" testId="webgpu-canvas" :style="{ width, height: 300 }" />
    <text>Textured Three.js · depth + 4× MSAA</text>
    <Canvas :source="three.id" testId="three-canvas" :style="{ width, height: 300 }" />
    <text v-if="error" :style="{ color: '#ff8888' }">{{ error }}</text>
  </div>
</template>
