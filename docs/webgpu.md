# WebGPU canvas

React and Vue share a renderer-owned GPU canvas source. Use real WebGPU devices
from the browser, or Dawn's `webgpu` package in Node.js, and display the result
with `<canvas source={canvas.id}>` / `<GpuiCanvas :source="canvas.id" />`.
Existing retained `commands` paint over the GPU image. Layout, clipping, focus,
mouse/keyboard events, and window ownership still belong to GPUI.

## Desktop and browser

```ts
// Desktop only: run the built application with Node.js.
import { createNativeGPU } from "@gpui-native/react/webgpu-native"
const gpu = await createNativeGPU()
// Browser: use navigator.gpu instead. Do not import the native entry point.
const adapter = await gpu.requestAdapter({ powerPreference: "high-performance" })
if (!adapter) throw new Error("No WebGPU adapter")
const device = await adapter.requestDevice()
```

The Vue equivalent is `@gpui-native/vue/webgpu-native`. Both frameworks also
export `createGPUCanvas({ renderer, width, height, maxFramesInFlight })` for
applications that want to manage ownership explicitly.

React's `useGPUCanvas({ width, height })` returns `null` until its effect creates
the canvas, then returns the canvas. It resizes that source when dimensions
change and destroys it on unmount, including StrictMode replay. Configure and
render from an effect, and dispose your device/resources in its cleanup.

Vue's `useGPUCanvas({ width, height })` returns a canvas during setup and destroys
it when the setup scope ends. Call `canvas.resize(width, height)` for later size
changes. The GPU device is application-owned in both adapters.

```ts
canvas.getContext("webgpu").configure({
  device,
  format: "rgba8unorm",
  alphaMode: "opaque",
})
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
```

`present()` resolves `true` if the frame was published, `false` if backpressure,
resize, disposal, or a newer completed frame made it obsolete. Actual GPU or
transport failures reject. Always handle the returned promise. No queue methods
are monkey-patched, and submitting GPU work does not implicitly present it.

## Examples

- `bun --filter @gpui-react/example-webgpu build`, then
  `node examples/react-webgpu/dist/main.js`.
- `bun --filter @gpui-vue/example-webgpu build`, then
  `node examples/webgpu/dist/main.js`.
- The Pages gallery includes **React WebGPU** and **Vue WebGPU**. Both use the
  browser's WebGPU device and the single-threaded GPUI Wasm build; they need no
  shared memory, COOP, or COEP headers.

The examples share a rotating WGSL triangle with four-sample antialiasing and a
retained vector border. Unsupported browsers show an explicit startup error.

### Three.js

For desktop libraries expecting browser globals, explicitly call
`await installWebGPU()` from the native entry point. It installs Dawn's
constructors/constants, `navigator.gpu`, and missing animation scheduling globals.
The returned function restores previous globals and cancels its pending timers.
Dispose renderers/devices before restoring. Avoid overlapping installations.

```ts
import { installWebGPU } from "@gpui-native/react/webgpu-native"
import { WebGPURenderer } from "three/webgpu"
const restore = await installWebGPU()
const three = new WebGPURenderer({
  canvas: canvas as unknown as HTMLCanvasElement,
  antialias: true,
})
three.setSize(canvas.width, canvas.height, false)
await three.init()
three.render(scene, camera)
await canvas.present()
// On teardown: dispose scene resources, three.dispose(), then restore().
```

This canvas supplies an event target, width/height, client dimensions, style,
and a WebGPU context. It is **not** a DOM element: DOM controls, media elements,
`getContext("2d")`, and browser image/video loading require separate host support.
Forward GPUI pointer events to camera controls explicitly. Three.js 0.185.1 is
covered by the native GPU smoke test.

## Supported operations and limits

The GPU API comes from the device implementation, including compute, WGSL,
buffers, texture uploads, samplers, cube maps, multisampling, asynchronous
pipelines, error scopes, and device-loss reporting. Consult `adapter.features`
and `device.limits`; GPUI does not fabricate capabilities. Platform/media APIs
outside Dawn's Node bindings are not provided by GPUI.

Presentation supports `rgba8unorm` and `bgra8unorm`, standard sRGB, and opaque or
premultiplied alpha. Alpha is converted to GPUI's straight-alpha image format.
HDR formats, extended tone mapping, and Display P3 are rejected explicitly.
Use an offscreen HDR target and a tone-mapping pass into the presentation target.

The context retains/reuses an offscreen texture until resize/reconfiguration;
it does not expire that texture automatically like a browser swapchain. Old
completed pixels remain visible during reconfiguration or device loss until a
replacement arrives or the source is destroyed. Listen for `contextlost`, obtain
a replacement device, and configure it explicitly. Destroying the canvas is
idempotent and immediately invalidates future presentation attempts.

Desktop Dawn currently requires **Node.js**. A standalone reproduction of
asynchronous pipeline creation crashes Bun 1.4.0; the native entry point rejects
Bun before loading the binding. Other GPUI functionality still supports Bun.
A Bun application may supply its own compatible `GPUDevice` to `GPUCanvas`.
Single-file Bun executables containing Dawn are not supported by these examples.

## Presentation cost and resource bounds

This is an **asynchronous CPU-readback transport**, not zero-copy. Each frame
copies the GPU texture into a pooled, padded staging buffer, asynchronously maps
it, passes a typed byte array through N-API/Wasm, packs/converts pixels in Rust,
and uploads the resulting image through GPUI's atlas. It never serializes pixel
bytes as JSON or base64. No per-frame animation requests are made by the canvas
primitive: completed frames invalidate their owning renderer.

There are two readback slots by default, configurable from one to three. Busy
slots drop incoming presentation requests rather than growing an unbounded
queue. Resize and destruction cancel obsolete mappings; old completions cannot
overwrite newer frames. GPUI retains the latest submitted image and last painted
image per source; obsolete atlas entries are removed on the next window frame,
including when canvas elements are unmounted or change source.

Limits: dimensions 1–8192, at most 64 MiB per padded frame, 256 live sources and
256 MiB of current images per renderer. Readback buffers and last painted images
are additional memory; budget for them when choosing resolution and slot count.
Sources are renderer-local and must not be shared across windows/renderers.
Unmounting an element releases its atlas entry but does not destroy an externally
owned source, which can be displayed by another canvas in the same renderer.

Read `canvas.stats` for submitted/presented/dropped/failed counts, current
in-flight count, total bytes, and the most recent presentation duration.
`lastPresentMs` includes asynchronous readback and native publication, not display
latency. Statistics are snapshots, not mutable shared state.

### Measured baseline

`bun run bench:webgpu` runs 10 warmup frames and 60 measured frames per size.
On an Apple M5 Pro, macOS 26.6.2, Node 24.20.0, Dawn 0.6.0:

| Size        |   Median |      p95 |
| ----------- | -------: | -------: |
| 640 × 400   | 0.204 ms | 0.296 ms |
| 1920 × 1080 | 0.571 ms | 0.776 ms |
| 3840 × 2160 | 2.210 ms | 2.924 ms |

These numbers measure readback plus native pixel conversion/storage of an opaque
BGRA texture. They exclude scene rendering, atlas upload, composition, and
physical display latency. They are not an end-to-end FPS claim or a benchmark
comparison against gpuix. Profile your workload before choosing a resolution.

## Verification and comparison

`bun run test:webgpu` requires a built native addon, Node, and a working WebGPU
adapter. It checks compute results, cube-map uploads/sampling, 4× MSAA, padded
rows, RGBA/BGRA channel order, transparency, Three.js, and destruction. On macOS
and Windows it also asserts actual GPUI screenshot pixels. Linux uses the
production headless renderer for transport validation. Unit tests exercise
backpressure, ordering, resize, failures, device loss, and adapter ownership.

The referenced [gpuix prototype changeset](https://github.com/remorses/gpuix/blob/prototype/webgpu-canvas/.changeset/webgpu-canvas.md)
explicitly lists MSAA, cube maps, and `writeTexture` as unimplemented. This
implementation tests those operations using [Dawn's Node WebGPU bindings](https://github.com/dawn-gpu/node-webgpu).
That is a concrete capability improvement over that prototype, not a claim of
universal performance superiority or full WebGPU conformance certification.
