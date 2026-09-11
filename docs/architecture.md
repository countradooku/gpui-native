# Framework adapter architecture

GPUI Native is the shared engine. Vue 3, React 19.2, and Svelte 5 have independent adapters using the same framework-neutral TypeScript runtime.

## Ownership

| Location                              | Responsibility                                                                                                                          |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/gpui-core`                    | Retained tree, batch validation, layout, rendering, native components, text, input, focus, scrolling, windows, and native/Wasm bindings |
| `packages/core` (`@gpui-native/core`) | Native addon loading, generated ABI types, and platform binaries; no framework runtime dependency                                       |
| `packages/vue` (`@gpui-native/vue`)   | Vue reconciliation, components, refs, composables, and Vue-facing renderer helpers                                                      |
| `packages/runtime`                    | Shared backend wrappers, props, batching, events, automation and GPU test renderer                                                      |
| `packages/react`                      | React reconciliation, JSX, hooks, components and testing                                                                                |
| `packages/svelte`                     | Svelte runes, native compiler, isolated structural host, components and testing                                                         |
| `examples`                            | Vue, React and Svelte examples                                                                                                          |
| `web`                                 | Browser example gallery and generated Wasm bridge                                                                                       |

The dependency direction is framework adapter → core → GPUI. The core must
never import a framework or an adapter. The `gpui-vue` npm alias remains a
compatibility name for the Vue adapter, not the engine.

## Adapter contract

Each adapter translates its framework's host operations into the shared
`applyBatch` mutation protocol. Keep values structured until the outer JSON
serialization. Rust validates the complete batch before mutating the retained
tree. Events return through the engine's event API and the adapter dispatches
them into its framework. Component state and scheduling belong to the framework;
layout, painting, selection, focus, scrolling, and native component behavior
belong to GPUI.

Native consumers use `GpuiRenderer` from `@gpui-native/core`. Browser consumers
use `WebGpuiRenderer` from the bridge generated from the same Rust crate. The
Wasm artifact is `gpui_core.wasm`; generated browser bindings use the
`gpui_core` name. Browser hosting remains single-threaded and must not require
COOP/COEP headers.

`packages/runtime` contains the framework-neutral TypeScript contract, backend
wrappers, batching, events, automation, text matching and GPU test renderer.
Vue re-exports moved APIs for compatibility; its VNodes, refs and composables
remain in `packages/vue`. React owns its reconciler and hooks in `packages/react`.
All adapters claim exclusive ownership of a native renderer and share its ID
allocator. React materializes only committed trees, so speculative work cannot
leak native elements or event handlers.

## Adding an adapter

1. Add `packages/<framework>` and publish it as `@gpui-native/<framework>`.
2. Implement the framework's host integration against the same mutation and
   event contract. React reconciliation and Svelte compilation/runtime
   integration belong inside their respective adapters.
3. Reuse the native component and text pipelines. Translate framework-facing
   props and state without implementing a second renderer.
4. Verify mount, update, removal, event delivery, disposal, and native/browser
   behavior with the adapter's own lifecycle semantics.

`bun run test:native` exercises the core directly without loading Vue. Vue's
renderer tests exercise its adapter separately.

## Naming compatibility

The native binary prefix is `gpui-native`. `GPUI_NATIVE_LIBRARY_PATH` overrides
the addon location; `GPUI_VUE_NATIVE_LIBRARY_PATH` remains a fallback for existing
setups. Existing npm imports (`@gpui-native/core`, `@gpui-native/vue`, and the
published `gpui-vue` alias) keep their names. The Rust crate is now `gpui-core`
(`gpui_core` in Rust imports); scripts using the previous crate or artifact
names must use the new names.

The committed native declarations include `TestGpuiRenderer`, which is available
only in macOS and Windows builds with `test-support`. Linux production builds
do not export that class. CI verifies the complete generated declarations on
the supported test-renderer platforms.

## GPU canvas

The shared Rust `gpu` engine owns wgpu resources, command encoding and validated
submission. `gpu_binding` exposes Node-API operations to Bun and Node;
`renderer/wasm` exposes the compositor's existing browser device/texture objects.
React, Vue and Svelte share `GPUCanvas` lifecycle and presentation in `packages/runtime`.
No framework state enters the Rust GPU engine or `packages/core`.

Direct presentation publishes bounded immutable GPU snapshots with producer and
compositor completion leases. It never sends presentation pixels through JSON,
N-API byte arrays or the CPU image atlas. Explicit `async-readback` retains the
older fallback. See [the GPU canvas guide](webgpu.md) for device ownership,
upstream fork changes, platform qualification, API gaps and performance evidence.

## Svelte host integration

Svelte owns its rune dependency graph, scheduling, keyed blocks and lifecycle.
The adapter compiles native tags to typed component functions with Svelte's
`fragments: "tree"` compiler option. An isolated bundle of the pinned official
client runtime manipulates a private structural node tree. No browser globals
are replaced; no CSS, layout, input handling or painting is implemented in that
tree. Dirty values and parents lower to shared node operations and batching.
Native editors retain GPUI's text/IME behavior while `bind:value` updates Svelte
state. The compiler and runtime must be upgraded together. See [Svelte](svelte.md)
for supported syntax and verification.
