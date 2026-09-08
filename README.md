# gpui-vue

[![CI](https://github.com/countradooku/gpui-vue/actions/workflows/ci.yml/badge.svg)](https://github.com/countradooku/gpui-vue/actions/workflows/ci.yml)
[![GitHub Pages](https://github.com/countradooku/gpui-vue/actions/workflows/pages.yml/badge.svg)](https://countradooku.github.io/gpui-vue/)
[![npm](https://img.shields.io/npm/v/gpui-vue)](https://www.npmjs.com/package/gpui-vue)

Vue 3 bindings for [Zed's GPUI](https://gpui.rs/), implemented as a Vue custom renderer and a full native Rust component runtime.

## Status

gpui-vue is an early `0.1.x` release. Native rendering runs on Apple Silicon
macOS, x64 Linux (Wayland/X11), and x64 Windows, with a WebAssembly renderer
backing the browser examples. Every commit passes the full quality gate —
formatting, Oxlint, TypeScript, `vue-tsc`, clippy with warnings denied, and the
JavaScript and Rust test suites — plus a native build and headless smoke test
on all three platforms. The live WebAssembly example gallery deploys to
[countradooku.github.io/gpui-vue](https://countradooku.github.io/gpui-vue/) on
every push to `main`.

## Install

```bash
bun add gpui-vue   # or: npm install gpui-vue
```

`gpui-vue` depends on `@gpui-native/core`, which ships prebuilt N-API binaries
for the three supported targets. On other platforms, build the addon from
source with `bun --filter @gpui-native/core build` (see [Build](#build)).

This is a Vue-only monorepo. It has no React or `react-reconciler` dependency:

- `crates/gpui-vue-core` — retained tree, native GPUI renderer, window lifecycle, styles, events, text selection, motion, themes, automation, and native elements.
- `packages/gpui-vue` — `@vue/runtime-core` renderer, typed Vue components, composables, headless controls, and testing APIs.
- `packages/gpui-vue-native` — N-API addon loader and platform package metadata.

```text
Vue components
    ↓ @vue/runtime-core custom renderer
NodeOps + patchProp
    ↓ one batched mutation protocol
Rust RetainedTree + native element registry
    ↓ GPUI layout, input, text, paint, and events
Native GPUI window
```

## Quick start

```ts
import { GpuiCode, GpuiInput, defineComponent, h, ref, render } from "gpui-vue"

const App = defineComponent({
  setup() {
    const source = ref("const answer = 42")
    return () =>
      h("div", { style: { padding: 24, gap: 12 } }, [
        h(GpuiInput, {
          modelValue: source.value,
          "onUpdate:modelValue": (value: string) => {
            source.value = value
          },
        }),
        h(GpuiCode, {
          code: source.value,
          language: "typescript",
          showLineNumbers: true,
          style: { padding: 12, background: "#111827", borderRadius: 8 },
        }),
      ])
  },
})

render(App, { title: "My Vue app", width: 800, height: 600 })
```

`render()` owns one persistent native window and hot-remounts its Vue root. On Linux and Windows it keeps Node alive while GPUI's threaded window is open; closing the final window releases that handle. `resetRender()` unmounts Vue, destroys the retained tree, and closes the native event loop immediately.

## Native host elements

| Host tag            | Vue component                | Native implementation                                                                                                 |
| ------------------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `div`, `text`       | `GpuiDiv`, `GpuiTextElement` | GPUI layout, selectable text, focus, mouse/keyboard/scroll events, and pseudo-state styles                            |
| `input`, `textarea` | `GpuiInput`, `GpuiTextarea`  | Native editable GPUI text, caret, selection, clipboard, IME, submit/change events, read-only mode, and `v-model`      |
| `img`, `svg`        | `GpuiImage`, `GpuiSvg`       | Native image loading, object-fit, fallback content, and tinted SVG data                                               |
| `anchored`          | `GpuiAnchored`               | Deferred anchored layers with side/alignment, offsets, collision switching/snapping, priority, and occlusion          |
| `code`              | `GpuiCode`                   | Syntect language detection, cached syntax highlighting, optional line numbers, horizontal scrolling, and selection    |
| `diff`              | `GpuiDiff`                   | Unified-diff parsing, word highlights, collapsed files, show-more rows, line events, and default-on virtual scrolling |
| `markdown`          | `GpuiMarkdown`               | GFM parsing, headings, lists, tables, tasks, inline code, fenced code highlighting, selection, and link events        |
| `virtual-list`      | `GpuiVirtualList`            | Variable-height GPUI list virtualization, overdraw, alignment, follow-tail, and imperative scrolling                  |
| `canvas`            | `GpuiCanvas`                 | Retained paths, lines, polylines, rectangles, and circles, tessellated only when commands change                      |

All host tags remain directly usable with `h("code", props)` or templates/JSX. The named components provide discoverable TypeScript props; input components additionally translate Vue `v-model` to native `value`/`change` semantics.

## Higher-level Vue controls

The Vue package includes headless, shadcn-shaped controls rendered entirely through GPUI host nodes:

- `Select`, `SelectTrigger`, `SelectValue`, `SelectContent`, `SelectItem`, groups, labels, separators, and scroll buttons.
- `Combobox`, `ComboboxInput`, `ComboboxTrigger`, `ComboboxValue`, filtered/scoped lists, items, empty state, groups, labels, and separators.
- `TooltipProvider`, `Tooltip`, `TooltipTrigger`, and `TooltipContent`.
- `FloatingLayer` for reusable anchored content.
- `motion.div` and `MotionDiv` for native tweens, offset keyframes, physical springs, repeats, and staggered entrances.

Select and Combobox support ordinary `v-model`/`modelValue`; named `value`
models remain available for compatibility. Open state uses `v-model:open`.
Select also supports buffered `textValue` typeahead and keeps the active option
in view.

## NodeOps mapping

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

Vue patch waves are coalesced into one `applyBatch()` N-API call. Rust decodes
the tuples into typed operations before touching the tree, so a malformed batch
applies nothing. Equal style payloads share one retained allocation and unused
styles are swept after commits. The tree is updated under one lock and GPUI is
invalidated once per commit.
Frames take a structurally shared immutable snapshot and release that lock
before GPUI layout, input wiring, and paint construction.

## Composables

- `useGpuiRequired()` returns the current renderer.
- `useGpuiWindow()` exposes size/insets, title, focus, blur, selection, highlights, scrolling, and debug-overlay controls.
- `useElementRef()` returns a typed template ref for native elements.
- `useWindowSize()` follows native resize events; custom renderers without that capability use a configurable polling fallback.
- `useWindowInsets()` follows the same event path, reads safe-area/software-keyboard geometry, and derives the visible content height.
- `useTextSearch()` drives a find cursor through the native `highlight` prop; `findRanges()` uses the same Unicode matcher for virtualized rows.
- `useGpuiTimeline()` controls the live native animation clock: play, pause, seek, and playback rate.
- `useGpuiAudioFrames()` feeds bounded, interleaved decoded `Float32Array` PCM chunks to the native runtime.

## Native drawing and playback

Canvas commands are retained and tessellated in Rust. Vue sends geometry only
when the `commands` prop changes; paint frames do not execute JavaScript:

```ts
h(GpuiCanvas, {
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
h(MotionDiv, {
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
its own Vue app, retained tree, event scope, timeline, and lifecycle; `close()`
does not affect other windows. `render()` remains the persistent single-window
hot-remount API.

## Testing and automation

`mountGpui()` runs the exact Vue renderer against a deterministic in-memory retained tree:

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

The `gpui-vue/automation` export also provides the parity Playwright-style API: `App`, `Locator`, `connectTest()`, `connectStdio()`, `launch()`, the typed versioned request catalog, SSE codecs, and `enableAutomation()` for serving a live renderer over stdin/stdout. macOS and Windows test-support builds include GPUI's GPU-backed `TestGpuiRenderer`; use `hasNativeTestRenderer` and `createTestRoot()` for real layout, hit-testing, keyboard input, selection, scrolling, painted-text, clock, and screenshot tests.

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

`bun run build` produces the full native addon, the Vue package, and every example. Oxc
transforms the package TypeScript and generates source maps, while Vite 8's Oxc pipeline
transforms and minifies the Vue examples. Oxlint and Oxfmt are enforced by `bun run check`;
TypeScript and `vue-tsc` remain enabled for declaration generation and strict type checking.
`bun run build:binaries` additionally embeds each example and its host-platform N-API addon
into a standalone executable under that example's `dist/` directory.

`bun run build:pages` compiles the retained Rust renderer against GPUI's single-threaded browser
platform, generates its `wasm-bindgen` browser bridge, and creates the example gallery under
`dist-pages/`. The deployed gallery is available at
[countradooku.github.io/gpui-vue](https://countradooku.github.io/gpui-vue/).

Applications importing `gpui-vue/web` must alias the virtual
`@gpui-native/wasm` module to their generated `wasm-bindgen` JavaScript file. For
Vite, the essential configuration is:

```ts
resolve: {
  alias: {
    "@gpui-native/wasm": resolve(projectRoot, "web/pkg/gpui_vue_core.js"),
  },
}
```

Native builds use Syntect's Oniguruma engine; WebAssembly uses its pure-Rust
fancy-regex engine. Both targets therefore retain the same syntax definitions
and visual highlighting. Multiple native windows are represented by independent
canvases in the browser example.

CI runs formatting, clippy with warnings denied, Oxc, Vue, TypeScript, and Rust
checks plus one native target per OS:
Apple Silicon macOS, x64 Linux, and x64 Windows. Pushing a version tag such as
`v0.1.0` creates a GitHub Release containing those bindings and publishes
`gpui-vue` and `@gpui-native/core` to npm after each platform passes its
headless native smoke test and the generated declaration file is checked for
drift. Publishing requires an `NPM_TOKEN` repository secret with publish
access to both packages.
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

The full native component engine is adapted from GPUix's Apache-2.0 native package. Some upstream native subsystems derive from MIT-licensed Comet. Source details and license texts are recorded in [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md), [`LICENSES/Apache-2.0.txt`](./LICENSES/Apache-2.0.txt), and [`LICENSES/Comet-MIT.txt`](./LICENSES/Comet-MIT.txt). The Vue renderer, component API, state model, composables, automation client/protocol implementation, and examples are gpui-vue code.
