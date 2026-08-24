// The parity engine is an attributed upstream port. Keep its formatting and
// algorithms close enough for future source-level updates; project-owned glue
// (build.rs and the Vue package) remains linted normally.
#![allow(dead_code, clippy::all, clippy::pedantic)]

mod audio;
mod automation;
mod custom_elements;
mod diff;
mod element_tree;
mod markdown;
mod motion;
mod renderer;
mod retained_tree;
mod style;
mod syntax;
mod text;
mod theme;

#[cfg(all(feature = "test-support", target_os = "macos"))]
mod test_renderer;

pub use element_tree::*;
pub use renderer::*;
pub use style::*;
