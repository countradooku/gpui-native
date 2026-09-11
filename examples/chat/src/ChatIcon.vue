<script setup lang="ts">
import { Svg, type StyleDesc } from "@gpui-native/vue"
import { computed } from "vue"

import { icons, type IconName } from "./icons.js"

const props = withDefaults(
  defineProps<{
    name: IconName
    size?: number
    color: string
  }>(),
  { size: 14 },
)

const style = computed<StyleDesc>(() => ({
  width: props.size,
  height: props.size,
  flexShrink: 0,
  color: props.color,
}))
</script>

<template>
  <Svg :source="icons[name]" :style="style" />
</template>
