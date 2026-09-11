# Components

`@gpui-native/vue`, `@gpui-native/react`, and `@gpui-native/svelte` export these names, including
from their `/web` entries. Named components preserve the native host props and
lower directly to the existing mutation protocol. GPUI still owns layout,
painting, input, text selection, scrolling and motion.

| Component                    | Purpose                                              | Legacy Vue/React name      |
| ---------------------------- | ---------------------------------------------------- | -------------------------- |
| `View`                       | Bare container with all GPUI styles and events       | `GpuiDiv`                  |
| `Text`                       | Native selectable, searchable text                   | `GpuiTextElement`          |
| `TextInput`                  | Single-line native editor                            | `GpuiInput`                |
| `TextArea`                   | Multiline native editor                              | `GpuiTextarea`             |
| `Image`                      | Raster image                                         | `GpuiImage`                |
| `Svg`                        | Vector image                                         | `GpuiSvg`                  |
| `Canvas`                     | Retained drawing and GPU canvas                      | `GpuiCanvas`               |
| `Anchored`                   | GPUI positioned floating content                     | `GpuiAnchored`             |
| `Code`                       | Bare syntax-highlighted code                         | `GpuiCode`                 |
| `Diff`                       | Native diff viewer                                   | `GpuiDiff`                 |
| `Markdown`                   | Native Markdown content                              | `GpuiMarkdown`             |
| `VirtualList`                | Virtualized rows                                     | `GpuiVirtualList`          |
| `MotionView` / `motion.View` | Container animated by GPUI                           | `MotionDiv` / `motion.div` |
| `Row`                        | Flex container, horizontal by default                | —                          |
| `Column`                     | Flex container, vertical by default                  | —                          |
| `ScrollView`                 | Scroll container, vertical by default                | —                          |
| `Button`                     | Unstyled action with pointer and keyboard activation | —                          |

`ViewProps`, `TextProps`, `ImageProps`, `TextInputProps`, `TextAreaProps`,
`ScrollViewProps`, `ButtonProps`, and `MotionViewProps` accompany the component
names. Other primitives retain their existing prop type names. Vue editors keep
`v-model`; React editors keep `value` / `onChange`; Svelte editors use `bind:value`.
See [the Svelte guide](svelte.md) for its native compiler and rune APIs.

## Layout without restrictions

`View` has no added layout defaults. `Row` and `Column` set `display: "flex"` and
`flexDirection`. `ScrollView` uses the same flex defaults and scrolls vertically;
`horizontal` switches both direction and scrolling axis. Supply a bounded height
or width for the viewport. `style` overrides all layout defaults. Children stay
directly in the host container, so there are no extra wrappers or content styles.
Use `VirtualList` for large collections that should only render visible rows.

All native props remain available: grid/flex styles, positioning, pseudo-state
styles, event handlers, `highlight`, `motion`, and accessibility labels. You can
mix these components with raw host tags, including custom native elements.
`Code` remains bare; your app supplies its background, padding, border and header.

## Buttons

```vue
<script setup lang="ts">
import { Button, Row, Text } from "@gpui-native/vue"
import { ref } from "vue"
const count = ref(0)
</script>

<template>
  <Row :style="{ gap: 12 }">
    <Text>{{ count }}</Text>
    <Button :style="{ padding: 12 }" @press="count++">Increment</Button>
    <Button :disabled="count === 0" :style="{ padding: 12 }" @press="count = 0">Reset</Button>
  </Row>
</template>
```

In React and Svelte, pass `onPress` and ordinary children. A press callback receives the
original native `EventPayload`: `click`, `keyDown` for Enter, or `keyUp` for Space.
Space must start on this button; blur or disabling cancels it. Key repeats and
Ctrl/Alt/Cmd shortcuts do not activate. Use `onClick`, `onKeyDown`, `onKeyUp`, and
`onBlur` for raw events alongside `onPress`; do not bind the same action to both
`onClick` and `onPress`, since a pointer click calls both.

`disabled` suppresses press and click callbacks, disables autofocus, and sets
`tabIndex: -1`. Raw key/focus callbacks remain available. It does not disable
arbitrary interactive descendants; compose a button from text, icons and layouts.
The app controls disabled appearance. The button adds no fill, padding, border,
or size. It defaults to a pointer cursor and nonselectable text, both overridable.
It uses GPUI's button role and accessibility click action; `aria-label` is useful
for icon-only buttons. Disabled buttons remove the click action; this adapter
currently does not expose a native accessibility disabled-state property.

React refs point at the native host node. Vue Button component refs expose its
native `id`, suitable for `useGpuiWindow().focus()`. The other named primitives
and layout helpers use the existing native element refs.

## Migration

Replace imports and tags using the table above. The old `Gpui*` components and
`MotionDiv` are aliases of the new components, so existing apps continue to work.
Raw lowercase host tags also remain supported. Svelte starts with the new names and does not export legacy aliases. `Select`, `Combobox`, `Tooltip`,
and their parts keep their names.

One Vue export changes meaning: `Text` now means the GPUI text component. If you
previously imported Vue's internal text VNode symbol from this adapter, import
it directly from `vue` / `@vue/runtime-core` instead.
