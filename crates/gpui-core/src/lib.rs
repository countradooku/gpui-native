// N-API async Send/Sync proofs traverse wgpu's nested native resource graph.
#![recursion_limit = "256"]

// Keep the complete workspace lint policy active for the native engine.

mod accessibility;
#[cfg(target_os = "macos")]
mod app_menu;
mod audio;
mod automation;
mod color;
mod custom_elements;
mod diff;
mod element_tree;
pub mod gpu;
#[cfg(not(target_family = "wasm"))]
mod gpu_binding;
#[cfg(not(target_family = "wasm"))]
pub use gpu_binding::{NativeWgpuAdapter, NativeWgpuDevice, request_wgpu_adapter};
mod gpu_canvas;
mod markdown;
mod motion;
mod renderer;
mod retained_tree;
mod style;
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
