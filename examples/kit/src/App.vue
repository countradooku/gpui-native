<script setup lang="ts">
import type { EventPayload } from "@gpui-native/vue"
import { Button, Checkbox, Input, Select, Switch, Theme } from "@gpui-native/vue/kit"
import { computed, h, ref } from "vue"

import { kitExamples } from "./catalog.js"

const selected = ref<string | null>("kit-button")
const dark = ref(false)
const checked = ref(true)
const name = ref("Hello from GPUI Kit")
const lastEvent = ref("Interact with a component to see its value here.")
const values = ref<Record<string, unknown>>({ ...kitExamples["kit-button"] })
const tags = Object.keys(kitExamples).sort()
function choose(tag: string | null) {
  selected.value = tag
  values.value = { ...kitExamples[tag ?? "kit-button"] }
  lastEvent.value = "Ready"
}
const modelKeys: Record<string, string> = {
  "kit-checkbox": "checked",
  "kit-switch": "checked",
  "kit-radio": "checked",
  "kit-toggle": "checked",
  "kit-pagination": "page",
  "kit-tabs": "selectedIndex",
  "kit-radio-group": "selectedIndex",
  "kit-stepper": "selectedIndex",
  "kit-sidebar": "selectedIndex",
  "kit-sidebar-toggle": "collapsed",
  "kit-accordion": "openIndices",
  "kit-carousel": "selectedIndex",
  "kit-dialog": "open",
  "kit-sheet": "open",
  "kit-popover": "open",
  "kit-notification": "open",
  "kit-dock": "layout",
  "kit-resizable": "sizes",
}
const controlledValueTags = new Set([
  "kit-input",
  "kit-textarea",
  "kit-editor",
  "kit-number-input",
  "kit-otp-input",
  "kit-slider",
  "kit-calendar",
  "kit-date-picker",
  "kit-color-picker",
  "kit-select",
  "kit-combobox",
  "kit-rating",
])
const Preview = () => {
  const tag = selected.value ?? "kit-button"
  return h(
    tag,
    {
      key: tag,
      ...values.value,
      style: {
        width: "100%",
        height:
          tag.includes("chart") ||
          [
            "kit-data-table",
            "kit-tree",
            "kit-dock",
            "kit-list",
            "kit-command",
            "kit-editor",
          ].includes(tag)
            ? 320
            : undefined,
      },
      onChange: (event: EventPayload) => {
        const value: unknown = JSON.parse(event.value ?? "null")
        lastEvent.value = JSON.stringify(value, null, 2)
        const key = modelKeys[tag] ?? (controlledValueTags.has(tag) ? "value" : undefined)
        if (key) values.value = { ...values.value, [key]: value }
      },
    },
    [h("text", { style: { padding: 16 } }, "Framework content — selectable and searchable")],
  )
}
const source = computed(() => JSON.stringify(values.value, null, 2))
</script>

<template>
  <Theme :mode="dark ? 'dark' : 'light'" :style="{ width: '100%', height: '100%' }">
    <div
      :style="{
        width: '100%',
        height: '100%',
        padding: 28,
        display: 'flex',
        flexDirection: 'column',
        gap: 20,
        backgroundColor: dark ? '#111827' : '#f5f7fb',
        color: dark ? '#f1f5f9' : '#182238',
      }"
    >
      <div :style="{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }">
        <div>
          <text :style="{ fontSize: 30, fontWeight: 700 }">GPUI Kit</text>
          <text :style="{ fontSize: 14, marginTop: 6 }"
            >Native components · Vue, React and Svelte · Desktop and browser</text
          >
        </div>
        <Switch v-model="dark" label="Dark theme" />
      </div>
      <div :style="{ display: 'flex', gap: 12, alignItems: 'center' }">
        <Select
          :value="selected"
          :items="tags"
          searchable
          :on-value-change="choose"
          :style="{ width: 310 }"
        />
        <Button label="Reset example" :on-click="() => choose(selected)" />
        <text>{{ tags.length }} components</text>
      </div>
      <div :style="{ display: 'flex', gap: 24, flexGrow: 1, minHeight: 0 }">
        <div
          :style="{
            width: '65%',
            display: 'flex',
            flexDirection: 'column',
            gap: 18,
            padding: 24,
            borderRadius: 14,
            backgroundColor: dark ? '#1e293b' : '#ffffff',
          }"
        >
          <text :style="{ fontSize: 18, fontWeight: 600 }">{{ selected }}</text>
          <Preview />
        </div>
        <div
          :style="{ flexGrow: 1, width: '35%', display: 'flex', flexDirection: 'column', gap: 12 }"
        >
          <text :style="{ fontWeight: 600 }">Properties</text>
          <code
            :code="source"
            language="json"
            :style="{
              maxHeight: 240,
              overflow: 'scroll',
              fontSize: 12,
              backgroundColor: '#0f172a',
              color: '#e2e8f0',
              padding: 12,
              borderRadius: 8,
            }"
          />
          <text :style="{ fontWeight: 600 }">Last value</text>
          <text :style="{ fontSize: 13 }">{{ lastEvent }}</text>
          <text :style="{ fontWeight: 600, marginTop: 12 }">Two-way binding</text>
          <Input v-model="name" />
          <Checkbox v-model="checked" label="Enabled" />
          <text>{{ name }} · {{ checked ? "enabled" : "disabled" }}</text>
        </div>
      </div>
    </div>
  </Theme>
</template>
