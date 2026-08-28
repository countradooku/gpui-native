//! Shared conversion and dispatch for renderer-originated GPUI events.

#![allow(
    clippy::cast_precision_loss,
    reason = "validated element IDs are represented by JavaScript Number values"
)]

use super::EventCallback;
use crate::element_tree::EventPayload;

/// Helper to convert a GPUI Point<Pixels> to (f64, f64).
pub(crate) fn point_to_xy(p: gpui::Point<gpui::Pixels>) -> (f64, f64) {
    (f64::from(f32::from(p.x)), f64::from(f32::from(p.y)))
}

/// Convert GPUI `MouseButton` to our u32 encoding: 0=left, 1=middle, 2=right.
pub(crate) fn mouse_button_to_u32(button: gpui::MouseButton) -> u32 {
    match button {
        gpui::MouseButton::Left => 0,
        gpui::MouseButton::Middle => 1,
        gpui::MouseButton::Right => 2,
        gpui::MouseButton::Navigate(_) => 3,
    }
}

/// General-purpose event emitter. Builds a default `EventPayload`, lets the
/// caller customize it via a closure, then sends it through the callback.
/// Production: queues on Node.js event loop via `ThreadsafeFunction`.
/// Tests: pushes to a synchronous Vec for `drainEvents()`.
#[allow(
    clippy::ref_option,
    reason = "all event sources retain an Option callback and pass it without cloning"
)]
pub(crate) fn emit_event_full(
    callback: &Option<EventCallback>,
    element_id: u64,
    event_type: &str,
    build: impl FnOnce(&mut EventPayload),
) {
    if let Some(cb) = callback {
        let mut payload = EventPayload {
            element_id: element_id as f64,
            event_type: event_type.to_string(),
            ..Default::default()
        };
        build(&mut payload);
        cb(payload);
    }
}
