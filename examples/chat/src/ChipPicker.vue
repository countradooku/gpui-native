<script setup lang="ts">
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  type SelectItemState,
  type StyleDesc,
} from "gpui-vue"
import { computed } from "vue"

import ChatIcon from "./ChatIcon.vue"
import { colors, type PickerOption } from "./data.js"

const props = withDefaults(
  defineProps<{
    options: PickerOption[]
    accent?: boolean
    caret?: boolean
    menuWidth?: number
  }>(),
  { accent: false, caret: true, menuWidth: 220 },
)

const model = defineModel<string>({ required: true })
const selected = computed<PickerOption>(
  () =>
    props.options.find((option: PickerOption) => option.id === model.value) ?? props.options[0]!,
)
const menuStyle = computed<StyleDesc>(() => ({
  minWidth: props.menuWidth,
  padding: 4,
  background: colors.raised,
  borderWidth: 1,
  borderColor: colors.borderStrong,
  borderRadius: 12,
}))

function rowStyle(state: SelectItemState): StyleDesc {
  return {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
    width: "100%",
    paddingTop: 6,
    paddingBottom: 6,
    paddingLeft: 8,
    paddingRight: 8,
    borderRadius: 7,
    background: state.highlighted ? "#404040" : state.selected ? "#2C2C2C" : colors.raised,
    hover: { background: "#404040" },
  }
}
</script>

<template>
  <Select v-model="model" :style="{ flexShrink: 0 }">
    <div :style="{ position: 'relative', display: 'flex' }">
      <SelectTrigger
        :style="{
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          gap: 6,
          height: 26,
          paddingLeft: 7,
          paddingRight: 7,
          borderRadius: 6,
          cursor: 'pointer',
          hover: { background: colors.overlay },
        }"
      >
        <ChatIcon
          :name="selected.icon"
          :size="12"
          :color="accent ? colors.accent : colors.tertiary"
        />
        <text
          :style="{
            fontSize: 13,
            lineHeight: 16,
            color: accent ? colors.accent : colors.secondary,
            whiteSpace: 'nowrap',
            textOverflow: 'ellipsis',
          }"
        >
          {{ selected.label }}
        </text>
        <ChatIcon v-if="caret" name="chevronDown" :size="10.5" :color="colors.ghost" />
      </SelectTrigger>
      <SelectContent side="top" :side-offset="4" :style="menuStyle">
        <SelectItem
          v-for="option in options"
          :key="option.id"
          :value="option.id"
          :text-value="option.label"
        >
          <template #default="state">
            <div :style="rowStyle(state)">
              <ChatIcon :name="option.icon" :size="14" :color="colors.tertiary" />
              <text
                :style="{
                  fontSize: 12.5,
                  fontWeight: state.selected ? 600 : 500,
                  color: colors.text,
                  flexGrow: 1,
                  whiteSpace: 'nowrap',
                  textOverflow: 'ellipsis',
                }"
              >
                {{ option.label }}
              </text>
              <ChatIcon v-if="state.selected" name="check" :size="11" :color="colors.tertiary" />
            </div>
          </template>
        </SelectItem>
      </SelectContent>
    </div>
  </Select>
</template>
