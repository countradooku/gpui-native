# Framework adapter architecture

GPUI Native is the shared engine. Vue is its first framework adapter. React,
Svelte, and other adapters are planned; they are not implemented yet.

## Ownership

| Location                              | Responsibility                                                                                                                          |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/gpui-core`                    | Retained tree, batch validation, layout, rendering, native components, text, input, focus, scrolling, windows, and native/Wasm bindings |
| `packages/core` (`@gpui-native/core`) | Native addon loading, generated ABI types, and platform binaries; no framework runtime dependency                                       |
| `packages/vue` (`@gpui-native/vue`)   | Vue reconciliation, components, refs, composables, and Vue-facing renderer helpers                                                      |
| `examples`                            | Current Vue examples                                                                                                                    |
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

The current TypeScript browser wrapper, convenience types, batching helpers,
and automation helpers still live in `packages/vue`. Some include Vue types or
lifecycle assumptions. They are not a shared JavaScript runtime yet. When a
second adapter needs these helpers, extract the framework-independent contract
and implementation into a shared package, keeping Vue refs, VNodes, lifecycle
hooks, and reconciliation in the Vue adapter. Do not make React or Svelte
import Vue to reuse an engine capability.

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
