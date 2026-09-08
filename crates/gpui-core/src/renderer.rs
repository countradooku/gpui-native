//! GPUIX retained renderer for napi desktop hosts and GPUI's browser platform.
//!
//! Mutation-based API: Vue's reconciler sends individual mutations
//! (createElement, appendChild, setStyle, etc.) instead of a full JSON tree.
//! Rust maintains a `RetainedTree` and rebuilds GPUI elements from it each frame.
//!
//! Desktop lifecycle:
//!   const renderer = new GpuiRenderer(eventCallback)
//!   renderer.init({ title: 'My App', width: 800, height: 600 })
//!   renderer.createElement(1, "div")     // mutations from Vue reconciler
//!   renderer.appendChild(0, 1)
//!   `renderer.commitMutations()`           // signal batch complete
//!   setTimeout(function `loop()` {         // drive `AppKit` on macOS
//!     if (!`renderer.tick()`) process.exit(0)
//!     setTimeout(loop, 8)
//!   })
// N-API fixes public parameter ownership and exposes Result-returning methods to
// JavaScript rather than Rustdoc consumers. JavaScript Number and GPUI geometry
// also require deliberate narrowing at this bridge boundary.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    reason = "intentional N-API/JavaScript/GPUI boundary representation"
)]
mod api;
mod backend;
mod batch;
mod build;
mod events;
mod styles;
mod virtual_list;
#[cfg(target_family = "wasm")]
mod wasm;

pub use api::GpuiRenderer;

#[cfg(test)]
use batch::{BatchResult, apply_batch_to_tree};
pub(crate) use batch::{apply_parsed_batch_to_tree, parse_batch_ops};
pub(crate) use build::build_element;
use build::unmounted_virtual_row;
pub(crate) use events::{emit_event_full, mouse_button_to_u32, point_to_xy};
pub(crate) use styles::{apply_interactive_styles, apply_styles};
use virtual_list::{VirtualListConfig, VirtualListEntry, window_start_from_element};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use futures::{StreamExt as _, channel::mpsc};
use gpui::AppContext as _;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use napi::bindgen_prelude::*;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use napi_derive::napi;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
#[cfg(any(target_os = "macos", target_family = "wasm"))]
use std::rc::Rc;
use std::sync::Arc;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use std::sync::OnceLock;
#[cfg(target_family = "wasm")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use std::time::Duration;
use web_time::Instant;

use crate::custom_elements::{CustomElementRegistry, CustomRenderContext};
use crate::element_tree::EventPayload;
use crate::retained_tree::RetainedTree;
use crate::style::{DisplayValue, OverflowValue, PointerEventsValue, SelectValue, StyleDesc};
use crate::text::{SharedSelection, selectable_text, selection_frame_reset};
use crate::theme::Theme;

#[cfg(target_family = "wasm")]
#[derive(Debug)]
pub struct Error(String);

#[cfg(target_family = "wasm")]
impl Error {
    pub(crate) fn from_reason(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

#[cfg(target_family = "wasm")]
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(target_family = "wasm")]
impl std::error::Error for Error {}

#[cfg(target_family = "wasm")]
pub(crate) type Result<T> = std::result::Result<T, Error>;

gpui::actions!(gpui_vue_focus, [FocusNext, FocusPrevious]);

pub(crate) fn init_key_bindings(cx: &mut gpui::App) {
    cx.bind_keys([
        gpui::KeyBinding::new("tab", FocusNext, None),
        gpui::KeyBinding::new("shift-tab", FocusPrevious, None),
    ]);
}

/// The Window menu items act on the focused window, and the root element is the
/// only place in GPUIX that has one. `crate::app_menu` owns everything else.
#[cfg(target_os = "macos")]
fn with_window_menu_actions(root: gpui::Div) -> gpui::Div {
    use crate::app_menu::{CloseWindow, MinimizeWindow, ZoomWindow};
    use gpui::prelude::*;

    root.on_action(|_: &MinimizeWindow, window, _cx| window.minimize_window())
        .on_action(|_: &ZoomWindow, window, _cx| window.zoom_window())
        .on_action(|_: &CloseWindow, window, _cx| window.remove_window())
}

#[cfg(not(target_os = "macos"))]
fn with_window_menu_actions(root: gpui::Div) -> gpui::Div {
    root
}

/// Abstracted event callback shared by desktop, browser, and test renderers.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub(crate) type EventCallback = Arc<dyn Fn(EventPayload) + Send + Sync>;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) type EventCallback = Rc<dyn Fn(EventPayload)>;

/// A weak TSFN does not keep Node alive after Vue unmounts the last window.
#[cfg(not(target_family = "wasm"))]
type NativeEventCallback =
    ThreadsafeFunction<EventPayload, Unknown<'static>, EventPayload, Status, true, true>;

#[cfg(target_family = "wasm")]
type NativeEventCallback = EventCallback;

#[cfg(not(target_family = "wasm"))]
fn event_callback_from_native(callback: NativeEventCallback) -> EventCallback {
    Arc::new(move |payload: EventPayload| {
        callback.call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
    })
}

#[cfg(target_family = "wasm")]
fn event_callback_from_native(callback: NativeEventCallback) -> EventCallback {
    callback
}

/// Validate and convert a JS number (f64) to a u64 element ID.
/// JS numbers are f64 — lossless for integers up to 2^53.
fn raw_element_id(id: f64) -> std::result::Result<u64, String> {
    if !id.is_finite() || id < 0.0 || id.fract() != 0.0 || id > 9_007_199_254_740_991.0 {
        return Err(format!("Invalid element id: {id}"));
    }
    Ok(id as u64)
}

pub(crate) fn to_element_id(id: f64) -> Result<u64> {
    raw_element_id(id).map_err(Error::from_reason)
}

thread_local! {
    #[cfg(target_os = "macos")]
    static MAC_PLATFORM: RefCell<Option<Rc<gpui_macos::MacPlatform>>> = const { RefCell::new(None) };
    #[cfg(target_os = "macos")]
    static GPUI_APP: RefCell<Option<gpui::ApplicationHandle>> = const { RefCell::new(None) };
    #[cfg(target_os = "macos")]
    static GPUI_WINDOW: RefCell<Option<gpui::WindowHandle<GpuiView>>> = const { RefCell::new(None) };
    #[cfg(target_family = "wasm")]
    static WEB_APPS: RefCell<HashMap<u64, WebAppEntry>> = RefCell::new(HashMap::new());
    /// Shared scroll handles — GpuiView writes here during render(),
    /// platform-local handlers read from here for programmatic scroll control.
    /// ScrollHandle is Rc<RefCell<...>> so its methods (set_offset, offset,
    /// scroll_to_item) work without an App context.
    ///
    /// NOTE: This is a singleton — if multiple renderers/windows coexist,
    /// the last one to render wins. Acceptable for now (single-window only).
    /// TODO: Scope by renderer/window ID when multi-window support is added.
    static SCROLL_HANDLES: RefCell<HashMap<u64, gpui::ScrollHandle>> = RefCell::new(HashMap::new());
    static VIRTUAL_LIST_STATES: RefCell<HashMap<u64, gpui::ListState>> = RefCell::new(HashMap::new());
    /// Virtual-list scrolls apply after the next retained child splice. Applying
    /// them eagerly would shift an index twice when a page was just prepended.
    static PENDING_VIRTUAL_LIST_SCROLLS: RefCell<HashMap<u64, gpui::ListOffset>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn queue_virtual_list_scroll(id: u64, index: usize, offset_in_item: f32) {
    PENDING_VIRTUAL_LIST_SCROLLS.with(|cell| {
        cell.borrow_mut().insert(
            id,
            gpui::ListOffset {
                item_ix: index,
                offset_in_item: gpui::px(offset_in_item),
            },
        );
    });
}

#[cfg(target_family = "wasm")]
struct WebAppEntry {
    app: Rc<gpui::ApplicationHandle>,
    window: Rc<RefCell<Option<gpui::WindowHandle<GpuiView>>>>,
}

#[cfg(target_family = "wasm")]
fn update_web_window<R>(
    renderer_id: u64,
    update: impl FnOnce(&mut GpuiView, &mut gpui::Window, &mut gpui::Context<GpuiView>) -> R,
) -> Result<Option<R>> {
    let entry = WEB_APPS.with(|apps| {
        apps.borrow()
            .get(&renderer_id)
            .map(|entry| (entry.app.clone(), entry.window.clone()))
    });
    let Some((app, window)) = entry else {
        return Ok(None);
    };
    let Some(window) = *window.borrow() else {
        return Ok(None);
    };
    app.update(|cx| {
        window
            .update(cx, update)
            .map(Some)
            .map_err(|error| Error::from_reason(error.to_string()))
    })
}

#[cfg(target_family = "wasm")]
fn update_web_window_without_view<R>(
    renderer_id: u64,
    update: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> R,
) -> Result<Option<R>> {
    let entry = WEB_APPS.with(|apps| {
        apps.borrow()
            .get(&renderer_id)
            .map(|entry| (entry.app.clone(), entry.window.clone()))
    });
    let Some((app, window)) = entry else {
        return Ok(None);
    };
    let Some(window) = *window.borrow() else {
        return Ok(None);
    };
    app.update(|cx| {
        gpui::AnyWindowHandle::from(window)
            .update(cx, move |_view, window, cx| update(window, cx))
            .map(Some)
            .map_err(|error| Error::from_reason(error.to_string()))
    })
}

#[cfg(target_family = "wasm")]
fn invalidate_web_window(renderer_id: u64) -> Result<()> {
    update_web_window(renderer_id, |_view, window, cx| {
        cx.notify();
        window.refresh();
    })?;
    Ok(())
}

fn parse_debug_frame_overlay_mode_str(
    mode: &str,
) -> std::result::Result<gpui::DebugFrameOverlayMode, String> {
    match mode {
        "hidden" => Ok(gpui::DebugFrameOverlayMode::Hidden),
        "minimal" => Ok(gpui::DebugFrameOverlayMode::Minimal),
        "full" => Ok(gpui::DebugFrameOverlayMode::Full),
        other => Err(format!(
            "Unknown debug frame overlay mode {other:?}. Use hidden, minimal, or full."
        )),
    }
}

pub(crate) fn parse_debug_frame_overlay_mode(mode: &str) -> Result<gpui::DebugFrameOverlayMode> {
    parse_debug_frame_overlay_mode_str(mode).map_err(Error::from_reason)
}

pub(crate) fn debug_frame_overlay_mode_name(mode: gpui::DebugFrameOverlayMode) -> &'static str {
    match mode {
        gpui::DebugFrameOverlayMode::Hidden => "hidden",
        gpui::DebugFrameOverlayMode::Minimal => "minimal",
        gpui::DebugFrameOverlayMode::Full => "full",
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub(crate) fn debug_frame_overlay_stats_js(
    stats: gpui::DebugFrameOverlayStats,
) -> DebugFrameOverlayStats {
    DebugFrameOverlayStats {
        current_ms: stats.current_ms.map(f64::from),
        p90_ms: stats.p90_ms.map(f64::from),
        p99_ms: stats.p99_ms.map(f64::from),
        max_ms: stats.max_ms.map(f64::from),
        frames: stats.frames as f64,
        samples: stats.samples as f64,
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
fn recv_ui_response<T>(receiver: std::sync::mpsc::Receiver<T>, operation: &str) -> Result<T> {
    match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(response) => Ok(response),
        Err(RecvTimeoutError::Timeout) => Err(Error::from_reason(format!(
            "Timed out after 2 seconds waiting for {operation}"
        ))),
        Err(RecvTimeoutError::Disconnected) => Err(Error::from_reason(format!(
            "The GPUI UI thread stopped during {operation}"
        ))),
    }
}

#[cfg(target_os = "macos")]
fn update_window<R>(
    update: impl FnOnce(&mut GpuiView, &mut gpui::Window, &mut gpui::Context<GpuiView>) -> R,
) -> Result<R> {
    let window = GPUI_WINDOW
        .with(|window| *window.borrow())
        .ok_or_else(|| Error::from_reason("GPUI window is not initialized"))?;

    GPUI_APP.with(|app| {
        let app = app.borrow();
        let app = app
            .as_ref()
            .ok_or_else(|| Error::from_reason("GPUI application is not initialized"))?;
        app.update(|cx| {
            window
                .update(cx, update)
                .map_err(|error| Error::from_reason(error.to_string()))
        })
    })
}

#[cfg(target_os = "macos")]
// Keyboard handlers can update GpuiView, so dispatch without leasing the root view.
fn update_window_without_view<R>(
    update: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> R,
) -> Result<R> {
    let window = GPUI_WINDOW
        .with(|window| *window.borrow())
        .ok_or_else(|| Error::from_reason("GPUI window is not initialized"))?;

    GPUI_APP.with(|app| {
        let app = app.borrow();
        let app = app
            .as_ref()
            .ok_or_else(|| Error::from_reason("GPUI application is not initialized"))?;
        app.update(|cx| {
            gpui::AnyWindowHandle::from(window)
                .update(cx, move |_view, window, cx| update(window, cx))
                .map_err(|error| Error::from_reason(error.to_string()))
        })
    })
}

#[cfg(target_os = "macos")]
fn invalidate_window() -> Result<()> {
    update_window(|_view, window, cx| {
        cx.notify();
        window.refresh();
    })
}

enum MouseInput {
    Click {
        x: f64,
        y: f64,
        button: u32,
        modifiers: gpui::Modifiers,
    },
    Down {
        x: f64,
        y: f64,
        button: u32,
        modifiers: gpui::Modifiers,
    },
    Up {
        x: f64,
        y: f64,
        button: u32,
        modifiers: gpui::Modifiers,
    },
    Move {
        x: f64,
        y: f64,
        pressed_button: Option<u32>,
        modifiers: gpui::Modifiers,
    },
    Wheel {
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        modifiers: gpui::Modifiers,
    },
}

enum KeyInput {
    Keystrokes(String),
    Down { keystroke: String, is_held: bool },
    Up(String),
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
enum ClockControl {
    Pause,
    Set(f64),
    FastForward(f64),
    Resume,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
enum UiCommand {
    Invalidate,
    ActivateWindow,
    Close {
        response: SyncSender<std::result::Result<(), String>>,
    },
    SetWindowTitle(String),
    SetDebugFrameOverlay(gpui::DebugFrameOverlayMode),
    CycleDebugFrameOverlay {
        response: SyncSender<String>,
    },
    GetDebugFrameOverlay {
        response: SyncSender<String>,
    },
    GetDebugFrameOverlayStats {
        response: SyncSender<DebugFrameOverlayStats>,
    },
    ResetDebugFrameOverlayStats,
    ScrollTo {
        id: u64,
        x: f32,
        y: f32,
    },
    ScrollToItem {
        id: u64,
        index: usize,
        offset: f32,
    },
    GetScrollOffset {
        id: u64,
        response: SyncSender<Option<[f64; 2]>>,
    },
    GetListScrollTop {
        id: u64,
        response: SyncSender<Option<[f64; 3]>>,
    },
    GetAutomationBounds {
        response: SyncSender<HashMap<u64, crate::automation::ElementBounds>>,
    },
    GetWindowSize {
        response: SyncSender<WindowSize>,
    },
    GetElementBounds {
        id: u64,
        response: SyncSender<Option<crate::automation::ElementBounds>>,
    },
    FocusElement(u64),
    ControlClock {
        control: ClockControl,
        response: SyncSender<f64>,
    },
    DispatchMouse {
        input: MouseInput,
        response: SyncSender<std::result::Result<(), String>>,
    },
    DispatchKey {
        input: KeyInput,
        response: SyncSender<std::result::Result<(), String>>,
    },
    #[cfg(all(target_os = "windows", feature = "test-support"))]
    CaptureScreenshot {
        path: String,
        response: SyncSender<std::result::Result<(), String>>,
    },
    Blur,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
struct OpenWindowRequest {
    tree: Arc<Mutex<RetainedTree>>,
    callback: Option<EventCallback>,
    title: String,
    options: WindowOptions,
    selection: SharedSelection,
    clock: crate::automation::AutomationClock,
    initialized: Arc<Mutex<bool>>,
    commands: mpsc::UnboundedReceiver<UiCommand>,
    response: SyncSender<std::result::Result<(), String>>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
enum ThreadedAppCommand {
    Open(OpenWindowRequest),
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
fn threaded_app_sender() -> Result<mpsc::UnboundedSender<ThreadedAppCommand>> {
    static SENDER: OnceLock<Mutex<Option<mpsc::UnboundedSender<ThreadedAppCommand>>>> =
        OnceLock::new();
    let sender_slot = SENDER.get_or_init(|| Mutex::new(None));
    let mut sender_slot = sender_slot.lock();
    if let Some(sender) = sender_slot.as_ref() {
        return Ok(sender.clone());
    }

    let (sender, receiver) = mpsc::unbounded();
    let (startup_sender, startup_receiver) = sync_channel(1);
    let exit_startup_sender = startup_sender.clone();
    std::thread::Builder::new()
        .name("gpui_vue-ui".to_string())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                gpui_platform::application()
                    .with_quit_mode(gpui::QuitMode::Explicit)
                    .run(move |cx| {
                        init_key_bindings(cx);
                        crate::custom_elements::input::init(cx);
                        cx.spawn(async move |cx| {
                            run_threaded_app_commands(receiver, cx).await;
                        })
                        .detach();
                        startup_sender.send(Ok(())).ok();
                    });
            }));

            let error = match result {
                Ok(()) => "The shared GPUI event loop exited unexpectedly".to_string(),
                Err(payload) => format!(
                    "The shared GPUI UI thread panicked: {}",
                    panic_message(payload)
                ),
            };
            exit_startup_sender.try_send(Err(error)).ok();
        })
        .map_err(|error| {
            Error::from_reason(format!(
                "Failed to spawn the shared GPUI UI thread: {error}"
            ))
        })?;

    match startup_receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => {
            *sender_slot = Some(sender.clone());
            Ok(sender)
        }
        Ok(Err(error)) => Err(Error::from_reason(error)),
        Err(RecvTimeoutError::Timeout) => Err(Error::from_reason(
            "Timed out waiting for the shared GPUI UI thread to start",
        )),
        Err(RecvTimeoutError::Disconnected) => Err(Error::from_reason(
            "The shared GPUI UI thread stopped during initialization",
        )),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
async fn run_threaded_app_commands(
    mut commands: mpsc::UnboundedReceiver<ThreadedAppCommand>,
    cx: &mut gpui::AsyncApp,
) {
    while let Some(command) = commands.next().await {
        match command {
            ThreadedAppCommand::Open(request) => {
                let OpenWindowRequest {
                    tree,
                    callback,
                    title,
                    options,
                    selection,
                    clock,
                    initialized,
                    commands,
                    response,
                } = request;
                let width = options.width.unwrap_or(800.0);
                let height = options.height.unwrap_or(600.0);
                let activate = options.focus.unwrap_or(true);
                let window_options = options.clone();
                let initialized_for_close = initialized.clone();
                let callback_for_close = callback.clone();
                let result = cx.update(|cx| -> std::result::Result<(), String> {
                    let bounds = gpui::Bounds::centered(
                        None,
                        gpui::size(gpui::px(width as f32), gpui::px(height as f32)),
                        cx,
                    );
                    let window = cx
                        .open_window(
                            to_gpui_window_options(&window_options, bounds),
                            |_window, cx| {
                                cx.new(|_| GpuiView::new(tree, callback, title, selection, clock))
                            },
                        )
                        .map_err(|error| format!("Failed to open the GPUI window: {error}"))?;
                    let window_id = window.window_id();
                    cx.on_window_closed(move |_cx, closed_id| {
                        if closed_id == window_id {
                            *initialized_for_close.lock() = false;
                            emit_event_full(&callback_for_close, 0, "windowClose", |_| {});
                        }
                    })
                    .detach();
                    cx.spawn(async move |cx| {
                        run_ui_commands(commands, window, cx).await;
                    })
                    .detach();
                    *initialized.lock() = true;
                    if activate {
                        cx.activate(true);
                    }
                    Ok(())
                });
                response.send(result).ok();
            }
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
fn refresh_ui_window(
    window: gpui::WindowHandle<GpuiView>,
    cx: &mut gpui::AsyncApp,
) -> gpui::Result<()> {
    window.update(cx, |_view, window, cx| {
        cx.notify();
        window.refresh();
    })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
#[allow(
    clippy::too_many_lines,
    reason = "the UI-thread command loop is one exhaustive protocol dispatcher"
)]
async fn run_ui_commands(
    mut commands: mpsc::UnboundedReceiver<UiCommand>,
    window: gpui::WindowHandle<GpuiView>,
    cx: &mut gpui::AsyncApp,
) {
    let mut closed_by_command = false;
    while let Some(command) = commands.next().await {
        let result = match command {
            UiCommand::Invalidate => refresh_ui_window(window, cx),
            UiCommand::ActivateWindow => window.update(cx, |_view, window, cx| {
                cx.activate(true);
                window.activate_window();
            }),
            UiCommand::Close { response } => {
                let result = window
                    .update(cx, |_view, window, _cx| window.remove_window())
                    .map_err(|error| error.to_string());
                closed_by_command = result.is_ok();
                response.send(result).ok();
                break;
            }
            UiCommand::SetWindowTitle(title) => window.update(cx, move |view, window, cx| {
                view.window_title = title;
                cx.notify();
                window.refresh();
            }),
            UiCommand::SetDebugFrameOverlay(mode) => {
                window.update(cx, move |_view, window, _cx| {
                    window.set_debug_frame_overlay_mode(mode);
                })
            }
            UiCommand::CycleDebugFrameOverlay { response } => {
                window.update(cx, move |_view, window, _cx| {
                    window.cycle_debug_frame_overlay_mode();
                    response
                        .send(
                            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).into(),
                        )
                        .ok();
                })
            }
            UiCommand::GetDebugFrameOverlay { response } => {
                window.update(cx, move |_view, window, _cx| {
                    response
                        .send(
                            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).into(),
                        )
                        .ok();
                })
            }
            UiCommand::GetDebugFrameOverlayStats { response } => {
                window.update(cx, move |_view, window, _cx| {
                    response
                        .send(debug_frame_overlay_stats_js(
                            window.debug_frame_overlay_stats(),
                        ))
                        .ok();
                })
            }
            UiCommand::ResetDebugFrameOverlayStats => window.update(cx, |_view, window, _cx| {
                window.reset_debug_frame_overlay_stats();
            }),
            UiCommand::ScrollTo { id, x, y } => {
                if !VIRTUAL_LIST_STATES.with(|cell| {
                    let states = cell.borrow();
                    let Some(state) = states.get(&id) else {
                        return false;
                    };
                    state.set_offset_from_scrollbar(gpui::point(gpui::px(x), gpui::px(y)));
                    true
                }) {
                    SCROLL_HANDLES.with(|cell| {
                        if let Some(handle) = cell.borrow().get(&id) {
                            handle.set_offset(gpui::point(gpui::px(x), gpui::px(y)));
                        }
                    });
                }
                refresh_ui_window(window, cx)
            }
            UiCommand::ScrollToItem { id, index, offset } => {
                if !VIRTUAL_LIST_STATES.with(|cell| {
                    if !cell.borrow().contains_key(&id) {
                        return false;
                    }
                    queue_virtual_list_scroll(id, index, offset);
                    true
                }) {
                    SCROLL_HANDLES.with(|cell| {
                        if let Some(handle) = cell.borrow().get(&id) {
                            handle.scroll_to_item(index);
                        }
                    });
                }
                refresh_ui_window(window, cx)
            }
            UiCommand::GetScrollOffset { id, response } => {
                let offset = VIRTUAL_LIST_STATES
                    .with(|cell| {
                        cell.borrow().get(&id).map(|state| {
                            let offset = state.scroll_px_offset_for_scrollbar();
                            [
                                f64::from(f32::from(offset.x)),
                                f64::from(f32::from(offset.y)),
                            ]
                        })
                    })
                    .or_else(|| {
                        SCROLL_HANDLES.with(|cell| {
                            cell.borrow().get(&id).map(|handle| {
                                let offset = handle.offset();
                                [
                                    f64::from(f32::from(offset.x)),
                                    f64::from(f32::from(offset.y)),
                                ]
                            })
                        })
                    });
                response.send(offset).ok();
                Ok(())
            }
            UiCommand::GetListScrollTop { id, response } => {
                let top = VIRTUAL_LIST_STATES.with(|cell| {
                    cell.borrow().get(&id).map(|state| {
                        let top = state.logical_scroll_top();
                        [
                            top.item_ix as f64,
                            f64::from(f32::from(top.offset_in_item)),
                            f64::from(f32::from(state.viewport_bounds().size.height)),
                        ]
                    })
                });
                response.send(top).ok();
                Ok(())
            }
            UiCommand::GetWindowSize { response } => {
                window.update(cx, move |_view, window, _cx| {
                    let size = window.viewport_size();
                    response
                        .send(WindowSize {
                            width: f64::from(f32::from(size.width)),
                            height: f64::from(f32::from(size.height)),
                        })
                        .ok();
                })
            }
            UiCommand::GetAutomationBounds { response } => {
                window.update(cx, move |_view, window, cx| {
                    cx.notify();
                    window.refresh();
                    window.on_next_frame(move |_window, _cx| {
                        response.send(crate::automation::all_bounds()).ok();
                    });
                })
            }
            UiCommand::GetElementBounds { id, response } => {
                window.update(cx, move |_view, window, cx| {
                    cx.notify();
                    window.refresh();
                    window.on_next_frame(move |_window, _cx| {
                        response.send(crate::automation::get_bounds(id)).ok();
                    });
                })
            }
            UiCommand::FocusElement(id) => window.update(cx, move |view, window, cx| {
                view.reveal_virtual_list_ancestor(id);
                if let Some(handle) = view.focus_handles.get(&id) {
                    handle.focus(window, cx);
                }
                cx.notify();
                window.refresh();
            }),
            UiCommand::ControlClock { control, response } => {
                window.update(cx, move |view, _window, cx| {
                    let now_ms = match control {
                        ClockControl::Pause => view.clock.pause(),
                        ClockControl::Set(now_ms) => view.clock.set_ms(now_ms),
                        ClockControl::FastForward(delta_ms) => view.clock.fast_forward_ms(delta_ms),
                        ClockControl::Resume => view.clock.resume(),
                    };
                    cx.notify();
                    response.send(now_ms).ok();
                })
            }
            UiCommand::DispatchMouse { input, response } => {
                let result = window.update(cx, move |_view, window, cx| match input {
                    MouseInput::Click {
                        x,
                        y,
                        button,
                        modifiers,
                    } => {
                        crate::automation::dispatch_click(window, cx, x, y, button, modifiers);
                    }
                    MouseInput::Down {
                        x,
                        y,
                        button,
                        modifiers,
                    } => {
                        crate::automation::dispatch_mouse_down(window, cx, x, y, button, modifiers);
                    }
                    MouseInput::Up {
                        x,
                        y,
                        button,
                        modifiers,
                    } => {
                        crate::automation::dispatch_mouse_up(window, cx, x, y, button, modifiers);
                    }
                    MouseInput::Move {
                        x,
                        y,
                        pressed_button,
                        modifiers,
                    } => {
                        crate::automation::dispatch_mouse_move(
                            window,
                            cx,
                            x,
                            y,
                            pressed_button,
                            modifiers,
                        );
                    }
                    MouseInput::Wheel {
                        x,
                        y,
                        delta_x,
                        delta_y,
                        modifiers,
                    } => {
                        crate::automation::dispatch_scroll_wheel(
                            window, cx, x, y, delta_x, delta_y, modifiers,
                        );
                    }
                });
                response
                    .send(
                        result
                            .as_ref()
                            .map_err(|error| format!("{error:#}"))
                            .copied(),
                    )
                    .ok();
                result
            }
            UiCommand::DispatchKey { input, response } => {
                let result = gpui::AnyWindowHandle::from(window)
                    .update(cx, move |_view, window, cx| match input {
                        KeyInput::Keystrokes(keystrokes) => {
                            crate::automation::dispatch_keystrokes(window, cx, &keystrokes)
                        }
                        KeyInput::Down { keystroke, is_held } => {
                            crate::automation::dispatch_key_down(window, cx, &keystroke, is_held)
                        }
                        KeyInput::Up(keystroke) => {
                            crate::automation::dispatch_key_up(window, cx, &keystroke)
                        }
                    })
                    .and_then(|result| result.map_err(|error| std::io::Error::other(error).into()));
                response
                    .send(
                        result
                            .as_ref()
                            .map_err(|error| format!("{error:#}"))
                            .copied(),
                    )
                    .ok();
                result
            }
            #[cfg(all(target_os = "windows", feature = "test-support"))]
            UiCommand::CaptureScreenshot { path, response } => {
                let error_response = response.clone();
                let result = window.update(cx, move |_view, window, cx| {
                    cx.notify();
                    window.refresh();
                    window.on_next_frame(move |window, _cx| {
                        let result = window
                            .render_to_image()
                            .map_err(|error| format!("Screenshot capture failed: {error}"))
                            .and_then(|image| {
                                image
                                    .save(&path)
                                    .map_err(|error| format!("Failed to save screenshot: {error}"))
                            });
                        response.send(result).ok();
                    });
                });
                if let Err(error) = &result {
                    error_response.send(Err(format!("{error:#}"))).ok();
                }
                result
            }
            UiCommand::Blur => window.update(cx, |_view, window, _cx| window.blur()),
        };
        if let Err(error) = result {
            log::error!("Failed to handle GPUI UI command: {error:#}");
        }
    }
    // Dropping a renderer without calling close still releases only its own
    // window. The shared GPUI application remains available for later roots.
    if !closed_by_command {
        window
            .update(cx, |_view, window, _cx| window.remove_window())
            .ok();
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string())
}

fn collect_text(id: u64, tree: &RetainedTree, texts: &mut Vec<String>) {
    if let Some(element) = tree.elements.get(&id) {
        if let Some(ref content) = element.content {
            texts.push(content.to_string());
        }
        for &child_id in &element.children {
            collect_text(child_id, tree, texts);
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
impl Drop for GpuiRenderer {
    fn drop(&mut self) {
        self.ui_commands.lock().take();
    }
}

// ── GPUI View ────────────────────────────────────────────────────────

pub(crate) struct GpuiView {
    pub(crate) tree: Arc<Mutex<RetainedTree>>,
    /// Structurally shared immutable tree used after the mutation lock is released.
    render_tree: Arc<RetainedTree>,
    pub(crate) event_callback: Option<EventCallback>,
    pub(crate) window_title: String,
    /// Persistent `FocusHandles` keyed by element ID.
    /// Created lazily for elements with keyboard or focus/blur listeners.
    /// Handles persist across renders so GPUI maintains focus state.
    pub(crate) focus_handles: HashMap<u64, gpui::FocusHandle>,
    /// Active focus/blur subscriptions keyed by element and event type.
    pub(crate) focus_subscriptions: HashMap<(u64, String), gpui::Subscription>,
    /// Registry for custom element types (input, editor, diff, etc.).
    /// Stores factories (one per type) and live instances (one per element ID).
    pub(crate) custom_registry: CustomElementRegistry,
    /// Persistent `ScrollHandles` keyed by element ID.
    /// Created lazily for elements with overflow: "scroll" (or per-axis scroll).
    /// Handles persist across renders so GPUI maintains scroll offset state.
    pub(crate) scroll_handles: HashMap<u64, gpui::ScrollHandle>,
    /// Native animation clocks keyed by retained element ID.
    pub(crate) motion_states: HashMap<u64, crate::motion::MotionState>,
    /// Live text selection, shared with the paint closures and the napi methods.
    pub(crate) selection: SharedSelection,
    /// Persistent measurement and scroll state for Vue-backed virtual lists.
    virtual_lists: HashMap<u64, VirtualListEntry>,
    /// Motion / review clock. Live wall time unless automation freezes it.
    pub(crate) clock: crate::automation::AutomationClock,
    /// Resolved `highlight` state, keyed by the element that declared it.
    /// Empty in every app that does not use search.
    highlights: HashMap<u64, HighlightCacheEntry>,
    /// Immutable process theme used by inherited renderer chrome.
    theme: Theme,
    /// Last revision for O(tree) registry/focus maintenance.
    last_tree_revision: u64,
    window_bounds_subscription: Option<gpui::Subscription>,
}

/// Two-level cache for one element's `highlight`.
///
/// The group list is keyed by `search_revision`, which a query change does NOT
/// move, so typing in a find bar never re-walks or re-folds text. The matches
/// are additionally keyed by the matcher hash, which excludes `activeIndex` and
/// the colours, so moving the find cursor only re-colours what it already found.
///
/// Do not key the group list on `subtree_revision`: `highlight` is a custom
/// prop, so every keystroke moves that revision and the cache would do nothing.
/// `highlight_cache_tests` at the bottom of this file compares `Arc` identity
/// and fails if either level regresses. A timing budget does not catch it: on
/// the 1000-turn chat the broken version is 2.7ms against 1.9ms.
struct HighlightCacheEntry {
    revision: u64,
    groups: Arc<crate::text::GroupList>,
    matcher_hash: u64,
    /// The spec plus the located matches. Ordinals and colours are decided at
    /// paint, so a colour or `activeIndex` change reuses this whole value.
    context: Arc<crate::text::HighlightContext>,
    /// Last identity delivered through `onHighlight`. Only written once an
    /// event is really queued, so adding the listener later still reports.
    reported: Option<u64>,
}

#[allow(
    clippy::ref_option,
    reason = "the callback container is shared with the event-emission API and avoids repeated adaptation"
)]
fn emit_highlight_events(callback: &Option<EventCallback>, events: &[(u64, usize)]) {
    for &(id, total) in events {
        emit_event_full(callback, id, "highlight", |payload| {
            payload.match_count = Some(total as f64);
        });
    }
}

/// Resolve one element's `highlight` prop, reusing both cache levels.
///
/// Returns the context, plus the match count when `has_listener` and the result
/// differs from the last one this element reported. Identity, not count:
/// swapping a query for a different one with the same number of hits is still a
/// new result.
fn resolve_highlight(
    cache: &mut HashMap<u64, HighlightCacheEntry>,
    tree: &RetainedTree,
    id: u64,
    value: &serde_json::Value,
    theme: &Theme,
    has_listener: bool,
) -> Option<(Arc<crate::text::HighlightContext>, Option<usize>)> {
    let set = crate::text::HighlightSet::parse(value, theme)?;
    // `search_revision`, NOT `subtree_revision`: `highlight` is a custom prop,
    // so the general revision moves on every keystroke and this cache would
    // never hit for the one case it exists for.
    let revision = tree.elements.get(&id)?.search_revision;
    let matcher_hash = set.matcher_hash();

    let cached = cache
        .get(&id)
        .filter(|entry| entry.revision == revision && entry.matcher_hash == matcher_hash);
    let context = match cached {
        // Nothing moved at all. Returning the same `Arc` keeps the whole
        // subtree's inherited value identical, which the cache tests assert.
        Some(entry) if entry.context.set == set => entry.context.clone(),
        // Same matches, different colours or find cursor: reuse the located
        // matches and swap only the spec. No text is scanned.
        Some(entry) => {
            let context = Arc::new(crate::text::HighlightContext {
                declaration: id,
                set,
                matches: entry.context.matches.clone(),
            });
            cache.get_mut(&id)?.context = context.clone();
            context
        }
        None => {
            let groups = match cache.get(&id) {
                Some(entry) if entry.revision == revision => entry.groups.clone(),
                _ => Arc::new(crate::text::GroupList::collect(tree, id)),
            };
            let context = Arc::new(crate::text::HighlightContext {
                declaration: id,
                matches: Arc::new(crate::text::search::resolve(&groups, &set)),
                set,
            });
            let reported = cache.get(&id).and_then(|entry| entry.reported);
            cache.insert(
                id,
                HighlightCacheEntry {
                    revision,
                    groups,
                    matcher_hash,
                    context: context.clone(),
                    reported,
                },
            );
            context
        }
    };

    if !has_listener {
        return Some((context, None));
    }
    let identity = context.matches.identity();
    let entry = cache.get_mut(&id)?;
    if entry.reported == Some(identity) {
        return Some((context, None));
    }
    entry.reported = Some(identity);
    let total = context.matches.total;
    Some((context, Some(total)))
}

impl GpuiView {
    pub(crate) fn new(
        tree: Arc<Mutex<RetainedTree>>,
        event_callback: Option<EventCallback>,
        window_title: String,
        selection: SharedSelection,
        clock: crate::automation::AutomationClock,
    ) -> Self {
        Self {
            tree,
            render_tree: Arc::new(RetainedTree::new()),
            event_callback,
            window_title,
            focus_handles: HashMap::new(),
            focus_subscriptions: HashMap::new(),
            custom_registry: CustomElementRegistry::with_defaults(),
            scroll_handles: HashMap::new(),
            motion_states: HashMap::new(),
            selection,
            virtual_lists: HashMap::new(),
            clock,
            highlights: HashMap::new(),
            theme: Theme::dark(),
            last_tree_revision: 0,
            window_bounds_subscription: None,
        }
    }

    /// Refresh the immutable render tree only when a retained mutation landed.
    /// The mutex is held for pointer-map cloning, never for GPUI construction.
    fn tree_snapshot(&mut self) -> Arc<RetainedTree> {
        let current_revision = self.render_tree.revision();
        let next = {
            let tree = self.tree.lock();
            (tree.revision() != current_revision).then(|| Arc::new(tree.render_snapshot()))
        };
        if let Some(next) = next {
            self.render_tree = next;
        }
        self.render_tree.clone()
    }

    fn build_virtual_child(
        &mut self,
        list_id: u64,
        index: usize,
        expected_child_id: u64,
        inherited: Inherited,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let row_focus_handle = self.virtual_lists.get_mut(&list_id).and_then(|entry| {
            entry.seen_rows.insert(expected_child_id);
            (entry.child_at(index) == Some(expected_child_id))
                .then(|| {
                    index
                        .checked_sub(entry.window_start)
                        .and_then(|offset| entry.row_focus_handles.get(offset).cloned())
                })
                .flatten()
                .flatten()
        });

        let tree = self.tree_snapshot();
        let window_start = self
            .virtual_lists
            .get(&list_id)
            .map_or(0, |entry| entry.window_start);
        let child_matches = tree.elements.get(&list_id).and_then(|list| {
            index
                .checked_sub(window_start)
                .and_then(|offset| list.children.get(offset))
        }) == Some(&expected_child_id);
        if !child_matches {
            let height = self
                .virtual_lists
                .get(&list_id)
                .and_then(|entry| entry.config.estimated_item_height)
                .unwrap_or(1.0);
            return unmounted_virtual_row(height);
        }

        let callback = self.event_callback.clone();
        let now = self.clock.now();
        let mut motion_active = false;
        let mut highlight_events = Vec::new();

        // Re-resolve against the tree as it is NOW. gpui calls this during
        // layout and prepaint, after the root render returned, and on Windows
        // and Linux the Node thread can commit new text in between. Reusing the
        // captured ranges would paint a wash over the wrong glyphs, or at a byte
        // offset that is no longer a character boundary.
        let mut inherited = inherited;
        if let Some(declaration) = inherited.highlight.as_ref().map(|ctx| ctx.declaration) {
            inherited.highlight = tree
                .elements
                .get(&declaration)
                .and_then(|element| element.custom_props.get("highlight"))
                .and_then(|value| {
                    resolve_highlight(
                        &mut self.highlights,
                        &tree,
                        declaration,
                        value,
                        &self.theme,
                        false,
                    )
                })
                .map(|(context, _)| context);
        }

        let mut build_ctx = BuildCtx {
            tree: &tree,
            event_callback: &callback,
            focus_handles: &self.focus_handles,
            scroll_handles: &mut self.scroll_handles,
            custom_registry: &mut self.custom_registry,
            virtual_lists: &mut self.virtual_lists,
            motion_states: &mut self.motion_states,
            now,
            motion_active: &mut motion_active,
            selection: self.selection.clone(),
            inherited,
            highlights: &mut self.highlights,
            highlight_events: &mut highlight_events,
            theme: &self.theme,
        };
        let child = build_element(expected_child_id, &mut build_ctx, window, cx);
        emit_highlight_events(&callback, &highlight_events);
        if motion_active && self.clock.is_playing() {
            window.request_animation_frame();
        }
        let Some(focus_handle) = row_focus_handle else {
            return child;
        };
        gpui::div()
            .id(gpui::SharedString::from(format!(
                "__gpui_vue_virtual_row_{list_id}_{expected_child_id}"
            )))
            .w_full()
            .track_focus(&focus_handle)
            .child(child)
            .into_any_element()
    }

    pub(crate) fn scroll_virtual_list_to_item(
        &self,
        id: u64,
        index: usize,
        offset_in_item: f32,
    ) -> bool {
        if !self.virtual_lists.contains_key(&id) {
            return false;
        }
        queue_virtual_list_scroll(id, index, offset_in_item);
        emit_event_full(&self.event_callback, id, "visibleRange", |payload| {
            payload.start_index = Some(index as f64);
            payload.end_index = Some((index + 1) as f64);
        });
        true
    }

    #[cfg(all(
        feature = "test-support",
        any(target_os = "macos", target_os = "windows")
    ))]
    pub(crate) fn virtual_list_scroll_top(&self, id: u64) -> Option<[f64; 3]> {
        let state = &self.virtual_lists.get(&id)?.state;
        let top = state.logical_scroll_top();
        Some([
            top.item_ix as f64,
            f64::from(f32::from(top.offset_in_item)),
            f64::from(f32::from(state.viewport_bounds().size.height)),
        ])
    }

    #[cfg(all(
        feature = "test-support",
        any(target_os = "macos", target_os = "windows")
    ))]
    pub(crate) fn set_virtual_list_offset(&self, id: u64, x: f32, y: f32) -> bool {
        let Some(entry) = self.virtual_lists.get(&id) else {
            return false;
        };
        entry
            .state
            .set_offset_from_scrollbar(gpui::point(gpui::px(x), gpui::px(y)));
        true
    }

    #[cfg(all(
        feature = "test-support",
        any(target_os = "macos", target_os = "windows")
    ))]
    pub(crate) fn virtual_list_offset(&self, id: u64) -> Option<[f64; 2]> {
        let offset = self
            .virtual_lists
            .get(&id)?
            .state
            .scroll_px_offset_for_scrollbar();
        Some([
            f64::from(f32::from(offset.x)),
            f64::from(f32::from(offset.y)),
        ])
    }

    pub(crate) fn reveal_virtual_list_ancestor(&self, id: u64) -> bool {
        let tree_arc = self.tree.clone();
        let tree = tree_arc.lock();
        let mut current = id;
        let location = loop {
            let Some(parent_id) = tree
                .elements
                .get(&current)
                .and_then(|element| element.parent)
            else {
                break None;
            };
            if self.virtual_lists.contains_key(&parent_id) {
                let index = tree
                    .elements
                    .get(&parent_id)
                    .and_then(|parent| parent.children.iter().position(|child| *child == current));
                break index.map(|index| (parent_id, index));
            }
            current = parent_id;
        };
        drop(tree);

        let Some((list_id, index)) = location else {
            return false;
        };
        self.scroll_virtual_list_to_item(list_id, index, 0.0)
    }
}

/// Everything `build_element` threads through the tree.
///
/// Split into a struct because the recursion needs eight-plus shared references
/// and adding one more to every call site is how this file rots. `window` and
/// `cx` stay separate parameters: they are `&mut` and gpui reborrows them.
pub(crate) struct BuildCtx<'a> {
    pub tree: &'a RetainedTree,
    pub event_callback: &'a Option<EventCallback>,
    pub focus_handles: &'a HashMap<u64, gpui::FocusHandle>,
    pub scroll_handles: &'a mut HashMap<u64, gpui::ScrollHandle>,
    pub custom_registry: &'a mut CustomElementRegistry,
    virtual_lists: &'a mut HashMap<u64, VirtualListEntry>,
    pub motion_states: &'a mut HashMap<u64, crate::motion::MotionState>,
    pub now: Instant,
    pub motion_active: &'a mut bool,
    pub selection: SharedSelection,
    /// Inherited text state, resolved the way CSS inherits it. The renderer's
    /// own theme only seeds the root selection wash; custom elements resolve
    /// their own theme from their `theme` prop.
    pub inherited: Inherited,
    /// Persistent `highlight` caches, keyed by the declaring element.
    highlights: &'a mut HashMap<u64, HighlightCacheEntry>,
    /// `onHighlight` payloads queued during the build.
    ///
    /// Never emitted inline: a handler that calls `setState` repaints, which
    /// would re-enter the build and emit again. They are flushed once the root
    /// build has returned.
    highlight_events: &'a mut Vec<(u64, usize)>,
    /// Cached renderer theme; constructing a palette allocates strings.
    theme: &'a Theme,
}

/// Style properties that cascade into descendants.
///
/// Not `Copy`: `highlight` holds an `Arc`. Every call site must clone
/// explicitly, including the deferred `build_virtual_child` callback, which gpui
/// may run more than once per frame.
#[derive(Clone)]
pub(crate) struct Inherited {
    /// False once an ancestor sets `userSelect: "none"`.
    pub selectable: bool,
    /// Selection wash colour for this subtree.
    pub selection_wash: gpui::Hsla,
    /// The nearest ancestor's `highlight`, resolved. `None` in every app that
    /// does not use search. It carries the declaring element id, which is what
    /// a virtual-list row re-resolves against: that row is built after the root
    /// render returns, and on Windows and Linux the Node thread can edit text
    /// in between, so a stale range would paint over the wrong glyphs.
    pub highlight: Option<Arc<crate::text::HighlightContext>>,
}

impl Inherited {
    fn root(theme: &Theme) -> Self {
        let mut wash = theme.accent;
        wash.a = 0.35;
        Self {
            selectable: true,
            selection_wash: wash,
            highlight: None,
        }
    }

    /// Apply the inheritable parts of `style` for the subtree below it.
    fn descend(mut self, style: Option<&StyleDesc>) -> Self {
        let Some(style) = style else { return self };
        match style.resolved_user_select() {
            Some(SelectValue::None) => self.selectable = false,
            Some(SelectValue::Text) => self.selectable = true,
            _ => {}
        }
        if let Some(color) = style.resolved_selection_color() {
            self.selection_wash = color.into();
        }
        self
    }
}

impl GpuiView {
    /// Sync focus handles with the current element tree.
    /// Creates handles for new focusable elements, subscribes `on_focus/on_blur`,
    /// and cleans up handles for destroyed elements.
    #[allow(
        clippy::ref_option,
        reason = "the retained optional callback is shared by every installed focus subscription"
    )]
    fn sync_focus_handles(
        &mut self,
        tree: &RetainedTree,
        callback: &Option<EventCallback>,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let tab_index = |element: &crate::retained_tree::RetainedElement| {
            element
                .custom_props
                .get("tabIndex")
                .and_then(serde_json::Value::as_i64)
                .and_then(|index| isize::try_from(index).ok())
        };
        let needs_focus = |element: &crate::retained_tree::RetainedElement| {
            matches!(element.element_type.as_str(), "input" | "textarea")
                || tab_index(element).is_some()
                || element.events.contains("keyDown")
                || element.events.contains("keyUp")
                || element.events.contains("focus")
                || element.events.contains("blur")
        };
        // Create handles for elements that need focus but don't have one yet.
        for (&id, element) in &tree.elements {
            let tab_index = tab_index(element).or_else(|| {
                matches!(element.element_type.as_str(), "input" | "textarea").then_some(0)
            });

            if needs_focus(element) && !self.focus_handles.contains_key(&id) {
                let handle = match tab_index {
                    Some(index) => cx.focus_handle().tab_index(index).tab_stop(index >= 0),
                    None => cx.focus_handle(),
                };
                // Focus once, at creation. Re-focusing every frame would
                // steal focus back from whatever the user clicked next.
                if element.auto_focus {
                    handle.focus(window, cx);
                }
                self.focus_handles.insert(id, handle);
            } else if let (Some(handle), Some(index)) =
                (self.focus_handles.get(&id).cloned(), tab_index)
            {
                self.focus_handles
                    .insert(id, handle.tab_index(index).tab_stop(index >= 0));
            } else if let Some(handle) = self.focus_handles.get(&id).cloned() {
                self.focus_handles.insert(id, handle.tab_stop(false));
            }
        }

        self.focus_subscriptions.retain(|(id, event), _| {
            tree.elements
                .get(id)
                .is_some_and(|element| element.events.contains(event))
        });
        for (&id, element) in &tree.elements {
            let Some(handle) = self.focus_handles.get(&id).cloned() else {
                continue;
            };
            let focus_key = (id, "focus".to_string());
            if element.events.contains("focus")
                && !self.focus_subscriptions.contains_key(&focus_key)
            {
                let callback = callback.clone();
                let subscription = cx.on_focus(&handle, window, move |_this, _window, _cx| {
                    emit_event_full(&callback, id, "focus", |_| {});
                });
                self.focus_subscriptions.insert(focus_key, subscription);
            }
            let blur_key = (id, "blur".to_string());
            if element.events.contains("blur") && !self.focus_subscriptions.contains_key(&blur_key)
            {
                let callback = callback.clone();
                let subscription = cx.on_blur(&handle, window, move |_this, _window, _cx| {
                    emit_event_full(&callback, id, "blur", |_| {});
                });
                self.focus_subscriptions.insert(blur_key, subscription);
            }
        }

        // Clean up handles for elements that no longer exist.
        self.focus_handles.retain(|id, _| {
            tree.elements
                .get(id)
                .is_some_and(|element| needs_focus(element))
        });
    }
}

impl gpui::Render for GpuiView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::IntoElement;

        window.set_window_title(&self.window_title);

        if self.window_bounds_subscription.is_none() {
            let callback = self.event_callback.clone();
            self.window_bounds_subscription =
                Some(cx.observe_window_bounds(window, move |_view, window, _cx| {
                    let size = window.viewport_size();
                    emit_event_full(&callback, 0, "windowResize", |payload| {
                        payload.width = Some(f64::from(f32::from(size.width)));
                        payload.height = Some(f64::from(f32::from(size.height)));
                    });
                }));
        }

        // Snapshot under the mutation mutex, then release it before any GPUI
        // construction. Batches can now arrive while a frame is being built.
        let tree = self.tree_snapshot();
        let callback = self.event_callback.clone();
        let tree_revision = tree.revision();
        let tree_changed = tree_revision != self.last_tree_revision;

        if tree_changed {
            // All of these are O(tree) maintenance passes. Motion frames reuse
            // them until a retained mutation advances the revision.
            self.sync_focus_handles(&tree, &callback, window, cx);
            self.custom_registry
                .prune_missing(|id| tree.elements.contains_key(&id));
            self.scroll_handles
                .retain(|id, _| tree.elements.contains_key(id));
            self.virtual_lists
                .retain(|id, _| tree.elements.contains_key(id));
            self.motion_states
                .retain(|id, _| tree.elements.contains_key(id));
            self.highlights.retain(|id, _| {
                tree.elements
                    .get(id)
                    .is_some_and(|element| element.custom_props.contains_key("highlight"))
            });
            self.last_tree_revision = tree_revision;
        }

        // Build the element tree. custom_registry, focus_handles, and scroll_handles
        // are different fields of self, so Rust allows borrowing all simultaneously.
        let theme = &self.theme;
        let now = self.clock.now();
        let mut motion_active = false;
        let mut highlight_events = Vec::new();
        let result = match tree.root_id {
            Some(root_id) => {
                let mut ctx = BuildCtx {
                    tree: &tree,
                    event_callback: &callback,
                    focus_handles: &self.focus_handles,
                    scroll_handles: &mut self.scroll_handles,
                    custom_registry: &mut self.custom_registry,
                    virtual_lists: &mut self.virtual_lists,
                    motion_states: &mut self.motion_states,
                    now,
                    motion_active: &mut motion_active,
                    selection: self.selection.clone(),
                    inherited: Inherited::root(theme),
                    highlights: &mut self.highlights,
                    highlight_events: &mut highlight_events,
                    theme,
                };
                build_element(root_id, &mut ctx, window, cx)
            }
            None => gpui::Empty.into_any_element(),
        };
        // Flushed after the root build so a `setState` in the handler cannot
        // re-enter this build.
        emit_highlight_events(&callback, &highlight_events);

        // The frame reset must paint BEFORE any text, so it is the first child of
        // the root wrapper. Without it the selection registry accumulates stale
        // entries across frames and a drag resolves against elements that are no
        // longer on screen.
        let result = {
            use gpui::prelude::*;
            let root = gpui::div()
                .size_full()
                .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
                .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx));
            with_window_menu_actions(root)
                .child(selection_frame_reset(self.selection.clone()))
                .child(crate::automation::bounds_frame_reset())
                .child(result)
                .into_any_element()
        };

        // Sync scroll handles to thread_local so napi methods (scrollTo,
        // getScrollOffset) can access them without an App context.
        if tree_changed {
            SCROLL_HANDLES.with(|cell| {
                let mut handles = cell.borrow_mut();
                handles.clear();
                for (&id, handle) in &self.scroll_handles {
                    handles.insert(id, handle.clone());
                }
            });
            VIRTUAL_LIST_STATES.with(|cell| {
                let mut states = cell.borrow_mut();
                states.clear();
                for (&id, entry) in &self.virtual_lists {
                    states.insert(id, entry.state.clone());
                }
            });
        }
        // One-shot: if the target list was not built, its committed indices can
        // no longer be assumed to mean the same thing on a later frame.
        PENDING_VIRTUAL_LIST_SCROLLS.with(|cell| cell.borrow_mut().clear());

        if motion_active && self.clock.is_playing() {
            window.request_animation_frame();
        }

        result
    }
}

// ── Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi(object))]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi(object))]
pub struct EdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi(object))]
pub struct WindowInsets {
    pub safe_area: EdgeInsets,
    pub ime: EdgeInsets,
    pub effective: EdgeInsets,
}

impl WindowInsets {
    #[cfg(any(target_os = "macos", target_family = "wasm"))]
    fn from_gpui(insets: gpui::WindowInsets) -> Self {
        let effective = insets.effective();
        Self {
            safe_area: EdgeInsets::from_gpui(insets.safe_area),
            ime: EdgeInsets::from_gpui(insets.ime),
            effective: EdgeInsets::from_gpui(effective),
        }
    }
}

impl EdgeInsets {
    #[cfg(any(target_os = "macos", target_family = "wasm"))]
    fn from_gpui(insets: gpui::Edges<gpui::Pixels>) -> Self {
        Self {
            top: f32::from(insets.top) as f64,
            right: f32::from(insets.right) as f64,
            bottom: f32::from(insets.bottom) as f64,
            left: f32::from(insets.left) as f64,
        }
    }
}

/// Recorded draw times from the debug frame overlay.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#[derive(Debug, Clone)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi(object))]
pub struct DebugFrameOverlayStats {
    pub current_ms: Option<f64>,
    pub p90_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub frames: f64,
    pub samples: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_family = "wasm"), napi(object))]
pub struct TimelineState {
    pub current_time_ms: f64,
    pub playback_rate: f64,
    pub playing: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_family = "wasm"), napi(object))]
pub struct AudioBufferState {
    pub sample_rate: u32,
    pub channels: u32,
    pub capacity_frames: f64,
    pub queued_frames: f64,
    pub dropped_frames: f64,
}

impl From<crate::audio::AudioQueueSnapshot> for AudioBufferState {
    fn from(snapshot: crate::audio::AudioQueueSnapshot) -> Self {
        Self {
            sample_rate: snapshot.sample_rate,
            channels: snapshot.channels,
            capacity_frames: snapshot.capacity_frames as f64,
            queued_frames: snapshot.queued_frames as f64,
            dropped_frames: snapshot.dropped_frames as f64,
        }
    }
}

impl From<crate::automation::ClockSnapshot> for TimelineState {
    fn from(snapshot: crate::automation::ClockSnapshot) -> Self {
        Self {
            current_time_ms: snapshot.current_time_ms,
            playback_rate: snapshot.playback_rate,
            playing: snapshot.playing,
        }
    }
}

fn validate_timeline_time(value: f64) -> Result<()> {
    if !value.is_finite() || !(0.0..=315_576_000_000.0).contains(&value) {
        return Err(Error::from_reason(
            "timeline time must be a finite non-negative millisecond value (maximum 10 years)",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_family = "wasm"), napi(object))]
pub struct WindowOptions {
    /// Retain and query the native tree without opening a platform window.
    pub headless: Option<bool>,
    pub title: Option<String>,
    /// The name used inside the macOS "Hide" and "Quit" menu items. Defaults to
    /// `title`. It does NOT set the title of the application menu itself: macOS
    /// takes that from the executable, and only a `.app` bundle changes it.
    pub app_name: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub resizable: Option<bool>,
    pub fullscreen: Option<bool>,
    /// Plain alpha transparency. Prefer `window_background` when you need blur.
    pub transparent: Option<bool>,
    /// Hide the native titlebar so the app can draw chrome under the traffic lights.
    pub titlebar_transparent: Option<bool>,
    /// `"opaque"` | `"transparent"` | `"blurred"`. `transparent: true` is the
    /// same as `"transparent"` when this is unset.
    pub window_background: Option<String>,
    pub traffic_light_x: Option<f64>,
    pub traffic_light_y: Option<f64>,
    /// Give the window focus when it opens. Ignored by GPUI on Linux.
    pub focus: Option<bool>,
    /// Show the window when it opens. Call `activateWindow()` to reveal it.
    /// Ignored by GPUI on Linux.
    pub show: Option<bool>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            headless: Some(false),
            title: Some("GPUI Vue".to_string()),
            app_name: None,
            width: Some(800.0),
            height: Some(600.0),
            min_width: None,
            min_height: None,
            resizable: Some(true),
            fullscreen: Some(false),
            transparent: Some(false),
            titlebar_transparent: Some(false),
            window_background: None,
            traffic_light_x: None,
            traffic_light_y: None,
            focus: Some(true),
            show: Some(true),
        }
    }
}

fn to_gpui_window_options(
    options: &WindowOptions,
    bounds: gpui::Bounds<gpui::Pixels>,
) -> gpui::WindowOptions {
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| "GPUI Vue".to_string());
    let titlebar_transparent = options.titlebar_transparent.unwrap_or(false);
    let traffic_light_position = match (options.traffic_light_x, options.traffic_light_y) {
        (Some(x), Some(y)) => Some(gpui::point(gpui::px(x as f32), gpui::px(y as f32))),
        _ => None,
    };
    let window_background = match options.window_background.as_deref() {
        Some("transparent") => gpui::WindowBackgroundAppearance::Transparent,
        Some("blurred") => gpui::WindowBackgroundAppearance::Blurred,
        Some("opaque") => gpui::WindowBackgroundAppearance::Opaque,
        _ if options.transparent.unwrap_or(false) => gpui::WindowBackgroundAppearance::Transparent,
        _ => gpui::WindowBackgroundAppearance::Opaque,
    };
    let window_min_size = match (options.min_width, options.min_height) {
        (Some(width), Some(height)) => {
            Some(gpui::size(gpui::px(width as f32), gpui::px(height as f32)))
        }
        _ => None,
    };
    let window_bounds = if options.fullscreen.unwrap_or(false) {
        gpui::WindowBounds::Fullscreen(bounds)
    } else {
        gpui::WindowBounds::Windowed(bounds)
    };
    gpui::WindowOptions {
        window_bounds: Some(window_bounds),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(title.into()),
            appears_transparent: titlebar_transparent,
            traffic_light_position,
        }),
        is_resizable: options.resizable.unwrap_or(true),
        window_background,
        window_min_size,
        focus: options.focus.unwrap_or(true),
        show: options.show.unwrap_or(true),
        ..Default::default()
    }
}

#[cfg(test)]
mod window_options_tests {
    use super::*;

    fn mapped(options: WindowOptions) -> gpui::WindowOptions {
        to_gpui_window_options(
            &options,
            gpui::Bounds {
                origin: gpui::point(gpui::px(0.0), gpui::px(0.0)),
                size: gpui::size(gpui::px(800.0), gpui::px(600.0)),
            },
        )
    }

    #[test]
    fn defaults_to_a_focused_visible_window() {
        let options = mapped(WindowOptions::default());
        assert!(options.focus);
        assert!(options.show);
    }

    #[test]
    fn unset_focus_and_show_remain_backwards_compatible() {
        let options = mapped(WindowOptions {
            focus: None,
            show: None,
            ..WindowOptions::default()
        });
        assert!(options.focus);
        assert!(options.show);
    }

    #[test]
    fn focus_and_visibility_are_independent() {
        let background = mapped(WindowOptions {
            focus: Some(false),
            ..WindowOptions::default()
        });
        assert!(!background.focus);
        assert!(background.show);

        let hidden = mapped(WindowOptions {
            show: Some(false),
            ..WindowOptions::default()
        });
        assert!(hidden.focus);
        assert!(!hidden.show);
    }
}

#[cfg(test)]
mod highlight_cache_tests {
    use super::*;

    fn tree_with_text() -> RetainedTree {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        tree.create_element(2, "text".to_string());
        tree.append_child(1, 2).unwrap();
        tree.set_text(2, "a fox and a fox".to_string());
        tree
    }

    fn query(text: &str) -> serde_json::Value {
        serde_json::json!({ "query": text })
    }

    fn declare(tree: &mut RetainedTree, value: &serde_json::Value) {
        tree.set_custom_prop(1, "highlight".to_string(), value.clone());
    }

    /// The whole reason `search_revision` exists. `highlight` is a custom prop,
    /// so keying the group list on `subtree_revision` means every keystroke
    /// re-walks and re-folds the subtree. The pointer comparison is the proof;
    /// a timing budget over a realistic app is far too coarse to catch it.
    #[test]
    fn a_query_change_reuses_the_group_list() {
        let theme = Theme::dark();
        let mut tree = tree_with_text();
        let mut cache = HashMap::new();

        declare(&mut tree, &query("f"));
        resolve_highlight(&mut cache, &tree, 1, &query("f"), &theme, false).expect("resolves");
        let first = Arc::as_ptr(&cache[&1].groups);

        declare(&mut tree, &query("fo"));
        resolve_highlight(&mut cache, &tree, 1, &query("fo"), &theme, false).expect("resolves");
        assert_eq!(
            Arc::as_ptr(&cache[&1].groups),
            first,
            "a query change must not rebuild the group list"
        );
    }

    /// Moving a find cursor changes no text and no matcher, so it must re-use
    /// the located matches. Colours and ordinals are decided at paint.
    #[test]
    fn a_cursor_move_reuses_the_located_matches() {
        let theme = Theme::dark();
        let mut tree = tree_with_text();
        let mut cache = HashMap::new();
        let spec = |active: u64| serde_json::json!({ "query": "fox", "activeIndex": active });

        declare(&mut tree, &spec(0));
        resolve_highlight(&mut cache, &tree, 1, &spec(0), &theme, true).expect("resolves");
        let matches = Arc::as_ptr(&cache[&1].context.matches);

        declare(&mut tree, &spec(1));
        let (context, changed) =
            resolve_highlight(&mut cache, &tree, 1, &spec(1), &theme, true).expect("resolves");
        assert_eq!(Arc::as_ptr(&context.matches), matches, "no rescan");
        assert_eq!(changed, None, "a cursor move is not a new result");
        assert_eq!(
            context.set.specs[0].active_index,
            Some(1),
            "spec still swapped"
        );
    }

    /// Editing the text must invalidate, or the wash paints over stale offsets.
    #[test]
    fn a_text_change_rebuilds_the_group_list() {
        let theme = Theme::dark();
        let mut tree = tree_with_text();
        let mut cache = HashMap::new();

        declare(&mut tree, &query("fox"));
        resolve_highlight(&mut cache, &tree, 1, &query("fox"), &theme, true).expect("resolves");
        let first = Arc::as_ptr(&cache[&1].groups);

        tree.set_text(2, "one fox only".to_string());
        let (_, changed) =
            resolve_highlight(&mut cache, &tree, 1, &query("fox"), &theme, true).expect("resolves");
        assert_ne!(Arc::as_ptr(&cache[&1].groups), first);
        assert_eq!(changed, Some(1), "two matches became one");
    }

    /// A review caught this: `reported` used to be written even with no
    /// listener, so mounting without `onHighlight` and adding it later reported
    /// nothing, forever.
    #[test]
    fn adding_the_listener_later_still_reports() {
        let theme = Theme::dark();
        let mut tree = tree_with_text();
        let mut cache = HashMap::new();

        declare(&mut tree, &query("fox"));
        let (_, changed) = resolve_highlight(&mut cache, &tree, 1, &query("fox"), &theme, false)
            .expect("resolves");
        assert_eq!(changed, None, "nothing to report without a listener");

        let (_, changed) =
            resolve_highlight(&mut cache, &tree, 1, &query("fox"), &theme, true).expect("resolves");
        assert_eq!(changed, Some(2), "the listener gets the current count");

        let (_, changed) =
            resolve_highlight(&mut cache, &tree, 1, &query("fox"), &theme, true).expect("resolves");
        assert_eq!(changed, None, "and only once");
    }
}

/// The `applyBatch` protocol. This is the surface JS talks to, so every rule it
/// relies on is asserted here against real JSON bytes rather than through a
/// hand-built `Vec<BatchOp>`.
#[cfg(test)]
mod batch_tests {
    use super::*;
    use crate::retained_tree::STYLE_SWEEP_FLOOR;
    use std::fmt::Write as _;

    fn apply(tree: &mut RetainedTree, json: &str) -> BatchResult<Vec<f64>> {
        apply_batch_to_tree(tree, json.as_bytes())
    }

    /// Everything a mutation can reach, so an unwanted partial apply shows up
    /// as a diff instead of hiding in a field the test forgot to read.
    fn describe(tree: &RetainedTree) -> String {
        let mut ids: Vec<_> = tree.elements.keys().copied().collect();
        ids.sort_unstable();
        let mut out = format!("root={:?}\n", tree.root_id);
        for id in ids {
            let element = &tree.elements[&id];
            let events = element.events.names();
            let mut props: Vec<_> = element.custom_props.iter().collect();
            props.sort_by_key(|(a, _)| *a);
            writeln!(
                out,
                "{id} type={} text={:?} style={:?} children={:?} parent={:?} events={events:?} props={props:?} rev={}/{}",
                element.element_type,
                element.content,
                element.style.as_deref(),
                element.children,
                element.parent,
                element.subtree_revision,
                element.search_revision,
            )
            .expect("writing to a String cannot fail");
        }
        out
    }

    /// The regression test for batch atomicity. `intern_style_payload` used to
    /// run inside the apply loop, so this batch created the element, set its
    /// text, and only then threw — leaving JS to retry against a tree that had
    /// already moved.
    #[test]
    fn a_malformed_style_applies_nothing_at_all() {
        let mut tree = RetainedTree::new();
        apply(&mut tree, r#"[["createElement",1,"div"],["setRoot",1]]"#).expect("valid batch");
        let before = describe(&tree);
        let styles_before = tree.styles.len();

        let error = apply(
            &mut tree,
            r#"[["createElement",2,"div"],["setText",2,"changed"],["setStyle",2,123]]"#,
        )
        .expect_err("a malformed style must reject the batch");

        assert_eq!(describe(&tree), before, "the tree must be untouched");
        assert_eq!(
            tree.styles.len(),
            styles_before,
            "the failed batch must not leave styles interned"
        );
        assert!(error.contains("setStyle"), "{error}");
    }

    /// A style that fails halfway through a long batch is unfindable without
    /// its index; serde reports a byte offset, which names nothing.
    #[test]
    fn a_style_error_names_its_op_index() {
        let mut tree = RetainedTree::new();
        let error = apply(
            &mut tree,
            r#"[["createElement",1,"div"],["setStyle",1,{"color":"red"}],["setStyle",1,{"color":5}]]"#,
        )
        .expect_err("a bad style rejects the batch");
        assert!(
            error.starts_with("Batch op 2 setStyle parse error:"),
            "{error}"
        );
    }

    #[test]
    fn a_legacy_string_encoded_style_still_applies() {
        let mut tree = RetainedTree::new();
        apply(
            &mut tree,
            r#"[["createElement",1,"div"],["setStyle",1,"{\"color\":\"red\"}"]]"#,
        )
        .expect("a JSON-string style is legacy, not invalid");
        assert_eq!(
            tree.elements[&1].style.as_deref().unwrap().color.as_deref(),
            Some("red")
        );
    }

    /// `null` is not "no style". Treating it as `{}` would silently clear every
    /// declared property instead of telling JS it sent something wrong.
    #[test]
    fn a_null_style_is_an_error() {
        let mut tree = RetainedTree::new();
        let error = apply(
            &mut tree,
            r#"[["createElement",1,"div"],["setStyle",1,null]]"#,
        )
        .expect_err("null is not a style");
        assert!(
            error.contains("Batch op 1 setStyle parse error:"),
            "{error}"
        );
        assert!(tree.elements.is_empty(), "and the batch stays atomic");
    }

    /// Skipping an unknown opcode would let a JS/Rust version skew desync the
    /// tree quietly. It has to throw.
    #[test]
    fn an_unknown_opcode_is_an_error() {
        let mut tree = RetainedTree::new();
        let error = apply(&mut tree, r#"[["teleportElement",1]]"#).expect_err("unknown opcode");
        assert!(error.contains("unknown operation"), "{error}");
        assert!(tree.elements.is_empty());
    }

    #[test]
    fn a_cycle_created_across_batch_ops_is_rejected_atomically() {
        let mut tree = RetainedTree::new();
        let error = apply(
            &mut tree,
            r#"[["createElement",1,"div"],["createElement",2,"div"],["appendChild",1,2],["appendChild",2,1]]"#,
        )
        .expect_err("a parent cycle must reject the batch");
        assert!(error.contains("would create a cycle"), "{error}");
        assert!(tree.elements.is_empty(), "validation must precede mutation");
    }

    /// Every op that takes an id must validate it. A fractional or oversized id
    /// would truncate into a *different* element, which is a silent desync.
    #[test]
    fn an_invalid_id_is_rejected_in_every_id_position() {
        let templates = [
            r#"[["createElement",ID,"div"]]"#,
            r#"[["destroyElement",ID]]"#,
            r#"[["appendChild",ID,2]]"#,
            r#"[["appendChild",1,ID]]"#,
            r#"[["removeChild",ID,2]]"#,
            r#"[["removeChild",1,ID]]"#,
            r#"[["insertBefore",ID,2,3]]"#,
            r#"[["insertBefore",1,ID,3]]"#,
            r#"[["insertBefore",1,2,ID]]"#,
            r#"[["setStyle",ID,{}]]"#,
            r#"[["setText",ID,"x"]]"#,
            r#"[["setEventListener",ID,"click",true]]"#,
            r#"[["setRoot",ID]]"#,
            r#"[["setCustomProp",ID,"k",1]]"#,
            r#"[["setCustomPropValue",ID,"k",1]]"#,
        ];
        // 1e999 overflows f64, 9007199254740992 is Number.MAX_SAFE_INTEGER + 1.
        let bad_ids = ["-1", "1.5", "9007199254740992", "1e999"];

        for template in templates {
            for bad in bad_ids {
                let json = template.replace("ID", bad);
                let mut tree = RetainedTree::new();
                let error = apply(&mut tree, &json).expect_err(&format!("{json} must be rejected"));
                assert!(error.contains("Batch op 0"), "{json}: {error}");
                assert!(tree.elements.is_empty(), "{json} mutated the tree");
                assert_eq!(tree.root_id, None, "{json} mutated the root");
            }
        }
    }

    /// The reconciler sends a bool; hand-written batches send 0 or 1. Anything
    /// else used to mean `true`, so `-1` silently registered a listener.
    #[test]
    fn has_handler_takes_a_bool_or_a_non_negative_integer() {
        for (payload, expected) in [("true", true), ("false", false), ("1", true), ("0", false)] {
            let mut tree = RetainedTree::new();
            let json =
                format!(r#"[["createElement",1,"div"],["setEventListener",1,"click",{payload}]]"#);
            apply(&mut tree, &json).expect("bool or non-negative integer");
            assert_eq!(
                tree.elements[&1].events.contains("click"),
                expected,
                "hasHandler {payload}"
            );
        }

        for payload in ["-1", "0.5"] {
            let mut tree = RetainedTree::new();
            let json =
                format!(r#"[["createElement",1,"div"],["setEventListener",1,"click",{payload}]]"#);
            apply(&mut tree, &json).expect_err(&format!("hasHandler {payload} is not a bool"));
        }
    }

    #[test]
    fn a_malformed_op_tuple_is_an_error() {
        let cases = [
            ("[42]", "a non-array op"),
            (r#"[["createElement",1]]"#, "a missing argument"),
            (r#"[[7,1,"div"]]"#, "a non-string op name"),
        ];
        for (json, what) in cases {
            let mut tree = RetainedTree::new();
            let error = apply(&mut tree, json).expect_err(what);
            assert!(
                error.starts_with("Failed to parse batch:"),
                "{what}: {error}"
            );
            assert!(tree.elements.is_empty(), "{what} mutated the tree");
        }
    }

    /// The single-op entry points sweep too. Without that,
    /// `for (...) renderer.setStyle(1, ...)` grows the table forever.
    #[test]
    fn repeated_direct_set_style_keeps_the_table_bounded() {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        for frame in 0..10_000 {
            let payload = format!(r#"{{"left":{frame}}}"#);
            tree.set_style_json(1, payload.as_bytes())
                .expect("valid style");
            assert!(
                tree.styles.len() <= STYLE_SWEEP_FLOOR,
                "frame {frame} left {} styles interned",
                tree.styles.len()
            );
        }
    }

    /// Interning keys on raw bytes, so re-ordered keys are two `Arc`s. They are
    /// still the same style, and a repaint per key order would be a real cost
    /// on any app that builds style objects conditionally.
    #[test]
    fn a_reordered_style_does_not_repaint() {
        let mut tree = RetainedTree::new();
        apply(
            &mut tree,
            r#"[["createElement",1,"div"],["setStyle",1,{"color":"red","left":10}]]"#,
        )
        .expect("valid batch");
        let revision = tree.elements[&1].subtree_revision;

        apply(&mut tree, r#"[["setStyle",1,{"left":10,"color":"red"}]]"#).expect("valid batch");
        assert_eq!(
            tree.elements[&1].subtree_revision, revision,
            "the same style in another key order is not a change"
        );
    }

    /// Three ways an interned style loses its last element reference.
    #[test]
    fn a_style_is_released_when_nothing_references_it() {
        let mut tree = RetainedTree::new();
        apply(&mut tree, r#"[["createElement",1,"div"]]"#).expect("valid batch");

        // Set on an id that does not exist: nothing keeps the style alive.
        apply(&mut tree, r#"[["setStyle",99,{"color":"red"}]]"#).expect("missing ids are ignored");
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 0, "a style nobody took must be released");

        apply(&mut tree, r#"[["setStyle",1,{"color":"red"}]]"#).expect("valid batch");
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 1);

        // Replaced.
        apply(&mut tree, r#"[["setStyle",1,{"color":"blue"}]]"#).expect("valid batch");
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 1, "the replaced style must be released");

        // Destroyed.
        apply(&mut tree, r#"[["destroyElement",1]]"#).expect("valid batch");
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 0);
    }
}
