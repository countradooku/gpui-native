# gpui-vue examples

Each example is a small native Vue application written as Vue SFCs with
`<template>` and `<script setup lang="ts">`. They use no browser DOM and no
JavaScript render loop.

| Example            | SFC                                                  | Demonstrates                                                        | Run                                                     |
| ------------------ | ---------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------- |
| `market-stream`    | `market-stream/src/App.vue`                          | 100,000-row windowed grid, 20K updates/s, retained live charts      | `bun --filter @gpui-vue/example-market-stream start`    |
| `chat`             | `chat/src/App.vue`                                   | Waku-style shell, 5,000-row transcript, native rich text, selectors | `bun --filter @gpui-vue/example-chat start`             |
| `counter`          | `counter/src/App.vue`                                | Vue reactivity, native input/code, and a motion entrance            | `bun --filter @gpui-vue/example-counter start`          |
| `canvas`           | `canvas/src/App.vue`                                 | Retained GPU paths, pointer events, and a 4,000-point stress case   | `bun --filter @gpui-vue/example-canvas start`           |
| `motion-timeline`  | `motion-timeline/src/App.vue`                        | Keyframes, springs, playback controls, and pointer-captured clips   | `bun --filter @gpui-vue/example-motion-timeline start`  |
| `multiple-windows` | `multiple-windows/src/{Controller,Inspector}App.vue` | Independent native Vue roots sharing reactive state                 | `bun --filter @gpui-vue/example-multiple-windows start` |
| `audio-buffer`     | `audio-buffer/src/App.vue`                           | Chunked stereo f32 ingestion, dequeue, and bounded-buffer overflow  | `bun --filter @gpui-vue/example-audio-buffer start`     |

Build the native addon, package, and every example from the repository root:

```bash
bun run build
bun run build:binaries
```

Build the WebAssembly browser gallery for local static hosting:

```bash
bun run setup:wasm
bun run build:pages
```

The checked-in Rust toolchain installs `rust-src` and
`wasm32-unknown-unknown`. The Pages script scopes `RUSTC_BOOTSTRAP=1` to Cargo's
Wasm-only `build-std` invocation.

GitHub Pages publishes the same artifact at
[countradooku.github.io/gpui-native](https://countradooku.github.io/gpui-native/). The web gallery uses
GPUI's single-threaded browser platform, so it does not depend on cross-origin-isolation headers.

Each example build runs strict `vue-tsc` template checking and then compiles a
small Node-compatible ESM bundle through Vite 8's Rolldown/Oxc pipeline and runs
it with Bun. Vue generates VNodes, while `render()` or `createWindow()` mounts
them through the GPUI custom renderer.

`bun run build:binaries` produces one self-contained executable per example in
its `dist/` directory. These executables embed Bun and the current platform's
N-API addon, so cross-platform releases must be built on or targeted with a
matching native-addon build for each operating system and architecture.

The audio example demonstrates the high-throughput decoded-frame bridge and
its low-latency overflow behavior. Actual device playback remains the
responsibility of the embedding native audio backend.

## React examples

See [the React guide](../docs/react.md). `react-counter`, `react-showcase`, and
`react-multiple-windows` each support `build`, `start`, and `build:binary`.
They also run in the Pages gallery with the shared GPUI Web backend.

## Svelte examples

See [the Svelte guide](../docs/svelte.md). These use the native Svelte compiler
and Svelte 5 runes, with strict `svelte-check` validation:

| Example                   | Demonstrates                                                                  |
| ------------------------- | ----------------------------------------------------------------------------- |
| `svelte-counter`          | Rune state/derived values, bound native editor and Button                     |
| `svelte-showcase`         | Select, Combobox, Tooltip, motion, rich text, search, canvas and virtual rows |
| `svelte-multiple-windows` | Independent component roots and window cleanup                                |
| `svelte-webgpu`           | Reactive GPU-canvas ownership and a shared WebGPU scene                       |

Run `bun --filter @gpui-svelte/example-counter build` followed by
`bun --filter @gpui-svelte/example-counter start`. Substitute any name above.
All four also support `build:binary` and the Pages browser gallery.
