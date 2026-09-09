# Local validation record

Date: 2026-09-09. Apple M5 Pro, arm64, macOS 26.6.2 (25G83), Metal.
Bun 1.4.0 and Node 26.8.1, with an additional Node 24.20.0 async/lifecycle run. The native addon was built in Cargo release mode.

| Check                                   | Result                                                                                                                                      |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `bun run check`                         | Passed: all adapter/example types, lint, formatting, Rust clippy and workspace check                                                        |
| `bun test`                              | Passed: 72 tests                                                                                                                            |
| `cargo test --workspace --all-features` | Passed: 249 Rust tests                                                                                                                      |
| `bun run build:native`                  | Passed; generated N-API declarations included                                                                                               |
| `bun run test:native`                   | Passed: native reopen, Vue/React GPU layout/input/accessibility/cleanup                                                                     |
| `bun run build:pages`                   | Passed; Wasm shared-device integration compiles                                                                                             |
| `bun run test:wgpu`                     | Passed: Node and Bun asynchronous pipeline/lifecycle tests, no unexpected validation errors                                                 |
| `native-webgpu.mts`, direct             | Passed in Node and Bun, actual Metal/GPUI screenshot assertions                                                                             |
| `native-webgpu.mts`, explicit readback  | Passed in Node and Bun                                                                                                                      |
| Browser actual output                   | React and Vue: two distinct animated GPU surfaces visible, compute triangle and textured Three.js cube; no captured console warnings/errors |

The browser check used the Codex in-app browser on localhost, with Pages served
by Vite preview without COOP/COEP. This was visual verification, not an automated
browser pixel/interaction suite. Native screenshot tests include transparency,
clipping, two canvases, repeated reuse, source destruction, device replacement and zero remaining snapshot allocation
bytes after all producer/compositor leases retire.
Physical device removal, multiple physical adapters, Linux display composition
and Windows hardware were not validated locally.

The local rust-lld executable failed to find its LLVM dynamic library when
invoked directly from Cargo for Wasm. Pages was built with an external temporary
linker wrapper setting DYLD_LIBRARY_PATH to that Rust toolchain's `lib` directory,
then executing its rust-lld. `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER` selected
that wrapper. No linker workaround is committed. Use a correctly installed Rust
LLVM linker on another machine. Build output also contains upstream `block`
future-compatibility and Vite chunk/eval warnings; they are not new test failures.

CI now distinguishes hosted software Vulkan tests from the opt-in
`gpu-hardware.yml` workflow, which requires a self-hosted runner labelled `gpu`
and a logged-in display session. A green software run is not hardware validation.
See [the support matrix and reproducible hardware steps](../webgpu.md).

## Production-window repaint regression

A user-run native example exposed a missing test boundary: asynchronous Metal
publication attempted to access AppKit's main-thread window state from a worker.
The test renderer and headless benchmarks did not exercise that path. macOS
publication now marks an atomic dirty flag; the existing main-thread host tick
performs the repaint. `native-webgpu-window.mts`, included in `test:wgpu`, runs a
real AppKit window in Bun and Node, asserts sustained presentation and actual
compute/Three.js screenshot pixels, and checks resizing. Both runtimes pass
locally. The React application was also relaunched and its output captured.
