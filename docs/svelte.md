# Svelte 5

`packages/svelte` provides the `@gpui-native/svelte` adapter for native GPUI
windows and the shared GPUI WebAssembly renderer. Write `.svelte` components
using `$state`, `$derived`, `$effect`, `$props`, `$bindable`, snippets, keyed
`{#each}`, `{#if}`, and `{#await}`. The adapter uses Svelte **5.57.0**, pinned
exactly because its compiled output and client runtime must agree.

This adapter is part of the early `0.1.x` codebase. Its tests and examples cover
native and browser integration; it does not make GPUI's existing experimental
platform or WebGPU capabilities production-qualified on every device.

## Run an example

From a checkout with Bun and the repository's Rust toolchain installed:

```sh
bun install
bun run build:native
bun run build:adapters
bun --filter @gpui-svelte/example-counter build
bun --filter @gpui-svelte/example-counter start
```

Other examples are `svelte-showcase`, `svelte-multiple-windows`, and
`svelte-webgpu`. Each supports `build`, `start`, and `build:binary`. All four
also have entries in the browser gallery produced by `bun run build:pages`.

## Components and runes

```svelte
<script lang="ts">
  import { Button, Column, Text, TextInput } from "@gpui-native/svelte"
  let name = $state("Svelte")
  let count = $state(0)
  const message = $derived(`Hello ${name}: ${count}`)
</script>

<Column style={{ padding: 24, gap: 12 }}>
  <TextInput bind:value={name} placeholder="Your name" />
  <Text>{message}</Text>
  <Button onPress={() => count++} style={{ padding: 12 }}>Increment</Button>
</Column>
```

```ts
// src/main.ts
import { render } from "@gpui-native/svelte"
import App from "./App.svelte"
render(App, { title: "Svelte + GPUI", width: 800, height: 600 })
```

Use the adapter's compiler plugin instead of the browser Svelte plugin:

```ts
// vite.config.ts — native executable entry
import { defineConfig } from "vite"
import { gpuiSvelte } from "@gpui-native/svelte/vite"

export default defineConfig({
  plugins: [gpuiSvelte()],
  build: {
    target: "node20",
    lib: { entry: "src/main.ts", formats: ["es"], fileName: "main" },
    rollupOptions: { external: [/^@gpui-native\//] },
  },
})
```

The plugin compiles components and `.svelte.ts` / `.svelte.js` rune modules.
It redirects `svelte`, `svelte/store`, and `svelte/reactivity` imports to the
same isolated client runtime, including imports from ordinary utility modules.
Use `svelte-check` for strict component checking. Development updates reload the
application; preservation of component state across hot reload is not implemented.

The component names match Vue and React: `View`, `Text`, `Row`, `Column`,
`ScrollView`, `Button`, `Image`, `Svg`, `TextInput`, `TextArea`, `Code`, `Diff`,
`Markdown`, `Canvas`, `VirtualList`, `Anchored`, and `MotionView` / `motion.View`.
[The component guide](components.md) describes their GPUI behavior. Styles,
pseudo-state styles, events, accessibility, highlighting, and motion remain
native props. `Code` is bare; applications supply padding, border and fill.

Named components accept camel-case callbacks such as `onPress`, `onClick`, and
`onChange`. Primitives also accept Svelte's lowercase spelling (`onpress`,
`onclick`, `onchange`). Callbacks receive the shared native `EventPayload`,
not a DOM Event. Native input uses `bind:value`; `onChange` additionally observes
the event. Host values remain structured through batching. Deep changes to
`$state` style objects are tracked and removed properties reset the native state.

Raw native tags such as `<div>`, `<text>`, `<code>`, `<input>` and
`<virtual-list>` are also compiled to these primitives. Named components provide
the strongest editor typing. This is a native target: HTML elements outside the
native catalog, CSS stylesheets/directives, DOM actions, `{@html}`, browser
special elements, transitions and animations are rejected by the compiler.
Use native `style`, `MotionView`, window APIs, and `Canvas` for those operations.
DOM-dependent Svelte libraries, SSR and hydration are not supported.

## Refs, effects and windows

```svelte
<script lang="ts">
  import { TextInput, useElementRef, useGpuiWindow } from "@gpui-native/svelte"
  const input = useElementRef()
  const window = useGpuiWindow()
  $effect(() => {
    if (input.current) window.focus(input.current)
  })
</script>
<TextInput bind:ref={input.current} />
```

A native ref exposes `id`. It is cleared on removal. `bind:this` returns the
primitive's exported native handle as well. Use native window operations to
focus, scroll or inspect bounds; a ref is not a browser element.

`useWindowSize()` and `useWindowInsets()` return objects with reactive fields.
Read their fields inside markup, derived expressions or effects rather than
copying them into nonreactive locals. Resize events are preferred; custom
renderers without events use a disposable polling fallback.

`useGpuiWindow()`, `useGpuiTimeline()`, `useGpuiAudioFrames()`, and
`useWindowEvent("windowKeyDown", callback)` expose shared engine capabilities.
Window subscriptions compose with `render`'s `onKeyDown` / `onKeyUp` callbacks
and are removed when their Svelte component is destroyed.

`createWindow(App, { props, ...windowOptions })` owns an independent tree and
component lifecycle. `root.render(App, { props })` updates the existing
component's props and preserves its rune state; changing the component remounts
it. `root.unmount()` removes content, and `root.close()` also stops the event
loop and closes the window. `render()` reuses one persistent window;
`resetRender()` closes it. Runtime roots include a native error display and an
`onError` callback. Lower-level `createGpuiRenderer()` surfaces failures to its
caller and supports an injected backend.

## Headless controls

Select, Combobox, Tooltip and FloatingLayer are Svelte components. They use
runes and Svelte context and reuse native focus, floating layout and events.
They are unstyled and accept ordinary host props.

```svelte
<script lang="ts">
  import {
    Select, SelectTrigger, SelectValue, SelectContent, SelectItem,
  } from "@gpui-native/svelte"
  let language = $state("rust")
  let open = $state(false)
</script>
<Select bind:value={language} bind:open>
  <SelectTrigger><SelectValue placeholder="Language" /></SelectTrigger>
  <SelectContent style={{ padding: 8, background: "#1e293b" }}>
    <SelectItem value="rust" textValue="Rust">Rust</SelectItem>
    <SelectItem value="typescript" textValue="TypeScript">TypeScript</SelectItem>
  </SelectContent>
</Select>
```

Select supports initial value/open defaults, disabled options, arrow/Home/End
navigation, Enter/Space selection, Escape/Tab dismissal, buffered typeahead,
and active-option scrolling. Supply `textValue` for the display label and
searchable text when children contain custom markup. Items stay registered
while content is closed, so selected labels and typeahead remain available.

Combobox adds `bind:inputValue`, `multiple`, filtering, optional `items`,
`itemToStringValue`, and `autoHighlight`. Set `filter={null}` for externally
filtered data. A custom filter receives `(value, query, itemToString)`.
`ComboboxList` accepts an `item(value, index)` snippet for its filtered items,
or ordinary children. Items accept a `style(state)` callback and a
`content(state)` snippet exposing `selected`, `highlighted`, and `disabled`.
These are the Svelte equivalents of render props/scoped slots.

Trigger components accept a `child(hostProps)` snippet to place their behavior
on your own native primitive without an extra wrapper. Spread the supplied
props onto that primitive and preserve its `ref` binding when composing Select
or Combobox triggers. TooltipProvider configures initial and repeat-hover delays;
Tooltip supports bound open state, focus/hover activation, dismissal and timer
cleanup. Groups, labels, separators, Select scroll buttons, Combobox empty state,
and reusable `FloatingLayer` complete the control parts.

## Text search and GPU canvas

`useTextSearch(() => ({ query }))` returns reactive `props`, `total`, `active`,
and navigation methods. Spread `search.props` onto the searchable container.
The matcher, selection and highlight rendering are shared with the other
adapters, including virtual-list offsets. Match-count callbacks aggregate ordinary
text nodes. For rich-text primitives such as Markdown, provide an externally
computed `matches: { total, indexOffset }` when driving find navigation.

`useGPUCanvas(() => ({ width, height, presentation }))` returns a reactive
`current` canvas. Resizing preserves its identity; component destruction releases
its resources. Read `current` in an effect to start a drawing producer and return
that producer's cleanup function. Use `<Canvas source={canvas.current.id} />`
when it exists. See the Svelte WebGPU example and [shared GPU guide](webgpu.md)
for device ownership and platform limitations. Native WebGPU setup remains an
explicit import from `@gpui-native/svelte/webgpu-native`.

## Browser build

Use `@gpui-native/svelte/web`, or alias the main package to that entry in the
browser build. Keep `gpuiSvelte()` enabled and alias `@gpui-native/wasm` to the
generated `web/pkg/gpui_core.js` bridge. The repository's
`vite.pages.config.mts` demonstrates the complete setup. The same native
components render into GPUI's canvas; Svelte never replaces the browser's
`document` or draws a second DOM interface. Static hosting needs no COOP/COEP
headers. Multiple windows map to independent canvases.

## Testing and performance

Import `mountGpui` or `createTestRoot` from `@gpui-native/svelte/testing`.
Compile test fixtures through `gpuiSvelte()` or `compileNative()` from
`@gpui-native/svelte/compiler`; ordinary DOM Svelte compilation is incompatible.

```ts
const root = mountGpui(App)
root.findByTestId("increment").click()
await root.flush()
expect(root.findByText("1")).toBeDefined()
root.unmount()
```

`mountGpui` uses the deterministic memory backend. `createTestRoot` uses GPUI's
GPU test renderer on macOS and Windows. Automation lives in the `/automation`
subpath. Tests cover real compiled runes, keyed identity, async blocks, bindings,
deep prop changes, controls, cleanup, window isolation, error boundaries and GPU
source lifetime. The native smoke test exercises layout, clicks, keyboard
activation, editable text and accessibility through the actual GPUI renderer.

Run `bun run bench:svelte` for the 10,000-row, 1,000-update regression workload.
It asserts exactly one native batch and one text mutation per leaf update.
Reported memory-backend timings include its batch validation/tree copy and are
not native frame times. GPUI still owns layout and drawing costs.

The structural bridge tracks dirty values and changed parents. Text/property
updates do not traverse the complete tree; structural edits inspect the changed
parent's children. Svelte owns its dependency graph, keyed reconciliation and
effect cleanup. GPUI receives one shared `applyBatch` mutation wave, with no
framework dependencies in Rust/core. The build bundles the pinned official
client runtime against a private structural host using build-time imports;
there is no global DOM monkey patch. An upgrade of Svelte requires rebuilding
that runtime and passing compiler, lifecycle, native and browser checks together.
