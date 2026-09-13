# GPUI Native

[![CI](https://github.com/countradooku/gpui-native/actions/workflows/ci.yml/badge.svg)](https://github.com/countradooku/gpui-native/actions/workflows/ci.yml)
[![GitHub Pages](https://github.com/countradooku/gpui-native/actions/workflows/pages.yml/badge.svg)](https://countradooku.github.io/gpui-native/)
[![Vue on npm](https://img.shields.io/npm/v/@gpui-native/vue?label=Vue)](https://www.npmjs.com/package/@gpui-native/vue)
[![React on npm](https://img.shields.io/npm/v/@gpui-native/react?label=React)](https://www.npmjs.com/package/@gpui-native/react)
[![Svelte on npm](https://img.shields.io/npm/v/@gpui-native/svelte?label=Svelte)](https://www.npmjs.com/package/@gpui-native/svelte)

A UI engine for **Vue 3, React 19.2, and Svelte 5**, built on [Zed's GPUI](https://gpui.rs/). Write components using your framework's state and reconciliation model; a shared Rust engine handles layout, input, text, painting, and windows on desktop and in the browser through WebAssembly.

[Live examples](https://countradooku.github.io/gpui-native/) · [Vue quick start](#vue-quick-start) · [React guide](docs/react.md) · [Svelte guide](docs/svelte.md) · [GPUI Kit catalog](docs/gpui-kit.md)

## What you can build with

| Capability          | Shared by Vue, React, and Svelte                                                                                          |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Native UI           | Typed layout, input, images, SVG, focus, keyboard/pointer events, and accessibility props                                 |
| GPUI Kit            | 86 styled native components, including editors, virtualized tables, menus, dialogs, charts, dock panels, and settings     |
| Rich text           | Selectable/searchable text, syntax-highlighted code, Markdown, diff viewers, and virtual lists                            |
| Graphics and motion | Retained canvas drawing, native keyframes and springs, a controllable animation timeline, and WebGPU/Three.js integration |
| Application windows | Independent roots, window controls, size/inset updates, selection, scrolling, and debug overlays                          |
| Testing             | Framework test roots, retained-tree assertions, automation, and GPU layout/input/screenshot tests on supported hosts      |

Each framework has its own adapter: Vue uses a custom renderer, React uses its reconciler, and Svelte uses a native compiler with runes. They share the same mutation protocol and Rust components. See [the architecture](docs/architecture.md).

## GPUI Kit

[GPUI Kit](https://github.com/longbridge/gpui-kit) is integrated into the shared native engine, with 86 typed components for Vue, React and Svelte. Import them from `@gpui-native/<framework>/kit`. Native controls, editors, tables, menus, overlays, charts, docking and settings share the same desktop and browser implementation.

The `/kit` entry points provide styled Kit components; the main package entry points retain the existing primitives and headless controls. Kit inputs support Vue `v-model`, React controlled callbacks, and Svelte `bind:value`.

See [the Kit guide and complete catalog](docs/gpui-kit.md), including API limits, or run the [Kit gallery](examples/kit/src/App.vue). This integration is included in this checkout; the existing `0.2.0` npm release predates the `/kit` entry points.

## Status

GPUI Native is an early `0.2.x` release. Native rendering runs on Apple Silicon
macOS, x64 Linux (Wayland/X11), and x64 Windows, with a WebAssembly renderer
backing the browser examples. CI runs formatting, lint, framework type checks,
clippy with warnings denied, JavaScript and Rust tests, native builds, and
headless addon smoke tests. GPU-backed layout/input/screenshot tests run on
macOS and Windows; Linux's production binding does not expose that test renderer.

| Host                    | Native/browser renderer     | GPU screenshot test renderer |
| ----------------------- | --------------------------- | ---------------------------- |
| macOS, Apple Silicon    | Native GPUI                 | Available                    |
| Linux, x64, Wayland/X11 | Native GPUI with wgpu       | Not exposed                  |
| Windows, x64            | Native GPUI                 | Available                    |
| Browser                 | Single-threaded WebAssembly | Desktop test API not exposed |

The live WebAssembly example gallery deploys to
[countradooku.github.io/gpui-native](https://countradooku.github.io/gpui-native/) on
every push to `main`.

The published npm packages live under the `@gpui-native` org:

| npm package                                                                  | Contents                                                             |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| [`@gpui-native/vue`](https://www.npmjs.com/package/@gpui-native/vue)         | The Vue 3 renderer, components, composables, and testing APIs        |
| [`@gpui-native/react`](https://www.npmjs.com/package/@gpui-native/react)     | React 19.2 reconciliation, JSX, components, hooks, and testing       |
| [`@gpui-native/svelte`](https://www.npmjs.com/package/@gpui-native/svelte)   | Svelte 5 native compiler, runes, components, bindings, and testing   |
| [`@gpui-native/runtime`](https://www.npmjs.com/package/@gpui-native/runtime) | Shared TypeScript batching, events, backend wrappers, and automation |
| [`@gpui-native/core`](https://www.npmjs.com/package/@gpui-native/core)       | Prebuilt N-API binaries for macOS (arm64), Linux x64, Windows x64    |
| [`gpui-vue`](https://www.npmjs.com/package/gpui-vue)                         | Alias of `@gpui-native/vue` kept for the shorter name                |

All five scoped packages and the `gpui-vue` alias
use the same release version. See [the full 0.2.0 release notes](docs/releases/v0.2.0.md).

The [`@gpui-native`](https://www.npmjs.com/org/gpui-native) org is the home
for this runtime going forward: bindings for other frameworks and platforms
(Solid and eventually mobile hosts) will be published there as they
land.

The upstream baseline and subsequent ports are tracked in [the GPUix synchronization record](docs/upstream-sync.md).

## Install

Choose the adapter for your application:

```bash
bun add @gpui-native/vue vue
# or
bun add @gpui-native/react react
# or
bun add @gpui-native/svelte
```

You can use `npm install` instead of `bun add`. All adapters use the shared
`@gpui-native/core` addon, which ships prebuilt N-API binaries
for the three supported targets. On other platforms, build the addon from
source with `bun --filter @gpui-native/core build` (see [Build](#build)).

The engine and framework adapters have separate ownership:

- `crates/gpui-core` — retained tree, native GPUI renderer, window lifecycle, styles, events, text selection, motion, themes, automation, and native elements.
- `packages/vue` — `@vue/runtime-core` renderer, typed Vue components, composables, headless controls, and testing APIs. Published as `@gpui-native/vue` (alias: `gpui-vue`).
- `packages/core` — framework-neutral N-API addon loader, generated ABI types, and prebuilt platform binaries. Published as `@gpui-native/core`.
- `packages/react` — React reconciliation, typed JSX, hooks, headless controls, and testing APIs. See [React support and examples](docs/react.md).
- `packages/svelte` — Svelte 5 runes, native compilation, typed components, headless controls, and testing. See [Svelte support](docs/svelte.md).
- `packages/runtime` — shared TypeScript backend wrappers, native props, batching, events, automation, and GPU testing.

Future adapters consume the shared runtime rather than depending on another framework. See
[the adapter architecture](docs/architecture.md) for the boundary and extension path.

```text
Vue custom renderer    React reconciler    Svelte native compiler
         \                    |                    /
          +---- shared TypeScript runtime --------+
                              ↓ applyBatch
Rust RetainedTree + native element registry
    ↓ GPUI layout, input, text, paint, and events
Desktop GPUI window or single-threaded WebAssembly host
```

## React quick start

```tsx
import { useState } from "react"
import { render, Button, Column, Text } from "@gpui-native/react"

function App() {
  const [count, setCount] = useState(0)
  return (
    <Column style={{ padding: 24, gap: 12 }}>
      <Text>Count: {count}</Text>
      <Button onPress={() => setCount((value) => value + 1)} style={{ padding: 12 }}>
        Increment
      </Button>
    </Column>
  )
}

render(<App />, { title: "React + GPUI", width: 800, height: 600 })
```

Set TypeScript's `jsx` to `react-jsx` and `jsxImportSource` to `@gpui-native/react`.

See [the React guide](docs/react.md) for JSX configuration, native and browser setup,
component parity, testing, and three runnable examples. In a checkout:

```bash
bun run build:adapters
bun --filter @gpui-react/example-counter build
bun --filter @gpui-react/example-counter start
```

## Svelte quick start

```svelte
<!-- App.svelte -->
<script lang="ts">
  import { Button, Column, Text } from "@gpui-native/svelte"
  let count = $state(0)
</script>

<Column style={{ padding: 24, gap: 12 }}>
  <Text>Count: {count}</Text>
  <Button onPress={() => count++} style={{ padding: 12 }}>Increment</Button>
</Column>
```

```ts
// main.ts
import { render } from "@gpui-native/svelte"
import App from "./App.svelte"

render(App, { title: "Svelte + GPUI", width: 800, height: 600 })
```

Compile with `gpuiSvelte` from `@gpui-native/svelte/vite` so components target GPUI.

See [the Svelte guide](docs/svelte.md) for runes, native/browser compiler setup,
bindings, snippets, controls, testing, and four runnable examples. In a checkout:

```bash
bun run build:adapters
bun --filter @gpui-svelte/example-counter build
bun --filter @gpui-svelte/example-counter start
```

## Vue quick start

Write ordinary Vue single-file components — `<template>`, `<script setup>`,
`v-model`, `v-if`/`v-for`, and event handlers all work — and they render into
a native GPUI window instead of the DOM:

```vue
<!-- App.vue -->
<script setup lang="ts">
import { Button, Code, Column, TextInput } from "@gpui-native/vue"
import { ref } from "vue"

const source = ref("const answer = 42")
</script>

<template>
  <Column :style="{ padding: 24, gap: 12 }">
    <TextInput v-model="source" placeholder="Type some TypeScript" />
    <Button :style="{ padding: 10, background: '#334155' }" @press="source = ''">Clear</Button>
    <Code
      :code="source"
      language="typescript"
      :showLineNumbers="true"
      :style="{ padding: 12, background: '#111827', borderRadius: 8 }"
    />
  </Column>
</template>
```

```ts
// main.ts
import { render } from "@gpui-native/vue"

import App from "./App.vue"

render(App, { title: "My Vue app", width: 800, height: 600 })
```

Single-file components compile through the standard `@vitejs/plugin-vue`
pipeline (see `vite.examples.config.mts` for a working config). The Vue
examples under [`examples/`](./examples) use this setup. Render functions
and JSX work equally well — anywhere you would write `h("div", ...)` in a
browser app, the same call renders a native element.

`render()` owns one persistent native window and hot-remounts its Vue root. On Linux and Windows it keeps Node alive while GPUI's threaded window is open; closing the final window releases that handle. `resetRender()` unmounts Vue, destroys the retained tree, and closes the native event loop immediately.

## Components

Use the same component names in Vue, React, and Svelte, on desktop and in the browser.
`View` is a bare GPUI container; `Row` and `Column` add flex layout defaults.
`ScrollView` adds vertical scrolling, or horizontal scrolling with `horizontal`.
Give it a bounded `height` or `width` so content can overflow. All three layout
helpers lower to one native `div`, with your `style` overriding their defaults.

`Button` is unstyled and accepts arbitrary children (text, icons, or layouts).
Use `@press` in Vue or `onPress` in React and Svelte for pointer/accessibility clicks, Enter
key down, and Space key release. Held keys and Ctrl/Alt/Cmd shortcuts do not
activate it; blur or disabling cancels a pending Space press. `disabled` blocks
activation and removes the tab stop. `onClick` / `@click` remains the raw click
callback; other host events, styles, motion and accessibility labels pass through.
The app owns padding, colors, hover/active styles, and disabled appearance. The
button supplies `role="button"`, `tabIndex={0}`, a pointer cursor, and
`userSelect: "none"`; host styles and enabled tab order remain overridable.

```tsx
<Button
  disabled={saving}
  onPress={save}
  style={{ padding: 12, background: "#334155", borderRadius: 8 }}
>
  <Text>Save changes</Text>
</Button>
```

See [component names and migration](docs/components.md) for the complete API.

### Native primitives

| Host tag            | Vue / React / Svelte component | Native implementation                                                                                                 |
| ------------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| `div`, `text`       | `View`, `Text`                 | GPUI layout, selectable text, focus, mouse/keyboard/scroll events, and pseudo-state styles                            |
| `input`, `textarea` | `TextInput`, `TextArea`        | Native editable GPUI text, caret, selection, clipboard, IME, submit/change events, read-only mode, and `v-model`      |
| `img`, `svg`        | `Image`, `Svg`                 | Native image loading, object-fit, fallback content, and tinted SVG data                                               |
| `anchored`          | `Anchored`                     | Deferred anchored layers with side/alignment, offsets, collision switching/snapping, priority, and occlusion          |
| `code`              | `Code`                         | Syntect language detection, cached syntax highlighting, optional line numbers, horizontal scrolling, and selection    |
| `diff`              | `Diff`                         | Unified-diff parsing, word highlights, collapsed files, show-more rows, line events, and default-on virtual scrolling |
| `markdown`          | `Markdown`                     | GFM parsing, headings, lists, tables, tasks, inline code, fenced code highlighting, selection, and link events        |
| `virtual-list`      | `VirtualList`                  | Variable-height GPUI list virtualization, overdraw, alignment, follow-tail, and imperative scrolling                  |
| `canvas`            | `Canvas`                       | Retained paths, lines, polylines, rectangles, and circles, tessellated only when commands change                      |

All host tags remain directly usable with `h("code", props)` or templates/JSX. The named components provide discoverable TypeScript props; input components additionally translate Vue `v-model` to native `value`/`change` semantics.

## Headless controls

All three adapters include headless controls rendered through GPUI host nodes:

- `Select`, `SelectTrigger`, `SelectValue`, `SelectContent`, `SelectItem`, groups, labels, separators, and scroll buttons.
- `Combobox`, `ComboboxInput`, `ComboboxTrigger`, `ComboboxValue`, filtered/scoped lists, items, empty state, groups, labels, and separators.
- `TooltipProvider`, `Tooltip`, `TooltipTrigger`, and `TooltipContent`.
- `FloatingLayer` for reusable anchored content.
- `motion.View` and `MotionView` for native tweens, offset keyframes, physical springs, repeats, and staggered entrances.

In Vue, Select and Combobox support ordinary `v-model`/`modelValue`; named `value`
models remain available for compatibility. Open state uses `v-model:open`.
Select also supports buffered `textValue` typeahead and keeps the active option
in view. React uses controlled props and callbacks; Svelte supports bindings.
See the [React](docs/react.md) and [Svelte](docs/svelte.md) guides for each framework's APIs.

## Shared mutation protocol

Vue's renderer, React commits, and Svelte host operations all feed the shared
runtime. The Vue operation names below illustrate how host mutations reach GPUI:

| Vue renderer operation          | Native behavior                                                                   |
| ------------------------------- | --------------------------------------------------------------------------------- |
| `createElement(tag)`            | Allocates a stable host ID and creates the matching retained Rust element/factory |
| `createText(value)`             | Creates a native selectable `text` node                                           |
| `createComment(value)`          | Keeps a JS-only structural anchor; it never enters native layout                  |
| `insert(child, parent, anchor)` | Reparents or reorders a node while preserving keyed identity                      |
| `remove(child)`                 | Detaches the node, clears Vue handlers, and recursively destroys native state     |
| `setText` / `setElementText`    | Updates text or replaces an element's children                                    |
| `patchProp(style)`              | Normalizes Vue style arrays and lowers `StyleDesc` through GPUI/Taffy             |
| `patchProp(onX)`                | Registers the Vue callback and toggles the native event listener                  |
| `patchProp(custom)`             | Sends typed native-element props to the Rust factory registry                     |
| `parentNode` / `nextSibling`    | Serves Vue keyed diffing, fragments, and component moves from local host links    |

Framework mutation waves are coalesced into `applyBatch()` calls. Rust decodes
the tuples into typed operations before touching the tree, so a malformed batch
applies nothing. Equal style payloads share one retained allocation and unused
styles are swept after commits. The tree is updated under one lock and GPUI is
invalidated once per commit.
Frames take a structurally shared immutable snapshot and release that lock
before GPUI layout, input wiring, and paint construction.

## Window APIs, hooks, and runes

All adapters expose window, timeline, audio-buffer and text-search controls.
Vue uses composables, React uses hooks, and Svelte exposes reactive values through
its rune-aware runtime. The following names describe the Vue APIs; the
[React API mapping](docs/react.md#vue-capability-mapping) and
[Svelte guide](docs/svelte.md) explain the corresponding return values and lifecycle.

- `useGpuiRequired()` returns the current renderer.
- `useGpuiWindow()` exposes size/insets, title, focus, blur, selection, highlights, scrolling, and debug-overlay controls.
- `useElementRef()` returns a typed template ref for native elements.
- `useWindowSize()` follows native resize events; custom renderers without that capability use a configurable polling fallback.
- `useWindowInsets()` follows the same event path, reads safe-area/software-keyboard geometry, and derives the visible content height.
- `useTextSearch()` drives a find cursor through the native `highlight` prop; `findRanges()` uses the same Unicode matcher for virtualized rows.
- `useGpuiTimeline()` controls the live native animation clock: play, pause, seek, and playback rate.
- `useGpuiAudioFrames()` feeds bounded, interleaved decoded `Float32Array` PCM chunks to the native runtime.

## Native drawing and playback

Canvas commands are retained and tessellated in Rust. The framework sends geometry only
when the `commands` prop changes; paint frames do not execute JavaScript:

```ts
h(Canvas, {
  style: { width: 640, height: 240 },
  commands: [
    { type: "rect", x: 0, y: 0, width: 640, height: 240, fill: "#10141c" },
    {
      type: "path",
      operations: [
        { op: "moveTo", x: 0, y: 120 },
        { op: "bezierTo", cp1x: 160, cp1y: 0, cp2x: 480, cp2y: 240, x: 640, y: 120 },
      ],
      stroke: "#62e6a7",
      strokeWidth: 3,
    },
  ],
})
```

Motion is evaluated on GPUI animation frames and shares a controllable window
timeline:

```ts
h(MotionView, {
  initial: { opacity: 0, left: -24 },
  animate: [
    { at: 0, value: { opacity: 0, left: -24 } },
    { at: 1, value: { opacity: 1, left: 0 } },
  ],
  transition: stagger(rowIndex, 0.04, {
    type: "spring",
    stiffness: 190,
    damping: 24,
  }),
})

const timeline = useGpuiTimeline()
timeline.pause()
timeline.seek(750)
timeline.setPlaybackRate(2)
timeline.play()
```

Decoded audio can be transferred as chunked interleaved f32 PCM without JSON
or per-sample calls. The bounded queue is intended for a native audio backend
or custom host and drops oldest frames on overflow to preserve low latency:

```ts
const audio = useGpuiAudioFrames()
audio.configure(48_000, 2, 48_000) // one second of stereo capacity
audio.enqueue(decodedInterleavedFloat32Chunk)
```

Use `createWindow()` for independently mounted roots. Each returned window has
its own framework root, retained tree, event scope, timeline, and lifecycle; `close()`
does not affect other windows. `render()` remains the persistent single-window
hot-remount API.

## Testing and automation

Each adapter provides framework testing APIs over the shared retained runtime.
For example, Vue's `mountGpui()` runs its renderer against a deterministic
in-memory retained tree:

```ts
const app = mountGpui(MyComponent)
app.findByTestId("save").click()
await app.flush()
expect(app.findByText("Saved")).toBeDefined()
app.unmount()
```

For native automation, `GpuiAutomation` exposes normalized tree snapshots, test-id/type lookup, bounds, native clicking and mouse input, deterministic clock control, painted text, and screenshots. Memory, native, and SSE automation all expose the same nested tree schema; `snapshotRenderer()` also accepts the older indexed shape.

`createTestRoot({ width, height })` sizes the GPU-backed offscreen window for
layout and wrapping tests; its native default remains 1280×800.

The shared `@gpui-native/runtime/automation` entry provides `App`, `Locator`, `connectTest()`, `connectStdio()`, `launch()`, the typed versioned request catalog, SSE codecs, and `enableAutomation()` for serving a live renderer over stdin/stdout. The Vue automation entry and `gpui-vue/automation` alias remain available. macOS and Windows test-support builds include GPUI's GPU-backed `TestGpuiRenderer`; check `hasNativeTestRenderer` before using GPU-backed test roots. Linux still runs addon load and headless retained-tree checks.

Locators support clicks, auxiliary buttons, hover, wheel input, text entry, held
modifier keys, and stepped drag gestures. Live desktop and WebAssembly renderers
share the same keyboard, pointer, scroll, focus, and deterministic-clock hooks.

## Text search and virtual lists

Declare `highlight` on any container to search its retained text subtree. The
native renderer reports `matchCount`, paints the active result separately, and
exposes the last frame through `useGpuiWindow().paintedHighlights()`:

```ts
const query = ref("")
const search = useTextSearch({ query })

return () =>
  h("div", search.props.value, [
    h(
      "text",
      null,
      search.total.value === 0 ? "No matches" : `${search.active.value + 1}/${search.total.value}`,
    ),
    h("div", null, documentText),
  ])
```

For a windowed `virtual-list`, supply `itemCount`, `estimatedItemHeight`, and
the mounted data's `windowStart`. `onVisibleRange` reports the visible index
interval. Scroll anchoring is index-based: when rows are prepended, update
`windowStart` with the data window so the same logical row stays pinned. If
searching rows that are not mounted, sum `findRanges()` above the window and
pass `{ total, indexOffset }` to `useTextSearch()`.

## Build

Install Bun 1.4 and Rust through rustup. The checked-in
`rust-toolchain.toml` installs the pinned compiler, `rust-src`, and
`wasm32-unknown-unknown`; `rust-src` is required because the browser build uses
Cargo's unstable `build-std` path. `scripts/build-pages.mts` scopes
`RUSTC_BOOTSTRAP=1` to that Wasm compilation. Also install the
`wasm-bindgen-cli` version recorded in `Cargo.lock` when building Pages locally.

```bash
bun install
bun run build
bun run build:binaries
bun run build:pages
bun run test
bun run check
bun run bench
bun --filter @gpui-vue/example-canvas start
```

`bun run build` produces the full native addon, all three framework adapters, and every example. Oxc
transforms the package TypeScript and generates source maps, while Vite 8's Oxc pipeline
transforms and minifies the Vue examples. Oxlint and Oxfmt are enforced by `bun run check`;
TypeScript and `vue-tsc` remain enabled for declaration generation and strict type checking.
`bun run build:binaries` additionally embeds each example and its host-platform N-API addon
into a standalone executable under that example's `dist/` directory.

`bun run build:pages` compiles the retained Rust renderer against GPUI's single-threaded browser
platform, generates its `wasm-bindgen` browser bridge, and creates the example gallery under
`dist-pages/`. The deployed gallery is available at
[countradooku.github.io/gpui-native](https://countradooku.github.io/gpui-native/).
The Pages workflow supplies the deployment base path from GitHub's Pages configuration.
For a local build with the same URL prefix, run `PAGES_BASE_PATH=/gpui-native/ bun run build:pages`.

Browser applications use their adapter's `/web` entry and must alias the virtual
`@gpui-native/wasm` module to their generated `wasm-bindgen` JavaScript file. For
Vite, the essential configuration is:

```ts
resolve: {
  alias: {
    "@gpui-native/wasm": resolve(projectRoot, "web/pkg/gpui_core.js"),
  },
}
```

Native builds use Syntect's Oniguruma engine; WebAssembly uses its pure-Rust
fancy-regex engine. Both targets therefore retain the same syntax definitions
and visual highlighting. Multiple native windows are represented by independent
canvases in the browser example.

CI runs formatting, clippy with warnings denied, Oxc, TypeScript, Vue and Svelte component checks, and Rust
checks plus one native target per OS:
Apple Silicon macOS, x64 Linux, and x64 Windows. Pushing a version tag such as
`v0.2.0` creates a GitHub Release containing those bindings and publishes
all five `@gpui-native` packages and the `gpui-vue` alias to npm after each platform passes its
headless native smoke test and the generated declaration file is checked for
drift. Publishing requires an `NPM_TOKEN` repository secret with publish
access to every release package.
See [`examples/README.md`](./examples/README.md) for runnable canvas,
chat, motion/timeline, multi-window, audio-buffer, and counter demos. Pass
`{ headless: true }` to `renderer.init()` when a retained tree is needed
without opening a window; headless is a runtime mode of the same parity binary.

## Performance regression benchmarks

`bun run bench` exercises the production JavaScript host links, batching and
audio queue plus Rust's release-mode `applyBatch` retained-tree path. It reports
warm-JIT medians and fails when conservative throughput floors or the linked
sibling speedup are missed. The suite compares current constant-time sibling
lookup with the former array-scan model, verifies a text-only Vue patch is one
mutation, and times 10,000-operation Rust text and resolved-style batches. This
keeps the performance claims reproducible without making CI depend on a single
machine's absolute timing.

The native engine pins the Zed/GPUI revision whose embedded-window, selectable-text, screenshot, and automation APIs it uses. On Linux, Fontconfig can be loaded dynamically; normal Wayland/XCB/XKB development packages are still recommended. The build script also handles distributions that install compatible runtime XCB/XKB libraries without unversioned linker symlinks.

## Attribution

The native component engine is adapted from GPUix's Apache-2.0 native package. Some upstream native subsystems derive from MIT-licensed Comet. Source details and license texts are recorded in [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md), [`LICENSES/Apache-2.0.txt`](./LICENSES/Apache-2.0.txt), and [`LICENSES/Comet-MIT.txt`](./LICENSES/Comet-MIT.txt). GPUI Native provides the framework adapters and shared host integration. GPUI Kit is vendored under Apache-2.0 with its source pin, licenses, and embedding patches recorded in [`vendor/gpui-kit/UPSTREAM.md`](vendor/gpui-kit/UPSTREAM.md).

### WebGPU canvas

React, Vue, and Svelte can display real WebGPU rendering through a shared canvas source, with
WGSL, compute, texture uploads, cube maps, MSAA, and Three.js coverage. See the
[WebGPU guide](docs/webgpu.md) for runnable examples, lifecycle rules, measured
transport benchmarks, Bun/Node support, and the tested platform matrix. Direct
presentation currently requires Metal or GPUI’s shared wgpu device; Windows
requires explicit readback. This integration is not yet production-qualified on
all platforms.
