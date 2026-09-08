# GPUix synchronization record

Reviewed on 2026-09-08 against GPUix `main` at
[`6b4be86952aa89cfe61bb573740aea33fef5c5c4`](https://github.com/remorses/gpuix/commit/6b4be86952aa89cfe61bb573740aea33fef5c5c4).
This is a review checkpoint, not a claim that this repository is an exact copy
of that commit.

## Where this repository started

The original imported source is recorded in `THIRD_PARTY_NOTICES.md` as
`b8bbff4ef0dc00dc24c3d3f3ca47818e1be15c69` (2026-08-23). Local commit
`ffbc88a5a1f8ca0776850f6ae1c7c36750419153` subsequently incorporated selected
upstream behavior and refactored the engine. It did not record a new GPUix
checkpoint. Before this review, the GPUI dependency was pinned to
`8b94defe56992b3ca4ffd4853ace741d8168111a`.

Consequently, commit dates alone cannot determine which fixes are present.
This review compared the implementations and commit diffs after the original
import, including the native engine and framework-facing lifecycle changes.

## Changes incorporated in this synchronization

| GPUix commits        | Change and local adaptation                                                                                                                                                                                                                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fb75c1c`, `1cd46cd` | Selection survives virtualized anchor removal; frame-owned drag listeners, edge autoscroll with end detection, soft-wrap wash geometry, input word selection and drag scrolling, and coalesced undo. Preserve local Unicode word boundaries and both entry and byte limits on undo/redo history.                                |
| `09e0cae`            | Structured two-stop linear gradients with sRGB/Oklab interpolation. Preserve the local parsed-style cache and share background resolution with occlusion and anchored fills.                                                                                                                                                    |
| `aacb070`            | Set Windows DPI awareness on the GPUI UI thread before native windows are created. Keep the existing event-driven window-close lifecycle instead of adopting upstream's polling loop.                                                                                                                                           |
| `b20e98a`, `0bb3c94` | Retain decoded raster data URLs and HTTP(S) URI sources; install GPUI's HTTP client on desktop; preserve definite image dimensions while loading. Keep the local SVG adapter and its decoding behavior.                                                                                                                         |
| `10e1bb0`, `3bb1ac6` | Remove process-wide Tab traversal; expose `focusNext`, `focusPrevious`, and renderer-level key callbacks with per-mount event identities on native and WebAssembly. Browser Tab still keeps keyboard focus inside GPUI.                                                                                                         |
| `1c4c67b`            | Persist renderer ownership, event registries, and monotonically increasing node IDs across module reloads. Keep Vue reconciliation and state machinery. Reject stale queued events after a remount.                                                                                                                             |
| `e948b20`            | Dispatch live mouse automation without leasing the root view on macOS, threaded hosts, and WebAssembly.                                                                                                                                                                                                                         |
| `bf98e07`            | Deliver primary clicks from mouse-up on host and custom surfaces; preserve mouse-up-before-click order and register the matching accessibility action.                                                                                                                                                                          |
| `9b1def2`, `5700c96` | Pick up nonblocking AppKit pumping through the GPUI revision update; retain the existing JavaScript frame loop.                                                                                                                                                                                                                 |
| `2487521`, `5f066f3` | Keep pumping after frame exceptions, contain native/browser event-handler exceptions, and display a per-window Vue error stack. Component errors use Vue's error boundary. Global process exception/rejection interception is intentionally not installed: applications retain ownership of their process-level failure policy. |
| `95757bb`            | Underline, line-through, and explicit removal of text decoration through the shared style/painter.                                                                                                                                                                                                                              |
| `ff3c1a2`            | Plain textarea Enter inserts a newline; an active submit listener makes Enter submit. Shift+Enter stays a newline. Listener changes update the key context.                                                                                                                                                                     |
| `4bea250`, `fa61d80` | Font-sized caret and matching IME bounds; vertically centered single-line inputs and clipping to rounded corners.                                                                                                                                                                                                               |
| `8138d4c`, `9e0db51` | Native accessibility roles and ARIA properties, text/input/image defaults, accessible click actions, and coverage for virtual lists, anchored surfaces, and empty images. Test API exposes the actual painted accessibility tree.                                                                                               |

The GPUI dependency now matches GPUix's reviewed submodule revision:
`1f9d1cd88656cf1759b0bdad32fa3e2df3c4b0b9`. This supplies the accessibility hooks
for list/image/SVG elements and activation in GPU-backed tests.

## Already present or intentionally different

- Atomic validated batches, content-interned styles and reclamation, stable
  native identities, removed text-node cleanup, one live Vue root per renderer,
  custom-element event wiring/reset, and shared text selection/highlight paths
  were already present. Do not replace the local split renderer with GPUix's
  monolithic renderer to reproduce these changes.
- The existing port already has browser rendering, pointer capture, native
  clipboard/input plumbing, safe-area insets, debug overlays, variable-height
  list estimates/anchors, virtualized highlight offsets, native automation,
  Syntect's native/Wasm backends, bare code surfaces, and theme-driven metrics.
- Window lifecycle and resize notifications are event-driven locally. GPUix's
  polling fixes are covered by the existing lifecycle implementation and tests.
- `5937978` exports a throwing Linux test-renderer stub upstream. Locally the
  GPU test renderer remains explicitly available only on macOS/Windows, and
  `hasNativeTestRenderer` checks the export. No throwing stub is needed.
- React reconciliation, hooks, Fast Refresh, React-specific test helpers,
  starter templates, app examples, release/publish metadata, screenshots, and
  website tooling are not copied into the Vue adapter.
- `e082247`, `2d6e599`, and `6b4be86` describe future iOS/Hermes packaging. They
  are plans/documentation, not a completed mobile renderer to port. This
  repository does not claim iOS or Hermes support.
- Dependency-only lockfile bumps and contributor preferences (such as leaving
  development windows running) are not renderer fixes.

## Application-facing changes

Window key handlers belong to each `render(App, options)` or `createWindow`
call. Applications that want Tab traversal must opt in; the runtime no longer
installs process-wide Tab bindings:

```ts
const root = render(App, {
  onKeyDown(event) {
    if (event.key !== "tab") return
    if (event.modifiers?.shift) root.renderer.focusPrevious?.()
    else root.renderer.focusNext?.()
  },
})
```

Use `role` and kebab-case `aria-*` host props for accessible names, descriptions,
values, selection and expansion. Images accept `alt`. Text and editable inputs
receive native default roles. Underline and strike-through use
`style.textDecoration` (`"underline"`, `"line-through"`, or `"none"`).

`style.background` still accepts a color string. A gradient uses
`{ type: "linear-gradient", angle: 90, stops: [{ color: "red", position: 0 },
{ color: "blue", position: 1 }], colorSpace: "srgb" }`; `"oklab"` is also
supported. Exactly two stops are required.

Textarea Enter now inserts a newline unless an `onSubmit` listener is present.
Shift+Enter always inserts a newline. Callback and component exceptions display
an error stack in the affected window; a remount resets that display.

## Verification and the next review

Rust tests cover selection continuation, wrap geometry, input key contexts,
undo coalescing/budgets, URI parsing, gradients, and accessibility role mapping.
Vue tests cover prop forwarding/removal, stale events, key-listener lifetime,
frame-error recovery, and the error display. The native smoke command includes
GPU-backed accessibility, click, textarea, and pixel-change checks on supported
platforms. WebAssembly must build with the same APIs and retain the
single-threaded Pages deployment.

Validated locally on macOS: the full `bun run check`, 39 JavaScript tests,
244 Rust tests, native binding generation, native load/close/reopen and GPU
smoke tests, and the Pages/WebAssembly build pass. Built HTML asset references
resolve under `/gpui-native/`. A separate Bun module-reload check confirms
shared event registrations survive reload and removed registrations stay removed.
Windows and Linux builds are delegated to the PR's CI matrix.

For the next review, compare GPUix commits after the checkpoint above, then
check code before porting. Update this document with the reviewed full SHA,
the GPUI pin, incorporated changes, intentional differences, and validation
results. Keep the original provenance in `THIRD_PARTY_NOTICES.md`.
