# Canvas transport benchmark, 2026-09-09

Apple M5 Pro, Metal, arm64, macOS 26.6.2 (25G83; Darwin 25.6.0).
Native addon: Cargo release build, Dawn 0.6.0 for the baseline, wgpu 29.0.4 plus the pinned browser-accessor
patch. Runtimes: Node 24.20.0 and Bun 1.4.0. Hardware adapter,
not software Vulkan. Results are a short local sample, not production qualification.

The comparison uses the same changing clear pass and the existing PR #7
asynchronous readback transport, now driven by wgpu, versus the new GPU snapshot
transport. One queue submission covers all canvases. Each case has 30 warmup
frames and three seconds of sustained sequential frames, awaiting all presents,
with two requested in-flight slots. Cases run sequentially, browser animations
closed, with no concurrent build or other GPU test initiated by this task.
Normal desktop background activity and thermal state are uncontrolled.
The final wgpu samples use native engine commit `27d3d96`, including producer
and compositor completion leases.

The native engine performs one-millisecond nonblocking polling. That overhead is
visible in small-frame timings. This benchmark exercises the producer copy and
publication but **does not open a composited display window**. GPU rendering
timestamps, atlas upload, GPUI composition, vsync and display latency are excluded.
The readback path's extra atlas upload is therefore absent too. No end-to-end FPS
or universal speed claim is justified.

## Total frame CPU/wait duration in milliseconds

Cells show p50 / p95 / p99. Total includes scene encoding, submission and waiting
for presentation; it is not just the texture copy's GPU time.

| Runtime      | Resolution  | Canvases |          Dawn readback |            wgpu readback |           wgpu GPU copy |
| ------------ | ----------- | -------: | ---------------------: | -----------------------: | ----------------------: |
| Node 24.20.0 | 640 × 400   |        1 |  0.247 / 0.304 / 0.390 |    1.268 / 1.526 / 2.702 |   1.290 / 3.798 / 3.868 |
| Node 24.20.0 | 640 × 400   |        4 |  0.478 / 0.583 / 0.734 |    1.266 / 2.229 / 2.560 | 2.322 / 11.499 / 21.872 |
| Node 24.20.0 | 1920 × 1080 |        1 |  0.653 / 0.803 / 0.952 |    1.895 / 2.680 / 2.906 |   1.281 / 6.135 / 6.416 |
| Node 24.20.0 | 1920 × 1080 |        4 |  2.115 / 2.530 / 3.131 |    4.306 / 5.116 / 5.391 |  2.508 / 6.454 / 15.444 |
| Node 24.20.0 | 3840 × 2160 |        1 |  2.177 / 2.614 / 2.926 |    5.512 / 6.807 / 7.586 |   1.277 / 2.633 / 3.847 |
| Node 24.20.0 | 3840 × 2160 |        4 | 8.212 / 9.263 / 10.251 | 18.755 / 23.636 / 27.468 |  2.560 / 5.118 / 11.568 |
| Bun 1.4.0    | 640 × 400   |        1 |                      — |    1.521 / 2.061 / 2.253 |   1.263 / 1.312 / 1.464 |
| Bun 1.4.0    | 640 × 400   |        4 |                      — |    2.532 / 3.518 / 4.339 |   1.268 / 1.336 / 2.303 |
| Bun 1.4.0    | 1920 × 1080 |        1 |                      — |    2.398 / 3.070 / 3.313 |   1.262 / 1.304 / 1.387 |
| Bun 1.4.0    | 1920 × 1080 |        4 |                      — |    6.114 / 6.723 / 7.597 |   1.264 / 1.380 / 2.536 |
| Bun 1.4.0    | 3840 × 2160 |        1 |                      — |    5.018 / 6.016 / 6.860 |   1.262 / 1.348 / 2.540 |
| Bun 1.4.0    | 3840 × 2160 |        4 |                      — | 15.477 / 18.231 / 22.648 |   1.264 / 2.531 / 2.625 |

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
The final Node sample also shows substantial tail-latency variance (for example,
640×400 with four direct canvases has a 21.872 ms p99). These regressions require
profiling and tuning before claiming consistently low frame latency.

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
