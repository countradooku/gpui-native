// The parity engine is an attributed upstream port. Keep its formatting and
// algorithms close enough for future source-level updates; project-owned glue
// (build.rs and the Vue package) remains linted normally.
#![allow(dead_code, clippy::all, clippy::pedantic)]

#[cfg(target_os = "macos")]
mod app_menu;
mod audio;
mod automation;
mod color;
mod custom_elements;
mod diff;
mod element_tree;
mod markdown;
mod motion;
mod renderer;
// The data model is public so `examples/bench_serde.rs` measures the real
// types instead of a copy that silently drifts from them.
pub mod retained_tree;
pub mod style;
mod syntax;
mod text;
mod theme;

#[cfg(all(
    feature = "test-support",
    any(target_os = "macos", target_os = "windows")
))]
mod test_renderer;

pub use element_tree::*;
pub use renderer::*;
pub use style::*;
