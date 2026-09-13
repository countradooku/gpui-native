# GPUI Kit

GPUI Native embeds GPUI Kit 0.6.1 at revision `84f57fdfcb4910623fb0bb7f795b077e249f9271`. The same native component implementations run behind Vue, React and Svelte, on the desktop and in the single-threaded WebAssembly renderer. Icons are embedded in both builds.

The adapters are available from `@gpui-native/vue/kit`, `@gpui-native/react/kit`, and `@gpui-native/svelte/kit`. Their complete prop and event types are exported from those entry points. Existing GPUI Native primitives and adapter components remain available from their original entry points.

## Try the gallery

```sh
bun install
bun run build:native
bun --filter @gpui-vue/example-kit build
bun --filter @gpui-vue/example-kit start
```

`bun run build:pages` includes the gallery at `dist-pages/examples/kit/`. Serve `dist-pages` through an HTTP server to use the WebAssembly build. It does not require shared memory, a separate icon server, or COOP/COEP headers.

The gallery lets you choose every exported component, edit native controls, inspect emitted values, reset examples, and switch themes. Its catalog is also used by the native rendering test, which checks that every exported component has a rendering case.

## Framework bindings

Vue supports default and named models:

```vue
<script setup lang="ts">
import { ref } from "vue"
import { Checkbox, Input, Slider, Theme } from "@gpui-native/vue/kit"
const enabled = ref(true)
const name = ref("Ada")
const range = ref<[number, number]>([20, 80])
</script>

<template>
  <Theme mode="light">
    <Input v-model="name" placeholder="Name" />
    <Checkbox v-model:checked="enabled" label="Enabled" />
    <Slider v-model="range" :min="0" :max="100" />
  </Theme>
</template>
```

React uses controlled props and typed value callbacks:

```tsx
import { useState } from "react"
import { Checkbox, Input } from "@gpui-native/react/kit"

export function Preferences() {
  const [enabled, setEnabled] = useState(true)
  const [name, setName] = useState("Ada")
  return (
    <>
      <Input value={name} onValueChange={setName} />
      <Checkbox checked={enabled} label="Enabled" onValueChange={setEnabled} />
    </>
  )
}
```

Svelte supports the corresponding bound native property:

```svelte
<script lang="ts">
  import { Checkbox, Input, Dialog } from "@gpui-native/svelte/kit"
  let enabled = $state(true)
  let name = $state("Ada")
  let open = $state(false)
</script>

<Input bind:value={name} />
<Checkbox bind:checked={enabled} label="Enabled" />
<Dialog bind:open title="Preferences">
  <Input bind:value={name} />
</Dialog>
```

`onValueChange` receives decoded native values. `onChange` receives the existing GPUI Native event payload; its `value` field contains JSON. Both callbacks can be supplied together. Input `onSubmit` uses the normal submit payload. Calendar dates are ISO `YYYY-MM-DD` strings; range selection uses a two-element tuple. NumberInput uses a string so an incomplete edit such as `-` remains representable.

Arrays, objects, styles and semantic tokens remain ordinary objects until the outer `applyBatch` serialization. Do not JSON-stringify individual component props.

## Ownership and composition

Kit owns native interaction state: input caret and undo history, popup focus, table navigation, dock layout, and scroll position. Frameworks own application data and reconciliation. Echoing an editor value preserves its caret; changed or removed props are synchronized into the native state. Unmounting a host releases its entities and subscriptions. Dialogs, sheets and notifications are owned by their host, so removing one host does not close unrelated overlays.

Use `style` for retained layout and host chrome. Kit's `size`, `variant`, and `Theme` configure component appearance. Kit has its own semantic theme; existing code/diff/markdown primitives continue to use the GPUI Native theme contract. `Theme` and `locale` are application-wide, matching Kit, and are not scoped CSS providers. Mount one stable Theme near the root. Removing it does not roll back the global theme.

Most controls accept native children. Compound components use these conventions:

| Component                                       | Child mapping                                                                    |
| ----------------------------------------------- | -------------------------------------------------------------------------------- |
| Tabs                                            | One child per tab; only the selected tab's child is displayed                    |
| Accordion, DescriptionList, Form                | One child per item                                                               |
| Dialog, Sheet, Popover, HoverCard, Notification | Children render in the native overlay                                            |
| Dock                                            | One child per entry in `panels`, with stable panel `id` values                   |
| Resizable                                       | One child per panel; `sizes` corresponds to child order                          |
| Settings                                        | One child per settings item in page/group/item order                             |
| MessageScroller                                 | One child per message; decrease `firstIndex` when prepending history             |
| Attachment                                      | Children are actions; `title`, `description` and `src` provide content and media |
| Tooltip                                         | Children are the trigger; `label` is the tooltip                                 |
| Sidebar                                         | First child is the header; remaining children are the footer                     |
| StatusBar                                       | First child is the left slot; remaining children are the right slot              |

Popover and HoverCard use `label` as their trigger. Dialog, Sheet and Notification use `open`; their close callback requests `false`. Use a controlled callback/model when the framework must reflect user dismissal. Notifications currently use in-app delivery. Window chrome components operate within the host's existing window configuration.

DataTable receives `columns` and `rows`. Its typed change events distinguish selection, sorting, column movement/resizing, context activation, and double clicks. Sorting is a request to the framework: update `rows` with the desired ordering. `onVisibleRange` reports virtualization separately. List and Command distinguish selection, confirmation, cancellation, and command search requests. Tree events distinguish selection from expansion. These event objects are not default two-way models.

Dock's `layout` is the serializable native dock state. Save the value from `onValueChange` and pass it back to restore layout; panel IDs must exist in `panels`, and duplicate references are rejected. Removing `layout` restores the default panel placement. A panel closed inside the dock remains in the framework's panel catalog until the application removes it.

Charts accept data objects with `label` and `value`; candlesticks instead require `open`, `high`, `low` and `close`. Sankey charts use labeled nodes in `data` and index-based `source`/`target` links. These are declarative single-series chart adapters. Low-level Rust plot builders and arbitrary Rust delegates are not JavaScript values.

Document labels, table cells, tree rows, chart labels and animated text participate in the shared selection and search pipeline. Editable controls retain Kit's native caret, selection, IME and undo implementation. Editor syntax highlighting uses the same bounded Syntect engine as existing code blocks on native and web; parser-driven code folding and language-server integration are not exposed by this adapter.

## Native component catalog

The public TypeScript types are the authoritative property reference. Each entry below is available as a named component from all three `/kit` packages and as a registered native host tag.

| Component          | Native tag              |
| ------------------ | ----------------------- |
| `Accordion`        | `kit-accordion`         |
| `Alert`            | `kit-alert`             |
| `AreaChart`        | `kit-area-chart`        |
| `Attachment`       | `kit-attachment`        |
| `AttachmentGroup`  | `kit-attachment-group`  |
| `Avatar`           | `kit-avatar`            |
| `Badge`            | `kit-badge`             |
| `BarChart`         | `kit-bar-chart`         |
| `Breadcrumb`       | `kit-breadcrumb`        |
| `Bubble`           | `kit-bubble`            |
| `BubbleGroup`      | `kit-bubble-group`      |
| `Button`           | `kit-button`            |
| `Calendar`         | `kit-calendar`          |
| `CandlestickChart` | `kit-candlestick-chart` |
| `Carousel`         | `kit-carousel`          |
| `Checkbox`         | `kit-checkbox`          |
| `Clipboard`        | `kit-clipboard`         |
| `Collapsible`      | `kit-collapsible`       |
| `ColorPicker`      | `kit-color-picker`      |
| `Combobox`         | `kit-combobox`          |
| `Command`          | `kit-command`           |
| `ContextMenu`      | `kit-context-menu`      |
| `DataTable`        | `kit-data-table`        |
| `DatePicker`       | `kit-date-picker`       |
| `DescriptionList`  | `kit-description-list`  |
| `Dialog`           | `kit-dialog`            |
| `Dock`             | `kit-dock`              |
| `DropdownMenu`     | `kit-dropdown-menu`     |
| `Editor`           | `kit-editor`            |
| `Empty`            | `kit-empty`             |
| `Field`            | `kit-field`             |
| `Form`             | `kit-form`              |
| `GroupBox`         | `kit-group-box`         |
| `HoverCard`        | `kit-hover-card`        |
| `Icon`             | `kit-icon`              |
| `Input`            | `kit-input`             |
| `Kbd`              | `kit-kbd`               |
| `Label`            | `kit-label`             |
| `LineChart`        | `kit-line-chart`        |
| `Link`             | `kit-link`              |
| `List`             | `kit-list`              |
| `Marker`           | `kit-marker`            |
| `Message`          | `kit-message`           |
| `MessageContent`   | `kit-message-content`   |
| `MessageFooter`    | `kit-message-footer`    |
| `MessageGroup`     | `kit-message-group`     |
| `MessageHeader`    | `kit-message-header`    |
| `MessageScroller`  | `kit-message-scroller`  |
| `Notification`     | `kit-notification`      |
| `NumberInput`      | `kit-number-input`      |
| `OtpInput`         | `kit-otp-input`         |
| `Pagination`       | `kit-pagination`        |
| `PieChart`         | `kit-pie-chart`         |
| `Popover`          | `kit-popover`           |
| `Progress`         | `kit-progress`          |
| `ProgressCircle`   | `kit-progress-circle`   |
| `RadarChart`       | `kit-radar-chart`       |
| `Radio`            | `kit-radio`             |
| `RadioGroup`       | `kit-radio-group`       |
| `Rating`           | `kit-rating`            |
| `Resizable`        | `kit-resizable`         |
| `SankeyChart`      | `kit-sankey-chart`      |
| `Select`           | `kit-select`            |
| `Separator`        | `kit-separator`         |
| `Settings`         | `kit-settings`          |
| `Sheet`            | `kit-sheet`             |
| `ShimmerText`      | `kit-shimmer-text`      |
| `Sidebar`          | `kit-sidebar`           |
| `SidebarFooter`    | `kit-sidebar-footer`    |
| `SidebarHeader`    | `kit-sidebar-header`    |
| `SidebarToggle`    | `kit-sidebar-toggle`    |
| `Skeleton`         | `kit-skeleton`          |
| `Slider`           | `kit-slider`            |
| `Spinner`          | `kit-spinner`           |
| `StatusBar`        | `kit-status-bar`        |
| `Stepper`          | `kit-stepper`           |
| `Switch`           | `kit-switch`            |
| `Tabs`             | `kit-tabs`              |
| `Tag`              | `kit-tag`               |
| `Textarea`         | `kit-textarea`          |
| `Theme`            | `kit-theme`             |
| `TitleBar`         | `kit-title-bar`         |
| `Toggle`           | `kit-toggle`            |
| `Tooltip`          | `kit-tooltip`           |
| `Tree`             | `kit-tree`              |
| `WindowBorder`     | `kit-window-border`     |

## Why the GPUI fork remains

GPUI Native pins `countradooku/zed` at `5fc2db20b183d305a1de5166ee1c42ad2b7794a6`. Its additional GPU surface changes provide leased external surfaces, alpha opacity and lease retention through compositor completion. Those changes support the existing WebGPU canvas integration and prevent an external texture from being released before the compositor has finished using it. They are independent of adopting Kit.

Kit normally depends on its published GPUI package family. The vendored manifest aligns that family to the existing GPUI Native revision. This keeps a single set of GPUI Rust types and preserves the canvas integration. Replacing the fork with upstream requires those surface capabilities to land upstream or an equivalent compatibility change first. See [the WebGPU integration](webgpu.md) and [the vendor patch record](../vendor/gpui-kit/UPSTREAM.md).

The upstream shell/CLI, its separate scripting runtime, sample applications, and low-level Rust-only utilities are not separate JavaScript components. GPUI Native supplies its own application/window host, retained mutation protocol, rendering loop, and framework runtimes.
