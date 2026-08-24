# Third-party notices

## GPUix native renderer

Portions of `crates/gpui-vue-core` are adapted from the native renderer in
[remorses/gpuix](https://github.com/remorses/gpuix), commit
`b8bbff4ef0dc00dc24c3d3f3ca47818e1be15c69`.

The upstream native package declares the Apache License 2.0. The port retains
the native algorithms and component implementations while replacing the
React-facing identity and host integration with gpui-vue's Vue renderer.

See `LICENSES/Apache-2.0.txt` for the license text.

## Comet

Parts of the native code, diff, Markdown, syntax highlighting, selectable text,
theme, and input implementations are derived from
[zeronsh/comet](https://github.com/zeronsh/comet), as identified in the
upstream source comments.

Comet is licensed under the MIT License. See `LICENSES/Comet-MIT.txt` for the
license and copyright notice.
