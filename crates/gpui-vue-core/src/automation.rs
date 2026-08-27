//! Shared automation host: paint bounds and a controllable motion clock.
//!
//! Record bounds during **paint**, not prepaint. The frame reset canvas
//! clears the map in paint, and GPUI prepaint runs for the whole tree
//! before any paint. A prepaint recorder would be wiped by the reset.
//!
//! TestGpuiRenderer and GpuiRenderer both use this so locators, screenshots,
//! and clock control do not fork between headless tests and a live window.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    canvas, point, px, App, Bounds, InputEvent, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Styled, Window,
};
use web_time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct ElementBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ElementBounds {
    fn from_gpui(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: f64::from(f32::from(bounds.origin.x)),
            y: f64::from(f32::from(bounds.origin.y)),
            width: f64::from(f32::from(bounds.size.width)),
            height: f64::from(f32::from(bounds.size.height)),
        }
    }
}

thread_local! {
    static BOUNDS: RefCell<HashMap<u64, ElementBounds>> = RefCell::new(HashMap::new());
}

/// Zero-size canvas. Paint it with the selection reset, before any content.
pub fn bounds_frame_reset() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, _, _, _| {
            BOUNDS.with(|cell| cell.borrow_mut().clear());
        },
    )
    .absolute()
    .w(px(0.0))
    .h(px(0.0))
}

pub fn record_bounds(id: u64, bounds: Bounds<Pixels>) {
    BOUNDS.with(|cell| {
        cell.borrow_mut()
            .insert(id, ElementBounds::from_gpui(bounds));
    });
}

pub fn get_bounds(id: u64) -> Option<ElementBounds> {
    BOUNDS.with(|cell| cell.borrow().get(&id).copied())
}

pub fn all_bounds() -> HashMap<u64, ElementBounds> {
    BOUNDS.with(|cell| cell.borrow().clone())
}

pub fn bounds_tracker(id: u64, selection_start: Option<bool>) -> impl IntoElement {
    canvas(
        |bounds, _, _| bounds,
        move |bounds, _, _, _| {
            record_bounds(id, bounds);
            if let Some(selectable) = selection_start {
                crate::text::record_start_region(bounds, selectable);
            }
        },
    )
    .absolute()
    .size_full()
}

struct ClockInner {
    origin: Instant,
    anchor_real: Instant,
    anchor_elapsed: Duration,
    playing: bool,
    playback_rate: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct ClockSnapshot {
    pub current_time_ms: f64,
    pub playback_rate: f64,
    pub playing: bool,
}

#[derive(Clone)]
pub struct AutomationClock {
    inner: Arc<Mutex<ClockInner>>,
}

impl Default for AutomationClock {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomationClock {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClockInner {
                origin: Instant::now(),
                anchor_real: Instant::now(),
                anchor_elapsed: Duration::ZERO,
                playing: true,
                playback_rate: 1.0,
            })),
        }
    }

    pub fn now(&self) -> Instant {
        let inner = self.inner.lock().unwrap();
        inner.origin + elapsed(&inner)
    }

    pub fn now_ms(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        elapsed(&inner).as_secs_f64() * 1000.0
    }

    pub fn pause(&self) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        inner.anchor_elapsed = elapsed(&inner);
        inner.playing = false;
        inner.anchor_elapsed.as_secs_f64() * 1000.0
    }

    pub fn set_ms(&self, now_ms: f64) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        inner.anchor_elapsed = duration_ms(now_ms);
        inner.anchor_real = Instant::now();
        inner.playing = false;
        inner.anchor_elapsed.as_secs_f64() * 1000.0
    }

    pub fn seek_ms(&self, now_ms: f64) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        inner.anchor_elapsed = duration_ms(now_ms);
        inner.anchor_real = Instant::now();
        inner.anchor_elapsed.as_secs_f64() * 1000.0
    }

    pub fn fast_forward_ms(&self, delta_ms: f64) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        inner.anchor_elapsed = elapsed(&inner).saturating_add(duration_ms(delta_ms));
        inner.anchor_real = Instant::now();
        inner.playing = false;
        inner.anchor_elapsed.as_secs_f64() * 1000.0
    }

    pub fn resume(&self) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        inner.anchor_elapsed = elapsed(&inner);
        inner.anchor_real = Instant::now();
        inner.playing = true;
        inner.anchor_elapsed.as_secs_f64() * 1000.0
    }

    pub fn set_playback_rate(&self, playback_rate: f64) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        inner.anchor_elapsed = elapsed(&inner);
        inner.anchor_real = Instant::now();
        inner.playback_rate = playback_rate;
        playback_rate
    }

    pub fn snapshot(&self) -> ClockSnapshot {
        let inner = self.inner.lock().unwrap();
        ClockSnapshot {
            current_time_ms: elapsed(&inner).as_secs_f64() * 1000.0,
            playback_rate: inner.playback_rate,
            playing: inner.playing,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.inner.lock().unwrap().playing
    }
}

fn elapsed(inner: &ClockInner) -> Duration {
    if !inner.playing {
        return inner.anchor_elapsed;
    }
    let real = Instant::now().saturating_duration_since(inner.anchor_real);
    let scaled = Duration::from_secs_f64(real.as_secs_f64() * inner.playback_rate);
    inner.anchor_elapsed.saturating_add(scaled)
}

fn duration_ms(ms: f64) -> Duration {
    Duration::from_secs_f64((ms / 1000.0).clamp(0.0, 315_576_000.0))
}

pub fn mouse_button(button: u32) -> MouseButton {
    match button {
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

/// Parse the held modifiers of a simulated mouse event from the same
/// hyphenated syntax `press("cmd-a")` already uses: `"cmd"`, `"cmd-shift"`,
/// `"alt"`. `None` and `""` mean no modifier. Unknown names are ignored, so a
/// typo weakens the gesture instead of failing the whole automation call.
///
/// A string, not an object, because the same value has to cross the napi, the
/// wasm and the stdio boundary, and only wasm makes objects awkward.
pub fn parse_modifiers(modifiers: Option<&str>) -> Modifiers {
    let mut parsed = Modifiers::default();
    let Some(text) = modifiers else {
        return parsed;
    };
    for part in text.split('-') {
        match part.trim().to_ascii_lowercase().as_str() {
            "cmd" | "meta" | "super" | "win" | "platform" => parsed.platform = true,
            "ctrl" | "control" => parsed.control = true,
            "alt" | "option" => parsed.alt = true,
            "shift" => parsed.shift = true,
            "fn" | "function" => parsed.function = true,
            _ => {}
        }
    }
    parsed
}

pub fn dispatch_keystrokes(
    window: &mut Window,
    cx: &mut App,
    keystrokes: &str,
) -> Result<(), String> {
    for keystroke in keystrokes.split(' ') {
        window.dispatch_keystroke(parse_keystroke(keystroke)?, cx);
    }
    Ok(())
}

pub fn dispatch_key_down(
    window: &mut Window,
    cx: &mut App,
    keystroke: &str,
    is_held: bool,
) -> Result<(), String> {
    window.dispatch_event(
        KeyDownEvent {
            keystroke: parse_keystroke(keystroke)?,
            is_held,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
    Ok(())
}

pub fn dispatch_key_up(window: &mut Window, cx: &mut App, keystroke: &str) -> Result<(), String> {
    window.dispatch_event(
        KeyUpEvent {
            keystroke: parse_keystroke(keystroke)?,
        }
        .to_platform_input(),
        cx,
    );
    Ok(())
}

fn parse_keystroke(keystroke: &str) -> Result<Keystroke, String> {
    Keystroke::parse(keystroke).map_err(|error| format!("Invalid keystroke '{keystroke}': {error}"))
}

/// Every automation mouse dispatcher takes modifiers, so a test can drive
/// cmd-wheel zoom, shift-click range selection, or alt-drag duplication.
pub fn dispatch_click(
    window: &mut Window,
    cx: &mut App,
    x: f64,
    y: f64,
    button: u32,
    modifiers: Modifiers,
) {
    let position = point(px(x as f32), px(y as f32));
    let button = mouse_button(button);
    window.dispatch_event(
        MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
}

pub fn dispatch_mouse_down(
    window: &mut Window,
    cx: &mut App,
    x: f64,
    y: f64,
    button: u32,
    modifiers: Modifiers,
) {
    window.dispatch_event(
        MouseDownEvent {
            button: mouse_button(button),
            position: point(px(x as f32), px(y as f32)),
            modifiers,
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
}

pub fn dispatch_mouse_up(
    window: &mut Window,
    cx: &mut App,
    x: f64,
    y: f64,
    button: u32,
    modifiers: Modifiers,
) {
    window.dispatch_event(
        MouseUpEvent {
            button: mouse_button(button),
            position: point(px(x as f32), px(y as f32)),
            modifiers,
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
}

pub fn dispatch_mouse_move(
    window: &mut Window,
    cx: &mut App,
    x: f64,
    y: f64,
    pressed_button: Option<u32>,
    modifiers: Modifiers,
) {
    window.dispatch_event(
        MouseMoveEvent {
            position: point(px(x as f32), px(y as f32)),
            pressed_button: pressed_button.map(mouse_button),
            modifiers,
        }
        .to_platform_input(),
        cx,
    );
}

pub fn dispatch_scroll_wheel(
    window: &mut Window,
    cx: &mut App,
    x: f64,
    y: f64,
    delta_x: f64,
    delta_y: f64,
    modifiers: Modifiers,
) {
    window.dispatch_event(
        gpui::ScrollWheelEvent {
            position: point(px(x as f32), px(y as f32)),
            delta: gpui::ScrollDelta::Pixels(point(px(delta_x as f32), px(delta_y as f32))),
            modifiers,
            touch_phase: gpui::TouchPhase::Moved,
        }
        .to_platform_input(),
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_clock_holds_and_fast_forwards() {
        let clock = AutomationClock::new();
        clock.set_ms(0.0);
        assert!((clock.now_ms() - 0.0).abs() < 0.001);
        clock.fast_forward_ms(150.0);
        assert!((clock.now_ms() - 150.0).abs() < 0.001);
        let later = clock.now();
        clock.fast_forward_ms(150.0);
        assert_eq!(
            clock.now().saturating_duration_since(later),
            Duration::from_millis(150)
        );
    }

    #[test]
    fn playback_rate_and_seek_preserve_timeline_state() {
        let clock = AutomationClock::new();
        clock.pause();
        clock.seek_ms(250.0);
        clock.set_playback_rate(2.0);
        let frozen = clock.snapshot();
        assert_eq!(frozen.current_time_ms, 250.0);
        assert_eq!(frozen.playback_rate, 2.0);
        assert!(!frozen.playing);
        clock.resume();
        assert!(clock.snapshot().playing);
    }
}
