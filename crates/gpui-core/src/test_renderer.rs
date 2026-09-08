// N-API requires owned arguments and instance methods; GPUI test state is thread-local.
// JS numbers are f64, while GPUI coordinates are f32. IDs are validated before use.
#![allow(
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use parking_lot::Mutex;
/// `TestGpuiRenderer` — GPU-backed GPUI test renderer exposed to Node.js via napi.
///
/// Uses `gpui::VisualTestAppContext` with the native Metal or DirectX renderer
/// and `TestDispatcher` for deterministic scheduling. Runs the SAME `GpuiView`,
/// `build_element()`, `apply_styles()`, and event handlers as production.
///
/// Windows are positioned offscreen at (-10000, -10000) — invisible but
/// fully rendered by the native GPU. This enables `capture_screenshot()` for visual
/// test validation.
///
/// `VisualTestAppContext` is !Send, so it is stored in thread-local state.
/// All napi calls happen on the JS main thread.
use std::cell::RefCell;
use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi_derive::napi;

use gpui::AppContext as _;

use crate::element_tree::EventPayload;
use crate::renderer::{
    DebugFrameOverlayStats, EventCallback, GpuiView, apply_parsed_batch_to_tree,
    debug_frame_overlay_mode_name, debug_frame_overlay_stats_js, parse_batch_ops,
    parse_debug_frame_overlay_mode, to_element_id,
};
use crate::retained_tree::RetainedTree;

// ── Thread-local storage for !Send GPUI types ────────────────────────

/// Bundles `VisualTestAppContext` + window handle + view entity.
/// Stored in `thread_local` because `VisualTestAppContext` is !Send (Rc<AppCell>).
/// Field order is load-bearing: Rust drops fields in declaration order, and
/// gpui panics at app teardown if an `Entity` handle outlives its `App`.
/// `view` must therefore be declared before `cx`.
struct VisualTestState {
    view: gpui::Entity<GpuiView>,
    window: gpui::AnyWindowHandle,
    cx: gpui::VisualTestAppContext,
}

/// Release every `Entity` handle the view is holding, while the `App` is alive.
///
/// The test build enables gpui's leak detector, which panics if a handle
/// outlives its `App`. `<input>` keeps an `Entity<TextEditorState>` in the
/// view's custom element registry, so that panic fires from a thread-local
/// destructor at process exit. macOS never runs this destructor, so the panic
/// only appeared once Windows started running the suite: every test file
/// passed and then the vitest worker died with "Worker exited unexpectedly".
///
/// `drop` runs before the fields are dropped, so `view` and `cx` are both
/// still usable here.
impl Drop for VisualTestState {
    fn drop(&mut self) {
        let view = self.view.clone();
        // Unmount, exactly as Vue would: empty the tree, then paint one more
        // frame. The registry is not the only owner of the entity. `<input>`
        // installs an `ElementInputHandler` during paint, and a clone of that
        // lives in the window's rendered frame and in the platform window. A
        // frame with nothing in it is what drops those, and it has to happen
        // while the `App` is still alive.
        self.cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.tree.lock().root_id = None;
                view.custom_registry.destroy_all();
                view.focus_subscriptions.clear();
                view.focus_handles.clear();
                cx.notify();
            });
        });
        // Err only means the window is already gone, which is the state this
        // is trying to reach.
        self.cx
            .update_window(self.window, |_, window, _| window.refresh())
            .ok();
        self.cx.run_until_parked();
    }
}

thread_local! {
    static TEST_STATE: RefCell<Option<VisualTestState>> = const { RefCell::new(None) };
}

/// Access `VisualTestAppContext` + window + view mutably within `thread_local`.
/// The closure receives (&mut cx, `window_handle`, &`view_entity`).
/// Returns Err if no `TestGpuiRenderer` has been created on this thread.
fn with_test_state<R>(
    f: impl FnOnce(
        &mut gpui::VisualTestAppContext,
        gpui::AnyWindowHandle,
        &gpui::Entity<GpuiView>,
    ) -> Result<R>,
) -> Result<R> {
    TEST_STATE.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let state = borrow
            .as_mut()
            .ok_or_else(|| Error::from_reason("TestGpuiRenderer not initialized"))?;
        f(&mut state.cx, state.window, &state.view)
    })
}

/// Default offscreen window size. Matches gpui's `open_offscreen_window_default`,
/// so a `new TestGpuiRenderer()` with no size behaves exactly as before.
///
/// Note for layout tests: 1280 is wide enough that a centered max-width content
/// column stays capped whether a sidebar is open or closed. A test that needs to
/// observe re-wrapping must pass a narrower width explicitly.
const DEFAULT_WINDOW_WIDTH: f64 = 1280.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;

/// Validate a caller-supplied window dimension, falling back to `default`.
///
/// Checks the value *after* the `f32` cast: a finite `f64` such as `1e300`
/// saturates to `f32::INFINITY`, which would open a window with no usable size.
fn window_dimension(value: Option<f64>, default: f64, label: &str) -> Result<f32> {
    let Some(value) = value else {
        return Ok(default as f32);
    };
    let pixels = value as f32;
    if !pixels.is_finite() || pixels <= 0.0 {
        return Err(Error::from_reason(format!(
            "TestGpuiRenderer {label} must be a positive, finite number, got {value}"
        )));
    }
    Ok(pixels)
}

/// Convert JS button number (0=left, 1=middle, 2=right) to GPUI `MouseButton`.
fn u32_to_mouse_button(button: u32) -> gpui::MouseButton {
    match button {
        1 => gpui::MouseButton::Middle,
        2 => gpui::MouseButton::Right,
        _ => gpui::MouseButton::Left,
    }
}

// ── TestGpuiRenderer ────────────────────────────────────────────────

/// GPU-backed GPUI test renderer. Uses `VisualTestAppContext` with the native
/// Metal or DirectX renderer and `TestDispatcher` for deterministic scheduling.
/// Same `GpuiView` and rendering pipeline as production.
///
/// Usage from JS:
/// ```javascript
/// const r = new TestGpuiRenderer()
/// r.createElement(1, "div")
/// r.setRoot(1)
/// r.commitMutations()
/// r.flush()                  // paints the retained view on the GPU
/// r.simulateClick(50, 50)    // dispatches through GPUI hit testing
/// const events = r.drainEvents()
/// r.captureScreenshot("/tmp/test.png")
/// ```
#[napi]
pub struct TestGpuiRenderer {
    tree: Arc<Mutex<RetainedTree>>,
    events: Arc<Mutex<Vec<EventPayload>>>,
    /// Same handle `GpuiView` paints against, so tests can assert on the live
    /// selection after simulating a drag.
    selection: crate::text::SharedSelection,
}

#[napi]
impl TestGpuiRenderer {
    #[napi(constructor)]
    pub fn new(width: Option<f64>, height: Option<f64>) -> Result<Self> {
        let window_size = gpui::size(
            gpui::px(window_dimension(width, DEFAULT_WINDOW_WIDTH, "width")?),
            gpui::px(window_dimension(height, DEFAULT_WINDOW_HEIGHT, "height")?),
        );
        let tree = Arc::new(Mutex::new(RetainedTree::new()));
        let events: Arc<Mutex<Vec<EventPayload>>> = Arc::new(Mutex::new(Vec::new()));

        // Event callback: push to Vec instead of ThreadsafeFunction.
        let events_clone = events.clone();
        let event_callback: Option<EventCallback> = Some(Arc::new(move |payload: EventPayload| {
            events_clone.lock().push(payload);
        }));

        let tree_clone = tree.clone();
        let callback_clone = event_callback.clone();
        let selection = crate::text::SharedSelection::default();
        let selection_clone = selection.clone();
        let clock = crate::automation::AutomationClock::default();

        let platform = gpui_platform::current_platform(false);
        let mut cx = gpui::VisualTestAppContext::new(platform);
        cx.update(|cx| {
            crate::custom_elements::input::init(cx);
        });

        // Open an offscreen window at (-10000, -10000) with the same GpuiView
        // and native GPU renderer as production.
        let window_handle = cx
            .open_offscreen_window(window_size, |_window, app| {
                app.new(|_cx| {
                    GpuiView::new(
                        tree_clone,
                        callback_clone,
                        "GPUI Vue Test".to_string(),
                        selection_clone,
                        clock,
                    )
                })
            })
            .map_err(|e| Error::from_reason(format!("Failed to open test window: {e}")))?;

        // Get the root entity (Entity<GpuiView>) from the window.
        let view = window_handle
            .entity(&cx)
            .map_err(|e| Error::from_reason(format!("Failed to get root view: {e}")))?;

        // Convert typed WindowHandle<GpuiView> to AnyWindowHandle for simulation methods.
        let window: gpui::AnyWindowHandle = window_handle.into();

        cx.update_window(window, |_, window, _| {
            window.set_a11y_active_for_tests(true);
        })
        .map_err(|error| Error::from_reason(error.to_string()))?;
        cx.run_until_parked();

        // Store !Send types on the JS main thread.
        TEST_STATE.with(|cell| {
            *cell.borrow_mut() = Some(VisualTestState { view, window, cx });
        });

        Ok(Self {
            tree,
            events,
            selection,
        })
    }

    // ── Mutation API (same interface as GpuiRenderer) ────────────────

    #[napi]
    pub fn create_element(&self, id: f64, element_type: String) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree.lock().create_element(id, element_type);
        Ok(())
    }

    /// Destroy an element and all descendants. Returns destroyed IDs
    /// so JS can clean up event handlers.
    #[napi]
    pub fn destroy_element(&self, id: f64) -> Result<Vec<f64>> {
        let id = to_element_id(id)?;
        let destroyed = self.tree.lock().destroy_element(id);
        Ok(destroyed.iter().map(|&id| id as f64).collect())
    }

    #[napi]
    pub fn append_child(&self, parent_id: f64, child_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        self.tree
            .lock()
            .append_child(parent_id, child_id)
            .map_err(Error::from_reason)
    }

    #[napi]
    pub fn remove_child(&self, parent_id: f64, child_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        self.tree.lock().remove_child(parent_id, child_id);
        Ok(())
    }

    #[napi]
    pub fn insert_before(&self, parent_id: f64, child_id: f64, before_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let before_id = to_element_id(before_id)?;
        self.tree
            .lock()
            .insert_before(parent_id, child_id, before_id)
            .map_err(Error::from_reason)
    }

    #[napi]
    pub fn set_style(&self, id: f64, style_json: String) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree
            .lock()
            .set_style_json(id, style_json.as_bytes())
            .map_err(|error| Error::from_reason(format!("Failed to parse style: {error}")))
    }

    #[napi]
    pub fn set_text(&self, id: f64, content: String) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree.lock().set_text(id, content);
        Ok(())
    }

    #[napi]
    pub fn set_event_listener(&self, id: f64, event_type: String, has_handler: bool) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree
            .lock()
            .set_event_listener(id, event_type, has_handler);
        Ok(())
    }

    /// Set the root element (called from appendChildToContainer).
    #[napi]
    pub fn set_root(&self, id: f64) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree.lock().set_root(id);
        Ok(())
    }

    /// Set a custom prop on an element (for non-div/text elements like input, editor, diff).
    #[napi]
    pub fn set_custom_prop(&self, id: f64, key: String, value_json: String) -> Result<()> {
        let id = to_element_id(id)?;
        let value: serde_json::Value = serde_json::from_str(&value_json)
            .map_err(|e| Error::from_reason(format!("Failed to parse custom prop value: {e}")))?;
        self.tree.lock().set_custom_prop(id, key, value);
        Ok(())
    }

    /// Get a custom prop value from an element.
    #[napi]
    pub fn get_custom_prop(&self, id: f64, key: String) -> Result<Option<String>> {
        let id = to_element_id(id)?;
        let tree = self.tree.lock();
        Ok(tree
            .get_custom_prop(id, &key)
            .map(|v| serde_json::to_string(v).unwrap_or_default()))
    }

    #[napi]
    pub fn create_canvas_source(&self) -> Result<u32> {
        self.tree
            .lock()
            .canvas_frames
            .lock()
            .create()
            .map_err(Error::from_reason)
    }

    #[napi]
    #[allow(
        clippy::too_many_arguments,
        reason = "flat binary frame signature shared by N-API and Wasm avoids JSON metadata in the presentation path"
    )]
    pub fn present_canvas_frame(
        &self,
        id: u32,
        width: u32,
        height: u32,
        stride: u32,
        pixels: Uint8Array,
        bgra: bool,
        opaque: bool,
    ) -> Result<()> {
        let image =
            crate::gpu_canvas::decode_frame(width, height, stride, pixels.as_ref(), bgra, opaque)
                .map_err(Error::from_reason)?;
        let frames = self.tree.lock().canvas_frames.clone();
        frames
            .lock()
            .publish_image(id, image)
            .map_err(Error::from_reason)
    }

    #[napi]
    pub fn destroy_canvas_source(&self, id: u32) {
        self.tree.lock().canvas_frames.lock().destroy(id);
    }

    /// Signal that a batch of mutations is complete.
    /// In tests, this is a no-op — `flush()` handles the actual re-render.
    #[napi]
    pub fn commit_mutations(&self) -> Result<()> {
        Ok(())
    }

    /// Apply a batch of mutations in a single FFI call.
    /// Same format as `GpuiRenderer::apply_batch` (string op names).
    /// Returns accumulated destroyed IDs from all destroyElement ops.
    #[napi]
    pub fn apply_batch(&self, json: String) -> Result<Vec<f64>> {
        let parsed = parse_batch_ops(json.as_bytes()).map_err(Error::from_reason)?;
        let mut tree = self.tree.lock();
        apply_parsed_batch_to_tree(&mut tree, parsed).map_err(Error::from_reason)
    }

    // ── Test-specific methods ────────────────────────────────────────

    /// Notify the view entity and run GPUI until parked.
    /// This triggers `GpuiView::render()` → `build_element()` → GPUI layout.
    /// Must be called after mutations and before simulating events (GPUI's
    /// hit testing requires elements to be laid out).
    #[napi]
    pub fn flush(&self) -> Result<()> {
        with_test_state(|cx, window, view| {
            let view = view.clone();
            cx.update_window(window, |_, _window, app| {
                view.update(app, |_, cx| {
                    cx.notify();
                });
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;

            cx.run_until_parked();
            Ok(())
        })
    }

    /// Simulate a click at the given window coordinates.
    /// Dispatches `MouseDown` + `MouseUp` through GPUI's input pipeline,
    /// which triggers the same event handlers as production.
    /// IMPORTANT: Call `flush()` before this — hit testing requires laid-out elements.
    /// `modifiers` uses the `press()` syntax: "cmd", "cmd-shift", "alt".
    #[napi]
    pub fn simulate_click(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());
        let button = button.unwrap_or(0);
        with_test_state(|cx, window, _view| {
            // Not `cx.simulate_click`: that helper hard-codes the left button,
            // so a right click silently became a left click.
            let position = gpui::point(gpui::px(x as f32), gpui::px(y as f32));
            let gpui_button = u32_to_mouse_button(button);
            cx.simulate_event(
                window,
                gpui::MouseDownEvent {
                    position,
                    modifiers,
                    button: gpui_button,
                    click_count: 1,
                    first_mouse: false,
                },
            );
            cx.simulate_event(
                window,
                gpui::MouseUpEvent {
                    position,
                    modifiers,
                    button: gpui_button,
                    click_count: 1,
                },
            );
            Ok(())
        })
    }

    /// Simulate key strokes through GPUI's input pipeline.
    /// Format: space-separated keys, e.g. "a", "enter", "cmd-shift-p".
    /// The focused element receives keyDown/keyUp events.
    #[napi]
    pub fn simulate_keystrokes(&self, keystrokes: String) -> Result<()> {
        with_test_state(|cx, window, _view| {
            cx.simulate_keystrokes(window, &keystrokes);
            Ok(())
        })
    }

    /// Simulate a single key down event through GPUI's input pipeline.
    /// Format: modifier-key string, e.g. "a", "enter", "cmd-s".
    /// Unlike `simulate_keystrokes`, this dispatches ONLY a `KeyDownEvent` —
    /// no automatic `KeyUpEvent` follows. Use with `simulate_key_up` for
    /// fine-grained key event testing.
    #[napi]
    pub fn simulate_key_down(&self, keystroke: String, is_held: Option<bool>) -> Result<()> {
        with_test_state(|cx, window, _view| {
            let parsed = gpui::Keystroke::parse(&keystroke)
                .map_err(|e| Error::from_reason(format!("Invalid keystroke '{keystroke}': {e}")))?;

            cx.simulate_event(
                window,
                gpui::KeyDownEvent {
                    keystroke: parsed,
                    is_held: is_held.unwrap_or(false),
                    prefer_character_input: false,
                },
            );

            Ok(())
        })
    }

    /// Simulate a single key up event through GPUI's input pipeline.
    /// Format: modifier-key string, e.g. "a", "enter", "cmd-s".
    /// Pairs with `simulate_key_down` for fine-grained key event testing.
    #[napi]
    pub fn simulate_key_up(&self, keystroke: String) -> Result<()> {
        with_test_state(|cx, window, _view| {
            let parsed = gpui::Keystroke::parse(&keystroke)
                .map_err(|e| Error::from_reason(format!("Invalid keystroke '{keystroke}': {e}")))?;

            cx.simulate_event(window, gpui::KeyUpEvent { keystroke: parsed });

            Ok(())
        })
    }

    /// Simulate a mouse move to the given coordinates.
    /// `pressed_button`: optional mouse button held during move (0=left, 1=middle, 2=right).
    /// Used to simulate drag events.
    #[napi]
    pub fn simulate_mouse_move(
        &self,
        x: f64,
        y: f64,
        pressed_button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());
        with_test_state(|cx, window, _view| {
            let button: Option<gpui::MouseButton> = pressed_button.map(u32_to_mouse_button);

            cx.simulate_mouse_move(
                window,
                gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                button,
                modifiers,
            );

            Ok(())
        })
    }

    /// Focus an element by its numeric ID.
    /// The element must have a `FocusHandle` (created by `sync_focus_handles` when
    /// the element has keyDown, keyUp, focus, or blur listeners).
    /// Call `flush()` before this so the element tree and focus handles exist.
    #[napi]
    pub fn focus_element(&self, id: f64) -> Result<()> {
        let id = to_element_id(id)?;

        with_test_state(|cx, window, view| {
            let view = view.clone();

            cx.update_window(window, |_, window, app| {
                view.update(app, |view, cx| {
                    view.reveal_virtual_list_ancestor(id);
                    if let Some(handle) = view.focus_handles.get(&id) {
                        handle.focus(window, cx);
                    }
                    cx.notify();
                });
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;

            cx.run_until_parked();
            Ok(())
        })
    }

    /// Simulate a mouse down event at the given window coordinates.
    /// Button: 0=left, 1=middle, 2=right. Defaults to left (0).
    #[napi]
    pub fn simulate_mouse_down(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());
        with_test_state(|cx, window, _view| {
            cx.simulate_mouse_down(
                window,
                gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                u32_to_mouse_button(button.unwrap_or(0)),
                modifiers,
            );
            Ok(())
        })
    }

    /// Simulate a mouse up event at the given window coordinates.
    /// Button: 0=left, 1=middle, 2=right. Defaults to left (0).
    #[napi]
    pub fn simulate_mouse_up(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());
        with_test_state(|cx, window, _view| {
            cx.simulate_mouse_up(
                window,
                gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                u32_to_mouse_button(button.unwrap_or(0)),
                modifiers,
            );
            Ok(())
        })
    }

    /// Simulate a scroll wheel event at the given position.
    /// `delta_x` and `delta_y` are in pixels (negative = scroll up/left).
    #[napi]
    pub fn simulate_scroll_wheel(
        &self,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());
        with_test_state(|cx, window, _view| {
            cx.simulate_event(
                window,
                gpui::ScrollWheelEvent {
                    position: gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(
                        gpui::px(delta_x as f32),
                        gpui::px(delta_y as f32),
                    )),
                    modifiers,
                    touch_phase: gpui::TouchPhase::Moved,
                },
            );
            Ok(())
        })
    }

    // ── Selection API ──────────────────────────────────────────────────

    /// The current text selection joined in document order, or null.
    #[napi]
    pub fn get_selected_text(&self) -> Option<String> {
        self.selection.lock().selected_text()
    }

    /// Drop the current selection.
    #[napi]
    pub fn clear_selection(&self) {
        self.selection.lock().clear();
    }

    /// Syntax-cache counters as `[hits, misses, documents]`.
    ///
    /// GPUIX rebuilds its whole element tree every frame, so a `<code>` block
    /// that misses the cache reparses at frame rate. A test can watch the hit
    /// count to catch that regression before a profiler does.
    #[napi]
    pub fn get_syntax_cache_stats(&self) -> Vec<f64> {
        let stats = crate::syntax::cache::stats();
        vec![
            stats.hits as f64,
            stats.misses as f64,
            stats.documents as f64,
        ]
    }

    /// Every string painted in the last frame, in paint order.
    ///
    /// `getAllText()` only sees `<text>` nodes in the retained tree. Native
    /// elements such as `<code>` and `<diff>` draw their text inside gpui, so
    /// this is the only way to assert on what they actually rendered.
    #[napi]
    pub fn get_painted_text(&self) -> Result<Vec<String>> {
        self.flush()?;
        Ok(crate::text::painted_text())
    }

    /// Every highlight wash painted in the last frame, in paint order.
    ///
    /// A quad is invisible to `getPaintedText()`, so this is the only way to
    /// assert on `highlight` without a screenshot. Each entry carries its rects,
    /// so a soft-wrapped match is provably two boxes.
    #[napi]
    pub fn get_painted_highlights(&self) -> Result<Vec<crate::element_tree::HighlightMatch>> {
        self.flush()?;
        Ok(crate::text::painted_highlights()
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Drag-select from one point to another: mouse down, move, up.
    ///
    /// A single helper rather than three calls because the listeners that drive
    /// selection are registered during **paint**, so a flush must sit between
    /// the down and the move. Getting that order wrong silently selects nothing,
    /// which is a miserable thing to debug from JS.
    #[napi]
    pub fn drag_select(&self, x1: f64, y1: f64, x2: f64, y2: f64) -> Result<()> {
        self.flush()?;
        self.simulate_mouse_down(x1, y1, None, None)?;
        self.flush()?;
        self.simulate_mouse_move(x2, y2, Some(0), None)?;
        self.flush()?;
        self.simulate_mouse_up(x2, y2, None, None)?;
        self.flush()?;
        Ok(())
    }

    // ── Scroll API ─────────────────────────────────────────────────────

    /// Set the scroll offset of a scrollable element.
    /// x and y are negative pixel values (scroll down = more negative y).
    /// Call `flush()` after to apply the offset and re-render.
    #[napi]
    pub fn scroll_to(&self, element_id: f64, x: f64, y: f64) -> Result<()> {
        let id = to_element_id(element_id)?;
        with_test_state(|cx, window, view| {
            let view = view.clone();
            cx.update_window(window, |_, _window, app| {
                view.update(app, |view, _cx| {
                    if view.set_virtual_list_offset(id, x as f32, y as f32) {
                        return;
                    }
                    if let Some(handle) = view.scroll_handles.get(&id) {
                        handle.set_offset(gpui::point(gpui::px(x as f32), gpui::px(y as f32)));
                    }
                });
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;
            Ok(())
        })
    }

    /// Scroll a child into view by its index in the children list.
    /// Call `flush()` after to apply and re-render.
    #[napi]
    pub fn scroll_to_item(
        &self,
        element_id: f64,
        index: f64,
        offset_in_item: Option<f64>,
    ) -> Result<()> {
        let id = to_element_id(element_id)?;
        let index = index as usize;
        let offset = offset_in_item.unwrap_or(0.0) as f32;
        with_test_state(|cx, window, view| {
            let view = view.clone();
            cx.update_window(window, |_, _window, app| {
                view.update(app, |view, _cx| {
                    if view.scroll_virtual_list_to_item(id, index, offset) {
                        return;
                    }
                    if let Some(handle) = view.scroll_handles.get(&id) {
                        handle.scroll_to_item(index);
                    }
                });
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;
            Ok(())
        })
    }

    /// Return `[itemIndex, offsetInItemPx, viewportHeightPx]` for a virtual
    /// list, or null for an ordinary element.
    #[napi]
    pub fn get_list_scroll_top(&self, element_id: f64) -> Result<Option<Vec<f64>>> {
        let id = to_element_id(element_id)?;
        with_test_state(|cx, window, view| {
            let view = view.clone();
            cx.update_window(window, |_, _window, app| {
                view.update(app, |view, _cx| {
                    view.virtual_list_scroll_top(id).map(|top| top.to_vec())
                })
            })
            .map_err(|error| Error::from_reason(error.to_string()))
        })
    }

    /// `"hidden"` | `"minimal"` | `"full"`.
    #[napi]
    pub fn set_debug_frame_overlay(&self, mode: String) -> Result<String> {
        let mode = parse_debug_frame_overlay_mode(&mode)?;
        with_test_state(|cx, window, _view| {
            cx.update_window(window, |_, window, _app| {
                window.set_debug_frame_overlay_mode(mode);
                debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
        })
    }

    /// Hidden → minimal → full → hidden.
    #[napi]
    pub fn cycle_debug_frame_overlay(&self) -> Result<String> {
        with_test_state(|cx, window, _view| {
            cx.update_window(window, |_, window, _app| {
                window.cycle_debug_frame_overlay_mode();
                debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
        })
    }

    #[napi]
    pub fn get_debug_frame_overlay(&self) -> Result<String> {
        with_test_state(|cx, window, _view| {
            cx.update_window(window, |_, window, _app| {
                debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
        })
    }

    /// Clears the last 1000 draw samples. Frame count stays.
    #[napi]
    pub fn reset_debug_frame_overlay_stats(&self) -> Result<()> {
        with_test_state(|cx, window, _view| {
            cx.update_window(window, |_, window, _app| {
                window.reset_debug_frame_overlay_stats();
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;
            Ok(())
        })
    }

    /// Same numbers as the on-screen overlay: current, p90, p99, max, frames.
    #[napi]
    pub fn get_debug_frame_overlay_stats(&self) -> Result<DebugFrameOverlayStats> {
        with_test_state(|cx, window, _view| {
            cx.update_window(window, |_, window, _app| {
                debug_frame_overlay_stats_js(window.debug_frame_overlay_stats())
            })
            .map_err(|e| Error::from_reason(e.to_string()))
        })
    }

    /// Get the current scroll offset of a scrollable element.
    /// Returns [x, y] or null if the element has no scroll handle.
    #[napi]
    pub fn get_scroll_offset(&self, element_id: f64) -> Result<Option<Vec<f64>>> {
        let id = to_element_id(element_id)?;
        with_test_state(|cx, window, view| {
            let view = view.clone();
            let result = cx
                .update_window(window, |_, _window, app| {
                    view.update(app, |view, _cx| {
                        if let Some(offset) = view.virtual_list_offset(id) {
                            return Some(offset.to_vec());
                        }
                        view.scroll_handles.get(&id).map(|handle| {
                            let offset = handle.offset();
                            vec![
                                f64::from(f32::from(offset.x)),
                                f64::from(f32::from(offset.y)),
                            ]
                        })
                    })
                })
                .map_err(|e| Error::from_reason(e.to_string()))?;
            Ok(result)
        })
    }

    /// Capture a screenshot of the current rendered state and save as PNG.
    /// Supported on macOS through Metal and Windows through DirectX.
    #[napi]
    pub fn capture_screenshot(&self, path: String) -> Result<()> {
        with_test_state(|cx, window, view| {
            let view = view.clone();

            // Flush: notify view and run until parked so layout/rendering are current.
            cx.update_window(window, |_, _window, app| {
                view.update(app, |_, cx| {
                    cx.notify();
                });
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;

            // Force a window refresh before capture so render_to_image reads
            // the most recent frame scene.
            cx.update_window(window, |_, window, _app| {
                window.refresh();
            })
            .map_err(|e| Error::from_reason(e.to_string()))?;

            cx.run_until_parked();

            // Capture via the platform renderer's render_to_image implementation.
            let image = cx
                .capture_screenshot(window)
                .map_err(|e| Error::from_reason(format!("Screenshot capture failed: {e}")))?;

            // Save as PNG (format inferred from file extension).
            image
                .save(&path)
                .map_err(|e| Error::from_reason(format!("Failed to save screenshot: {e}")))?;

            Ok(())
        })
    }

    /// Return and clear all collected events since the last drain.
    /// Events are collected synchronously — no event loop queuing.
    #[napi]
    pub fn drain_events(&self) -> Vec<EventPayload> {
        let mut events = self.events.lock();
        events.drain(..).collect()
    }

    // ── Tree inspection ──────────────────────────────────────────────

    /// Get all text content in the tree (depth-first order).
    #[napi]
    pub fn get_all_text(&self) -> Vec<String> {
        let tree = self.tree.lock();
        let mut texts = Vec::new();
        if let Some(root_id) = tree.root_id {
            Self::collect_text(root_id, &tree, &mut texts);
        }
        texts
    }

    /// Count every retained native node, including detached nodes that are not
    /// visible from a root-tree snapshot.
    #[napi]
    pub fn get_retained_element_count(&self) -> u32 {
        u32::try_from(self.tree.lock().elements.len()).unwrap_or(u32::MAX)
    }

    /// Find element IDs matching the given type (e.g. "div", "text").
    #[napi]
    pub fn find_by_type(&self, element_type: String) -> Vec<f64> {
        let tree = self.tree.lock();
        tree.elements
            .values()
            .filter(|e| e.element_type == element_type)
            .map(|e| e.id as f64)
            .collect()
    }

    /// Check if an element has a specific event listener.
    #[napi]
    pub fn has_event_listener(&self, id: f64, event_type: String) -> Result<bool> {
        let id = to_element_id(id)?;
        let tree = self.tree.lock();
        Ok(tree
            .elements
            .get(&id)
            .is_some_and(|e| e.events.contains(&event_type)))
    }

    /// Get the text content of an element.
    #[napi]
    pub fn get_text(&self, id: f64) -> Result<Option<String>> {
        let id = to_element_id(id)?;
        let tree = self.tree.lock();
        Ok(tree
            .elements
            .get(&id)
            .and_then(|e| e.content.as_ref().map(ToString::to_string)))
    }

    /// Get the full tree as JSON for snapshot testing.
    #[napi]
    pub fn get_tree_json(&self) -> Result<String> {
        let tree = self.tree.lock();
        let json = tree.to_json(&std::collections::HashMap::new());
        serde_json::to_string_pretty(&json)
            .map_err(|e| Error::from_reason(format!("JSON serialization failed: {e}")))
    }

    /// Tree JSON with last-paint bounds. Used by the automation locators.
    /// GPUI accessibility tree from the last painted frame.
    #[napi(js_name = "advanceTime")]
    pub fn advance_time(&self, milliseconds: f64) -> Result<()> {
        let duration =
            std::time::Duration::try_from_secs_f64(milliseconds / 1000.0).map_err(|_| {
                Error::from_reason("advanceTime requires a finite non-negative duration")
            })?;
        with_test_state(|cx, _window, _view| {
            cx.advance_clock(duration);
            cx.run_until_parked();
            Ok(())
        })
    }

    #[napi(js_name = "getA11yTree")]
    pub fn get_a11y_tree(&self) -> Result<String> {
        self.flush()?;
        with_test_state(|cx, window, _view| {
            cx.update_window(window, |_, window, _| {
                window
                    .debug_a11y_tree_json()
                    .unwrap_or_else(|| "{}".to_owned())
            })
            .map_err(|error| Error::from_reason(error.to_string()))
        })
    }

    #[napi]
    pub fn get_automation_tree(&self) -> Result<String> {
        self.flush()?;
        let tree = self.tree.lock();
        let json = tree.to_automation_json(&crate::automation::all_bounds());
        serde_json::to_string(&json)
            .map_err(|e| Error::from_reason(format!("JSON serialization failed: {e}")))
    }

    /// Last painted bounds for an element, or null if it was not painted.
    #[napi]
    pub fn get_element_bounds(&self, id: f64) -> Result<Option<Vec<f64>>> {
        let id = to_element_id(id)?;
        self.flush()?;
        Ok(crate::automation::get_bounds(id)
            .map(|bounds| vec![bounds.x, bounds.y, bounds.width, bounds.height]))
    }

    #[napi]
    pub fn clock_pause(&self) -> Result<f64> {
        with_test_state(|cx, window, view| {
            let view = view.clone();
            let now_ms = cx
                .update_window(window, |_, _window, app| {
                    view.update(app, |view, cx| {
                        let now_ms = view.clock.pause();
                        cx.notify();
                        now_ms
                    })
                })
                .map_err(|e| Error::from_reason(e.to_string()))?;
            cx.run_until_parked();
            Ok(now_ms)
        })
    }

    #[napi]
    pub fn clock_set(&self, now_ms: f64) -> Result<f64> {
        with_test_state(|cx, window, view| {
            let view = view.clone();
            let now_ms = cx
                .update_window(window, |_, _window, app| {
                    view.update(app, |view, cx| {
                        let now_ms = view.clock.set_ms(now_ms);
                        cx.notify();
                        now_ms
                    })
                })
                .map_err(|e| Error::from_reason(e.to_string()))?;
            cx.run_until_parked();
            Ok(now_ms)
        })
    }

    #[napi]
    pub fn clock_fast_forward(&self, delta_ms: f64) -> Result<f64> {
        with_test_state(|cx, window, view| {
            let view = view.clone();
            let now_ms = cx
                .update_window(window, |_, _window, app| {
                    view.update(app, |view, cx| {
                        let now_ms = view.clock.fast_forward_ms(delta_ms);
                        cx.notify();
                        now_ms
                    })
                })
                .map_err(|e| Error::from_reason(e.to_string()))?;
            cx.run_until_parked();
            Ok(now_ms)
        })
    }

    #[napi]
    pub fn clock_resume(&self) -> Result<f64> {
        with_test_state(|cx, window, view| {
            let view = view.clone();
            let now_ms = cx
                .update_window(window, |_, _window, app| {
                    view.update(app, |view, cx| {
                        let now_ms = view.clock.resume();
                        cx.notify();
                        now_ms
                    })
                })
                .map_err(|e| Error::from_reason(e.to_string()))?;
            cx.run_until_parked();
            Ok(now_ms)
        })
    }

    /// Get the root element ID, or null if no root is set.
    #[napi]
    pub fn get_root_id(&self) -> Option<f64> {
        self.tree.lock().root_id.map(|id| id as f64)
    }

    /// The offscreen window size, so `useWindowSize()` reports the same numbers
    /// under test as in a real window instead of falling back to a default.
    #[napi]
    pub fn get_window_size(&self) -> Result<crate::renderer::WindowSize> {
        with_test_state(|cx, window, _view| {
            let size = cx
                .update_window(window, |_, window, _| window.viewport_size())
                .map_err(|error| Error::from_reason(error.to_string()))?;
            Ok(crate::renderer::WindowSize {
                width: f64::from(f32::from(size.width)),
                height: f64::from(f32::from(size.height)),
            })
        })
    }

    // ── Private helpers ──────────────────────────────────────────────

    fn collect_text(id: u64, tree: &RetainedTree, texts: &mut Vec<String>) {
        if let Some(element) = tree.elements.get(&id) {
            if let Some(ref content) = element.content {
                texts.push(content.to_string());
            }
            for &child_id in &element.children {
                Self::collect_text(child_id, tree, texts);
            }
        }
    }
}
