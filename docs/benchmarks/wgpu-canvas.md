# Canvas transport benchmark, 2026-09-09

Apple M5 Pro, Metal, arm64, macOS 26.6.2 (25G83; Darwin 25.6.0).
Native addon: Cargo release build, Dawn 0.6.0 for the baseline, wgpu 29.0.4 plus the pinned browser-accessor
patch. Runtimes: Node 24.20.0, Node 26.8.1 and Bun 1.4.0. Hardware adapter,
not software Vulkan. Results are a short local sample, not production qualification.

The comparison uses the same changing clear pass and the existing PR #7
asynchronous readback transport, now driven by wgpu, versus the new GPU snapshot
transport. One queue submission covers all canvases. Each case has 30 warmup
frames and three seconds of sustained sequential frames, awaiting all presents,
with two requested in-flight slots. Cases run sequentially, browser animations
closed, with no concurrent build or other GPU test initiated by this task.
Normal desktop background activity and thermal state are uncontrolled.

The native engine performs one-millisecond nonblocking polling. That overhead is
visible in small-frame timings. This benchmark exercises the producer copy and
publication but **does not open a composited display window**. GPU rendering
timestamps, atlas upload, GPUI composition, vsync and display latency are excluded.
The readback path's extra atlas upload is therefore absent too. No end-to-end FPS
or universal speed claim is justified.

## Total frame CPU/wait duration in milliseconds

Cells show p50 / p95 / p99. Total includes scene encoding, submission and waiting
for presentation; it is not just the texture copy's GPU time.

| Runtime      | Resolution  | Canvases |          Dawn readback |            wgpu readback |         wgpu GPU copy |
| ------------ | ----------- | -------: | ---------------------: | -----------------------: | --------------------: |
| Node 24.20.0 | 640 × 400   |        1 |  0.247 / 0.304 / 0.390 |    1.267 / 1.730 / 2.666 | 1.262 / 2.649 / 3.837 |
| Node 24.20.0 | 640 × 400   |        4 |  0.478 / 0.583 / 0.734 |    1.267 / 2.021 / 2.486 | 1.275 / 5.115 / 6.188 |
| Node 24.20.0 | 1920 × 1080 |        1 |  0.653 / 0.803 / 0.952 |    2.175 / 2.694 / 3.002 | 1.257 / 6.319 / 6.412 |
| Node 24.20.0 | 1920 × 1080 |        4 |  2.115 / 2.530 / 3.131 |    4.680 / 5.592 / 6.323 | 1.266 / 2.568 / 5.062 |
| Node 24.20.0 | 3840 × 2160 |        1 |  2.177 / 2.614 / 2.926 |    4.963 / 5.905 / 6.593 | 1.261 / 2.550 / 3.781 |
| Node 24.20.0 | 3840 × 2160 |        4 | 8.212 / 9.263 / 10.251 | 15.541 / 17.719 / 18.379 | 1.268 / 3.724 / 4.872 |
| Bun 1.4.0    | 640 × 400   |        1 |                      — |    1.574 / 2.248 / 2.687 | 1.264 / 1.314 / 2.424 |
| Bun 1.4.0    | 640 × 400   |        4 |                      — |    2.499 / 3.206 / 3.592 | 1.273 / 2.564 / 2.772 |
| Bun 1.4.0    | 1920 × 1080 |        1 |                      — |    2.688 / 3.472 / 4.329 | 1.263 / 2.520 / 2.579 |
| Bun 1.4.0    | 1920 × 1080 |        4 |                      — |    5.965 / 6.637 / 7.341 | 1.267 / 2.546 / 2.570 |
| Bun 1.4.0    | 3840 × 2160 |        1 |                      — |    5.124 / 6.393 / 6.837 | 1.263 / 2.558 / 2.621 |
| Bun 1.4.0    | 3840 × 2160 |        4 |                      — | 15.060 / 17.344 / 19.220 | 1.267 / 1.320 / 2.544 |

Direct runs reported zero CPU pixel bytes. The readback runs transfer four bytes
per pixel per canvas per accepted frame, plus staging padding and CPU conversion.
At four 4K canvases that is 126.6 MiB of unpadded pixels per frame. Direct snapshots
still occupy GPU memory and incur a GPU copy; they are not zero-copy.

## Separate measurements and limitations

Raw reports include per-case p50/p95/p99 for CPU scene encoding, CPU submission,
and synchronization/transport, frame counts, elapsed sample time, CPU user/system
usage, peak process RSS and total CPU pixel bytes:

- [Dawn / Node 24.20.0 raw report](dawn-node24.json)
- [Node 24.20.0 raw report](wgpu-node24.json)
- [Node 26.8.1 raw report](wgpu-node.json)
- [Bun 1.4.0 raw report](wgpu-bun.json)

CPU usage includes warmup. RSS includes the runtime, deferred GC and unified-memory
effects; it does not isolate GPU allocation residency. Cases share a process,
so RSS can include retained runtime capacity from earlier cases. Snapshot memory
is capped separately at 256 MiB; application and source textures are additional.
No GC forcing, GPU profiler, timestamp query or composition instrumentation was
used. Scene GPU time and composition time are explicitly null in the raw data.
Repeated longer runs, richer scenes, hardware counters and compositor profiling
are needed before a performance release decision.

## Dawn baseline

Dawn 0.6.0 is a development-only baseline, explicitly selecting Metal. The
harness retains Dawn's root GPU wrapper for the lifetime of native callbacks and
requests a fresh adapter for each device. Those lifetime requirements also apply
when reproducing the older PR #7 implementation. An early harness without those
safeguards crashed; it was corrected before collecting the reported baseline.

The historical PR #7 table used a short, static-texture workload and is not
substituted for this sustained multi-canvas comparison. Both completed transports
use the same changing-clear workload here. Direct presentation has an observable
polling cost at small sizes; avoiding CPU pixels helps the high-resolution cases.

## Reproduce

Build the native addon first. Run the cases sequentially with other GPU workloads
closed; use an available hardware adapter. The current headless direct benchmark
is intended for Metal. Linux direct needs the device of an initialized GPUI window
and is not measured by this harness.

```sh
bun run build:native
cd packages/runtime
GPUI_BENCH_OUTPUT=wgpu-node.json node --import tsx test/bench-wgpu.mts
GPUI_BENCH_OUTPUT=wgpu-bun.json bun test/bench-wgpu.mts
GPUI_BENCH_BACKEND=dawn GPUI_BENCH_OUTPUT=dawn-node.json node --import tsx test/bench-wgpu.mts
```

Set `GPUI_BENCH_SECONDS=30` for longer cases. On this host Node 24 was additionally
selected using `npx --yes --package=node@24 node`; its actual reported version
was 24.20.0. Preserve the report's runtime/adapter metadata rather than assuming
the version from a command name. See [validation results](wgpu-validation.md).
