# GPUI Kit source pin and local patches

Source: [longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)

Revision: `84f57fdfcb4910623fb0bb7f795b077e249f9271` (0.6.1).
License: Apache-2.0; the original license and crate notices are retained.

The `base`, `component`, `component-macros`, `assets` and `kit` crates and themes are vendored. The upstream shell runtime, CLI and sample apps are not workspace members. GPUI Native owns its own framework runtimes and application/window hosts.

## Compatibility patches

- `Cargo.toml`: reduce workspace members to the embedded library crates; align GPUI, platform and utility crates with GPUI Native's pinned `countradooku/zed` revision. Rename the `sum_tree` package through the upstream `sum-tree` dependency key. The outer workspace owns the GPU patches and lockfile.
- `component-macros/src/crate_path.rs`: permit component derives to resolve the existing `gpui` crate name as well as upstream's published `gpui-pre` name.
- `assets/build.rs`, `assets/src/lib.rs`: generate an embedded icon index and expose `EmbeddedAssets` as a synchronous `AssetSource` on native and web. This avoids an icon server and makes static browser hosting self-contained.

## Shared text and focus patches

- `component/src/text/compat.rs`: add a custom text renderer that retains its plain-text value for accessibility and component metadata.
- `alert.rs`, `breadcrumb.rs`, `separator.rs`, `sidebar/menu.rs`, `label.rs`: accept or expose the custom text content while preserving existing component structure and accessible/search metadata.
- `plot/label.rs`: add a scoped label painter so canvas labels can register in GPUI Native's existing selection, search and highlight pipeline.
- `shimmer.rs`: add a paint observer for the original text layout, preserving the native animated glyph painter while registering the text with the same selection pipeline.
- `component/src/button/button.rs`, `button/toggle.rs`, `checkbox.rs`, `radio.rs`, `switch.rs`, and `base/src/switch.rs`: allow the retained host to supply a stable focus handle. Entity-backed controls expose their existing handle through the GPUI Native adapter.

## Overlay ownership patches

- `component/src/root.rs`: associate optional weak owners with dialogs and sheets; prune overlays whose retained host was removed and restore the appropriate previous focus. Pruning retains unrelated overlays.
- `component/src/notification.rs`, `root.rs`: associate optional weak owners with notifications and begin dismissal when their host disappears. Host content checks the owner before rendering, including during the exit animation.

## Chart correction

- `component/src/chart/pie_chart.rs`: preserve the calculated automatic outer radius when painting slices. Upstream passed the unset zero radius as an override, hiding slices while their labels remained visible. Explicit and per-slice radii still take precedence.

The patches add embedding seams; they do not introduce a second component renderer or a framework dependency into Kit. Framework-specific models and callbacks live in `packages/<framework>/src/kit.ts`; native adapters live in `crates/gpui-core/src/kit`.

When updating the source pin, replay these changes, regenerate the component catalog if the public API changes, and run the repository's complete quality gate, native lifecycle/interaction suite, and browser build. Test native focus and overlay removal explicitly: compilation alone cannot verify those integration seams.
