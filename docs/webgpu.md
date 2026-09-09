# wgpu canvas

React and Vue share a framework-neutral Rust wgpu engine and renderer-owned
canvas sources. Desktop uses the project's Node-API binding in Bun and Node.
Dawn is no longer a production dependency; it remains a development-only
benchmark baseline. Existing canvas element props, retained vector overlays,
framework lifecycle hooks and `configure` / `getCurrentTexture` / `present`
remain available.

**This change is not fully production-qualified.** Windows direct presentation
is not implemented. Linux hardware/display validation, Metal multi-GPU validation,
a browser automation suite, full WebGPU conformance and long-duration soak tests
remain release gates. Do not infer platform validation from a successful build.

## Platform matrix

| Platform          | Normal presentation                                                        | Implemented                           | Validation in this change                                                                                              |
| ----------------- | -------------------------------------------------------------------------- | ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| macOS / Metal     | wgpu render → GPU snapshot copy → retained MTLTexture sampled by GPUI      | Yes, compatible Metal device required | Apple M5 Pro: real GPUI pixel assertions in Bun and Node, alpha, clipping, two canvases, resize, recovery and Three.js |
| Linux / Vulkan    | GPUI's device and queue → GPU snapshot copy → GPUI texture surface         | Yes                                   | Target-specific CI added; no local Linux display/hardware validation                                                   |
| Browser / WebGPU  | GPUI's actual browser GPUDevice → GPU snapshot copy → GPUI texture surface | Yes                                   | Wasm/Pages build and actual React/Vue scenes visually checked in the macOS in-app browser                              |
| Windows / DirectX | Explicit asynchronous readback                                             | Direct sharing **not implemented**    | No local Windows hardware validation; native build and binding regression CI configured                                |
| Browser / WebGL   | Explicit asynchronous readback from a separately available WebGPU device   | Fallback only                         | Not validated here; no WebGPU device means no GPU canvas                                                               |

These direct paths are **GPU-copy paths, not zero-copy**. Source content is copied
once into an immutable, pooled snapshot; GPUI samples the snapshot during normal
composition. There is no pixel map, CPU transfer or atlas upload on these paths.
Screenshots and buffer readbacks used to assert correctness are test operations,
not part of normal presentation. `canvas.stats.transport` distinguishes
`gpu-copy` from `async-readback`; `bytesPresented` counts CPU pixel bytes and is
zero for direct frames. A missing direct capability fails explicitly. There is
no automatic fallback.

## Device acquisition and migration

```ts
// Native entry point, supported in Bun and Node.
import { createNativeGPU } from "@gpui-native/react/webgpu-native"
const gpu = await createNativeGPU()
// Browser: use navigator.gpu; do not import the native module.

// canvas comes from useGPUCanvas() or createGPUCanvas({ renderer, ... }).
// Mount it first: Linux/browser acquire the compositor device after GPUI starts.
const device = await canvas.requestDevice(gpu)
canvas.configure({ device, format: "bgra8unorm", alphaMode: "opaque" })
const encoder = device.createCommandEncoder()
const pass = encoder.beginRenderPass({
  colorAttachments: [
    {
      view: canvas.getContext("webgpu").getCurrentTexture().createView(),
      clearValue: [0.1, 0.2, 0.8, 1],
      loadOp: "clear",
      storeOp: "store",
    },
  ],
})
pass.end()
device.queue.submit([encoder.finish()])
await canvas.present()
// Release scene resources. Only destroy an owned device:
if (canvas.ownsDevice) device.destroy()
```

Use `@gpui-native/vue/webgpu-native` for Vue. Both delegate to the same runtime.
`requestDevice(gpu, descriptor)` checks requested features and limits on an
already-created shared device; it cannot retroactively enable new capabilities.
It waits up to ten seconds for GPUI initialization. Request the device once per
canvas lifetime and retain the ownership decision with that device. Never
destroy a shared compositor device from application cleanup.

Migration from PR #7:

- Direct presentation is now the default. Acquire the device through
  `canvas.requestDevice(gpu)`, particularly on Linux/browser. A device requested
  independently from a browser adapter is a different owner and is rejected.
- For Windows or a foreign WebGPU implementation, opt in explicitly with
  `createGPUCanvas({ renderer, width, height, presentation: "async-readback" })`.
  React/Vue hooks accept the same option. This retains the old transfer path.
- Remove Dawn option strings from `createNativeGPU` / `installWebGPU`; these now
  reject. Use standard adapter/device descriptors. Bun no longer needs a bypass.
- After resize/reconfiguration, previous frames are invalidated. On `contextlost`,
  dispose owned resources, acquire a replacement device and configure again.
- Do not import the desktop binding into browser bundles. Do not hand-edit
  generated `packages/core/index.d.ts`; the N-API build regenerates it.

`present()` resolves true when published, false on backpressure or an obsolete
completion, and rejects real GPU/transport errors. Submission never implicitly
presents. Handle promises. The context reuses an offscreen texture until resize
or reconfiguration; it does not implement browser swapchain expiration.

## Architecture and ownership

`gpu.rs`, `gpu/descriptors.rs` and `gpu/commands.rs` own the typed resource registry,
validation, command encoding and submission. IDs are process-wide monotonic
integers: stale, released and foreign-device handles fail. Command buffers are
single-use. JavaScript command recording retains referenced objects until native
encoding; asynchronous pipeline tasks retain dependencies until completion.
Framework adapters only create, resize and release a shared `GPUCanvas`.

`gpu/presentation.rs` maintains at most three immutable snapshots per source.
Publication takes a short lock to reserve a generation/sequence, performs GPU
allocation/copy/wait outside renderer/tree locks, then rechecks the generation
before publishing. Resize, replacement, loss and teardown invalidate old work.
The latest completion cannot be overwritten by an older one. GPUI scenes and
GPU-completion callbacks retain allocation leases; a texture is reusable only
when its producer and compositor users have released it. Destruction is
idempotent. Closing a renderer invalidates its sources and its shared engine.
A source can be used by several elements in its renderer, never another window.

Native producer completion is driven by a weakly owned poll worker, with
nonblocking polls at roughly one millisecond intervals. No GPU completion waits
run under a UI tree lock. The Metal producer and GPUI have separate command
queues; producer completion is awaited before publication and GPUI completion
retains the snapshot. Both wrappers retain the same native MTLTexture object.
GPUI checks the Metal device identity before sampling. A mismatched device is
currently logged and skipped by GPUI, not propagated through `present()`; this is
a remaining correctness limitation on multi-GPU Macs. There is no IOSurface
cross-device migration in this implementation.

Linux and browser use GPUI's exact device/queue. The browser exposes its actual
GPUDevice/GPUTexture through small wgpu accessors, while Rust owns handles and
snapshot lifetime. `single_threaded_web()` remains in use. The
`fragile-send-sync-non-atomic-wasm` feature makes single-threaded JS handles usable
by wgpu interfaces; it does not enable shared memory, atomics or worker threads.
No COOP/COEP headers are required.

GPUI changes are pinned in
[countradooku/zed](https://github.com/countradooku/zed/compare/1f9d1cd88656cf1759b0bdad32fa3e2df3c4b0b9...gpui-native/wgpu-canvas).
They add leased RGBA Metal surfaces, browser access to the existing wgpu device,
alpha/opacity handling and per-surface dynamic uniform offsets. The last fix
prevents multiple surfaces from all sampling with the final surface's bounds.
The pinned [wgpu 29.0.4 patch](https://github.com/countradooku/wgpu/compare/v29.0.4...gpui-native/webgpu-interop)
only exposes browser device/texture objects. Cargo.lock pins both forks; core,
hal and naga remain from the matching wgpu revision. These are reviewable fork
changes, not claimed to be accepted upstream.

Windows GPUI uses D3D11 while wgpu uses D3D12. Sharing needs a compatible adapter,
shared NT resource handles, D3D11/D3D12 format/usage compatibility and shared fence
synchronization in both directions. Simply casting a texture or passing a handle
would not be correct. This bridge is not implemented or hidden behind a
“zero-copy” claim. Reproduce and validate it on Windows before enabling direct
presentation there.

## API compatibility

Existing bindings were evaluated before adding the Node-API bridge:

| Binding                                                              | Evaluation                                                                                                                                 |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| [Dawn Node WebGPU](https://github.com/dawn-gpu/node-webgpu) 0.6.0    | Broad API and useful baseline, but a separate GPU engine and the PR #7 Bun asynchronous-pipeline crash                                     |
| [SylphxAI/webgpu](https://github.com/SylphxAI/webgpu)                | Rust/N-API, but inspected checkout used wgpu 0.19; incompatible with GPUI's wgpu 29 shared device and incomplete required feature handling |
| [argon-chat/wgpu](https://github.com/argon-chat/wgpu)                | wgpu-native 29, but Bun FFI integration does not supply a Node binding or GPUI's existing Rust device                                      |
| [Deno WebGPU](https://github.com/denoland/deno/tree/main/ext/webgpu) | Maintained wgpu implementation tied to V8/Deno; not a Bun/Node-API drop-in                                                                 |

The new bridge is a **tested subset, not a certified WebGPU implementation**.
The browser uses the browser's real WebGPU API. Desktop compatibility is:

| Area         | Native implementation / limits                                                                                                      |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| Buffers      | Uploads, copies, mapping, mapped-at-creation, detach on unmap, storage/uniform/vertex/index/indirect uses                           |
| Textures     | Explicit uploads, views, arrays/cube maps, copies, samplers, depth/stencil and MSAA resolve                                         |
| Pipelines    | WGSL render/compute, layouts, overrides, synchronous and asynchronous creation, compilation info                                    |
| Commands     | Render/compute passes, indexed/indirect draws, dispatch, viewport/scissor, stencil/blend state, occlusion queries, debug markers    |
| Errors/loss  | Real wgpu validation; JS validation/OOM/internal error scopes and uncaptured errors; device loss and explicit destroy               |
| Capabilities | Actual enabled features and limits, filtered to implemented WebGPU features; no invented native extensions                          |
| Three.js     | 0.185.1 tested textured DataTexture cube, depth, 4× MSAA, resize; not all materials/loaders/nodes                                   |
| Unsupported  | Render bundles/executeBundles, external image/video texture import and copy, pass timestamp writes/timestamp-query; explicit errors |
| Host APIs    | No DOM, image decoder, video, 2D canvas, OffscreenCanvas transfer or automatic camera-control event bridge                          |

Mapped native bytes are copied into JS-owned ArrayBuffers; mapping and texture
uploads are not zero-copy APIs. Error scopes model the supported operations but
have not passed the WebGPU CTS, including all asynchronous ordering edge cases.
Optional descriptor fields outside the implemented decoder need a compatibility
audit; do not assume acceptance means that every WebGPU extension is implemented.
Out-of-memory and physical device removal are not reliably injectable on the
local hardware and remain unvalidated. Application resource allocation is limited
by device limits and a 65,536-handle registry, not by the canvas snapshot budget.

Presentation accepts `rgba8unorm` / `bgra8unorm`, sRGB color space and opaque or
premultiplied alpha. GPUI converts sampled premultiplied colors for its straight
alpha blending. HDR, Display P3 and extended tone mapping are rejected. Render
HDR offscreen and tone-map into the presentation texture yourself.

## Bounds and performance

Limits: dimensions 1–8192; 64 MiB per snapshot; 256 MiB process-wide snapshot
budget including leased frames; 256 live sources per renderer. The JS presenter
allows one to three in-flight requests (default two), dropping busy work. GPU
source textures, application buffers/textures and driver allocations are
additional. Explicit readback additionally pools staging buffers and retains CPU
images/atlas resources. Its current-image budget is separate from old painted
images and readback slots.

Commands cross the language boundary once per encoder finish and once per queue
submission; uploads transfer typed bytes. Texture snapshots are reused. The
current implementation still allocates descriptor/command objects, polls native
completion and creates compositor bind groups per frame on wgpu. Those costs
are measured or listed as limitations, not claimed eliminated.

See [benchmark results and methodology](benchmarks/wgpu-canvas.md). The benchmark
separates CPU scene encoding, submission and synchronization/transport. It does
not isolate GPU scene time or physical display composition. No fastest or
end-to-end frame-rate claim follows from it.

## Examples and verification

Build the native addon and adapters, then run either example with Bun or Node:

```sh
bun run build:native
bun run build:adapters
bun --filter @gpui-react/example-webgpu build
bun examples/react-webgpu/dist/main.js
node examples/react-webgpu/dist/main.js
bun --filter @gpui-vue/example-webgpu build
bun examples/webgpu/dist/main.js
node examples/webgpu/dist/main.js
```

Both examples show two canvases: a compute-driven 4× MSAA WGSL triangle and a
textured Three.js cube with depth and 4× MSAA. Controls resize both canvases and
unmount/remount their scopes. Native `installWebGPU()` explicitly installs
constructors, navigator.gpu and missing animation timers; its cleanup restores
previous globals. Three receives an explicit device and therefore does not destroy
the shared compositor device on renderer disposal.

React's hook creates resources in an effect, handles StrictMode replay and
returns null before mount. Vue owns the source in its setup scope. Async startup
checks for disposal before adopting resources. Do not place GPU allocation in a
React render function or share a source across framework roots/windows.

```sh
bun run check
bun test
cargo test --workspace --all-features
bun run build:native
bun run test:native
bun run build:pages
bun run test:wgpu             # Node + Bun async/lifecycle regression
bun run test:webgpu           # explicit readback, compute/textures/Three
GPUI_GPU_PRESENTATION=direct bun packages/runtime/test/native-webgpu.mts
cd packages/runtime
GPUI_GPU_PRESENTATION=direct node --import tsx test/native-webgpu.mts
```

Native GPU tests assert compute values, cube sampling, MSAA, pixel channels,
transparency, clipping, two surfaces, repeated pool reuse, stale handles,
resize/unmount during completion, idempotent destruction and explicit device
replacement. macOS/Windows offer GPUI screenshot tests; Linux does not. Direct
screenshot assertions currently run on macOS. Unit tests exercise ordering,
backpressure, device loss, React StrictMode/Suspense and Vue scope cleanup.
CI labels hosted Linux Mesa as software Vulkan and runs Node/Bun regressions;
that does not validate Linux window composition or Windows sharing.

For hardware qualification: run both native example runtimes on each OS, verify
both surfaces animate independently, resize and remount at least 100 times, open
two application windows, close one during rendering, and confirm the other keeps
rendering. Capture pixel assertions on supported native test renderers. On Linux
repeat under X11 and Wayland with the production compositor and record driver,
adapter and Vulkan validation output. On multi-GPU Macs verify device matching.
For browsers, build Pages, serve without COOP/COEP, check both WebGPU routes,
resize/unmount/remount, and inspect console errors and actual output in Chromium,
Firefox and Safari where WebGPU is available. The current local visual check
covers only the in-app browser; cross-browser automation remains outstanding.
