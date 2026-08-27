//! GPUIX retained renderer for napi desktop hosts and GPUI's browser platform.
//!
//! Mutation-based API: Vue's reconciler sends individual mutations
//! (createElement, appendChild, setStyle, etc.) instead of a full JSON tree.
//! Rust maintains a RetainedTree and rebuilds GPUI elements from it each frame.
//!
//! Desktop lifecycle:
//!   const renderer = new GpuiRenderer(eventCallback)
//!   renderer.init({ title: 'My App', width: 800, height: 600 })
//!   renderer.createElement(1, "div")     // mutations from Vue reconciler
//!   renderer.appendChild(0, 1)
//!   renderer.commitMutations()           // signal batch complete
//!   setTimeout(function loop() {         // drive AppKit on macOS
//!     if (!renderer.tick()) process.exit(0)
//!     setTimeout(loop, 8)
//!   })
#[cfg(target_family = "wasm")]
mod wasm;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use futures::{channel::mpsc, StreamExt as _};
use gpui::AppContext as _;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use napi::bindgen_prelude::*;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use napi_derive::napi;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
#[cfg(any(target_os = "macos", target_family = "wasm"))]
use std::rc::Rc;
#[cfg(target_family = "wasm")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
use std::time::Duration;
use web_time::Instant;

use crate::custom_elements::{CustomElementRegistry, CustomRenderContext};
use crate::element_tree::EventPayload;
use crate::retained_tree::{RetainedTree, StyleTable};
use crate::style::StyleDesc;
use crate::text::{selectable_text, selection_frame_reset, SharedSelection};
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

/// Parse a CSS font-weight value (string or number) into a GPUI FontWeight.
/// Accepts named keywords ("bold", "semibold"), numeric strings ("700"),
/// and raw numbers (700). Falls back to 400 (normal) for unrecognized values.
pub(crate) fn parse_font_weight(value: &crate::style::FontWeightValue) -> gpui::FontWeight {
    match value {
        crate::style::FontWeightValue::Num(n) => gpui::FontWeight((*n as f32).clamp(1.0, 1000.0)),
        crate::style::FontWeightValue::Str(s) => {
            let lower = s.trim().to_ascii_lowercase();
            match lower.as_str() {
                "100" | "thin" => gpui::FontWeight(100.0),
                "200" | "extralight" | "extra-light" => gpui::FontWeight(200.0),
                "300" | "light" => gpui::FontWeight(300.0),
                "400" | "normal" => gpui::FontWeight(400.0),
                "500" | "medium" => gpui::FontWeight(500.0),
                "600" | "semibold" | "semi-bold" => gpui::FontWeight(600.0),
                "700" | "bold" => gpui::FontWeight(700.0),
                "800" | "extrabold" | "extra-bold" => gpui::FontWeight(800.0),
                "900" | "black" => gpui::FontWeight(900.0),
                _ => lower
                    .parse::<f32>()
                    .map(|n| gpui::FontWeight(n.clamp(1.0, 1000.0)))
                    .unwrap_or(gpui::FontWeight(400.0)),
            }
        }
    }
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
        current_ms: stats.current_ms.map(|ms| ms as f64),
        p90_ms: stats.p90_ms.map(|ms| ms as f64),
        p99_ms: stats.p99_ms.map(|ms| ms as f64),
        max_ms: stats.max_ms.map(|ms| ms as f64),
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

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
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

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
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
    },
    GetScrollOffset {
        id: u64,
        response: SyncSender<Option<[f64; 2]>>,
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
    let mut sender_slot = sender_slot.lock().unwrap();
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
                let window_options = options.clone();
                let initialized_for_close = initialized.clone();
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
                            *initialized_for_close.lock().unwrap() = false;
                        }
                    })
                    .detach();
                    cx.spawn(async move |cx| {
                        run_ui_commands(commands, window, cx).await;
                    })
                    .detach();
                    *initialized.lock().unwrap() = true;
                    cx.activate(true);
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
) -> anyhow::Result<()> {
    window.update(cx, |_view, window, cx| {
        cx.notify();
        window.refresh();
    })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
async fn run_ui_commands(
    mut commands: mpsc::UnboundedReceiver<UiCommand>,
    window: gpui::WindowHandle<GpuiView>,
    cx: &mut gpui::AsyncApp,
) {
    let mut closed_by_command = false;
    while let Some(command) = commands.next().await {
        let result = match command {
            UiCommand::Invalidate => refresh_ui_window(window, cx),
            UiCommand::Close { response } => {
                let result = window
                    .update(cx, |_view, window, _cx| window.remove_window())
                    .map(|_| ())
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
            UiCommand::ScrollToItem { id, index } => {
                if !VIRTUAL_LIST_STATES.with(|cell| {
                    let states = cell.borrow();
                    let Some(state) = states.get(&id) else {
                        return false;
                    };
                    state.scroll_to(gpui::ListOffset {
                        item_ix: index,
                        offset_in_item: gpui::px(0.0),
                    });
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
            UiCommand::GetWindowSize { response } => {
                window.update(cx, move |_view, window, _cx| {
                    let size = window.viewport_size();
                    response
                        .send(WindowSize {
                            width: f32::from(size.width) as f64,
                            height: f32::from(size.height) as f64,
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
                            .map(|_| ())
                            .map_err(|error| format!("{error:#}")),
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
                    .and_then(|result| result.map_err(anyhow::Error::msg));
                response
                    .send(
                        result
                            .as_ref()
                            .map(|_| ())
                            .map_err(|error| format!("{error:#}")),
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

/// The main GPUI renderer exposed to Node.js.
#[cfg_attr(not(target_family = "wasm"), napi)]
pub struct GpuiRenderer {
    event_callback: Mutex<Option<EventCallback>>,
    tree: Arc<Mutex<RetainedTree>>,
    initialized: Arc<Mutex<bool>>,
    headless: Mutex<bool>,
    window_size: Mutex<WindowSize>,
    /// Shared with the view so timeline controls avoid a UI-thread round trip.
    clock: crate::automation::AutomationClock,
    audio: Mutex<crate::audio::AudioFrameQueue>,
    /// Shared with GpuiView so napi methods can read the live selection
    /// without an App context. Paint and napi calls can use different threads.
    selection: SharedSelection,
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    ui_commands: Mutex<Option<mpsc::UnboundedSender<UiCommand>>>,
    #[cfg(target_family = "wasm")]
    web_renderer_id: u64,
}

#[cfg_attr(not(target_family = "wasm"), napi)]
impl GpuiRenderer {
    fn event_callback_for_view(&self) -> Option<EventCallback> {
        self.event_callback.lock().unwrap().clone()
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn send_ui_command(&self, command: UiCommand) -> Result<()> {
        self.ui_commands
            .lock()
            .unwrap()
            .as_ref()
            .ok_or_else(|| Error::from_reason("GPUI application is not initialized"))?
            .unbounded_send(command)
            .map_err(|_| Error::from_reason("The GPUI UI thread is not running"))
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn dispatch_mouse_input(&self, input: MouseInput) -> Result<()> {
        let (response_sender, response_receiver) = sync_channel(1);
        self.send_ui_command(UiCommand::DispatchMouse {
            input,
            response: response_sender,
        })?;
        recv_ui_response(response_receiver, "the GPUI UI command")?.map_err(Error::from_reason)
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn dispatch_key_input(&self, input: KeyInput) -> Result<()> {
        let (response_sender, response_receiver) = sync_channel(1);
        self.send_ui_command(UiCommand::DispatchKey {
            input,
            response: response_sender,
        })?;
        recv_ui_response(response_receiver, "the GPUI key command")?.map_err(Error::from_reason)
    }

    fn automation_bounds(&self) -> Result<HashMap<u64, crate::automation::ElementBounds>> {
        #[cfg(target_os = "macos")]
        return Ok(crate::automation::all_bounds());

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetAutomationBounds { response })?;
            return recv_ui_response(receiver, "the automation bounds query");
        }

        #[cfg(target_family = "wasm")]
        return Ok(crate::automation::all_bounds());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    fn element_bounds(&self, id: u64) -> Result<Option<crate::automation::ElementBounds>> {
        #[cfg(target_os = "macos")]
        return Ok(crate::automation::get_bounds(id));

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetElementBounds { id, response })?;
            return recv_ui_response(receiver, "the element bounds query");
        }

        #[cfg(target_family = "wasm")]
        return Ok(crate::automation::get_bounds(id));

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = id;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn control_clock(&self, control: ClockControl) -> Result<f64> {
        let (response, receiver) = sync_channel(1);
        self.send_ui_command(UiCommand::ControlClock { control, response })?;
        recv_ui_response(receiver, "the automation clock command")
    }

    fn request_invalidate(&self) -> Result<()> {
        if *self.headless.lock().unwrap() {
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        return invalidate_window();

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::Invalidate);

        #[cfg(target_family = "wasm")]
        return invalidate_web_window(self.web_renderer_id);

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason(
            "The production GPUI Vue renderer does not support this operating system",
        ))
    }

    #[cfg_attr(not(target_family = "wasm"), napi(constructor))]
    pub fn new(event_callback: Option<NativeEventCallback>) -> Self {
        #[cfg(not(target_family = "wasm"))]
        let _ = env_logger::try_init();
        #[cfg(target_family = "wasm")]
        static NEXT_WEB_RENDERER_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            event_callback: Mutex::new(event_callback.map(event_callback_from_native)),
            tree: Arc::new(Mutex::new(RetainedTree::new())),
            initialized: Arc::new(Mutex::new(false)),
            headless: Mutex::new(false),
            window_size: Mutex::new(WindowSize {
                width: 800.0,
                height: 600.0,
            }),
            clock: crate::automation::AutomationClock::new(),
            audio: Mutex::new(crate::audio::AudioFrameQueue::default()),
            selection: SharedSelection::default(),
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
            ui_commands: Mutex::new(None),
            #[cfg(target_family = "wasm")]
            web_renderer_id: NEXT_WEB_RENDERER_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Initialize GPUI using the native event-loop architecture for this OS.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn init(&self, options: Option<WindowOptions>) -> Result<()> {
        if *self.initialized.lock().unwrap() {
            return Err(Error::from_reason("Renderer is already initialized"));
        }

        let headless = options
            .as_ref()
            .and_then(|options| options.headless)
            .unwrap_or(false);
        if let Some(options) = &options {
            let mut size = self.window_size.lock().unwrap();
            size.width = options.width.unwrap_or(size.width);
            size.height = options.height.unwrap_or(size.height);
        }
        if headless {
            *self.headless.lock().unwrap() = true;
            *self.initialized.lock().unwrap() = true;
            return Ok(());
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = options;
            return Err(Error::from_reason(
                "The production GPUI Vue renderer does not support this operating system",
            ));
        }

        #[cfg(target_os = "macos")]
        return self.init_macos(options);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.init_threaded(options);

        #[cfg(target_family = "wasm")]
        return self.init_web(options);
    }

    #[cfg(target_family = "wasm")]
    fn init_web(&self, options: Option<WindowOptions>) -> Result<()> {
        static WEB_INIT: std::sync::Once = std::sync::Once::new();
        WEB_INIT.call_once(gpui_platform::web_init);

        let options = options.unwrap_or_default();
        let width = options.width.unwrap_or(800.0);
        let height = options.height.unwrap_or(600.0);
        let title = options
            .title
            .clone()
            .unwrap_or_else(|| "GPUI Vue".to_string());
        let window_options = options.clone();
        let tree = self.tree.clone();
        let callback = self.event_callback_for_view();
        let selection = self.selection.clone();
        let clock = self.clock.clone();
        let initialized = self.initialized.clone();
        let window_slot = Rc::new(RefCell::new(None));
        let window_slot_for_launch = window_slot.clone();

        let application = gpui_platform::single_threaded_web();
        let application_handle = application.run_embedded(move |cx: &mut gpui::App| {
            init_key_bindings(cx);
            crate::custom_elements::input::init(cx);
            let bounds = gpui::Bounds::centered(
                None,
                gpui::size(gpui::px(width as f32), gpui::px(height as f32)),
                cx,
            );
            match cx.open_window(
                to_gpui_window_options(&window_options, bounds),
                |_window, cx| cx.new(|_| GpuiView::new(tree, callback, title, selection, clock)),
            ) {
                Ok(window) => {
                    let window_id = window.window_id();
                    let initialized_for_close = initialized.clone();
                    cx.on_window_closed(move |_cx, closed_id| {
                        if closed_id == window_id {
                            *initialized_for_close.lock().unwrap() = false;
                        }
                    })
                    .detach();
                    *window_slot_for_launch.borrow_mut() = Some(window);
                    cx.activate(true);
                }
                Err(error) => {
                    *initialized.lock().unwrap() = false;
                    log::error!("Failed to open the GPUI web window: {error}");
                }
            }
        });

        WEB_APPS.with(|apps| {
            apps.borrow_mut().insert(
                self.web_renderer_id,
                WebAppEntry {
                    app: Rc::new(application_handle),
                    window: window_slot,
                },
            );
        });
        *self.initialized.lock().unwrap() = true;
        self.event_callback.lock().unwrap().take();
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn init_macos(&self, options: Option<WindowOptions>) -> Result<()> {
        let options = options.unwrap_or_default();

        {
            let initialized = self.initialized.lock().unwrap();
            if *initialized {
                return Err(Error::from_reason("Renderer is already initialized"));
            }
        }
        if MAC_PLATFORM.with(|platform| platform.borrow().is_some()) {
            return Err(Error::from_reason(
                "A GPUI application already exists on this thread",
            ));
        }

        let width = options.width.unwrap_or(800.0);
        let height = options.height.unwrap_or(600.0);
        let title = options
            .title
            .clone()
            .unwrap_or_else(|| "GPUI Vue".to_string());
        let app_name = options.app_name.clone().unwrap_or_else(|| title.clone());
        let window_options = options.clone();

        let platform = Rc::new(gpui_macos::MacPlatform::new_embedded());

        let tree = self.tree.clone();
        let callback = self.event_callback_for_view();

        let selection = self.selection.clone();
        let clock = self.clock.clone();
        let opened_window = Rc::new(RefCell::new(None));
        let startup_error = Rc::new(RefCell::new(None));
        let opened_window_for_app = opened_window.clone();
        let startup_error_for_app = startup_error.clone();
        // bun/node is not a .app. A Dock icon with no window cannot relaunch.
        // Last window close quits AppKit; tick() returns false and JS exits.
        let app = gpui::Application::with_platform(platform.clone())
            .with_quit_mode(gpui::QuitMode::LastWindowClosed);
        let app_handle = app.run_embedded(move |cx: &mut gpui::App| {
            init_key_bindings(cx);
            crate::custom_elements::input::init(cx);
            // After the other bindings: `set_menus` reads key equivalents out of
            // the keymap, so every binding must exist before it runs.
            crate::app_menu::init(&app_name, cx);
            let bounds = gpui::Bounds::centered(
                None,
                gpui::size(gpui::px(width as f32), gpui::px(height as f32)),
                cx,
            );

            match cx.open_window(
                to_gpui_window_options(&window_options, bounds),
                |_window, cx| {
                    cx.new(|_| {
                        GpuiView::new(
                            tree.clone(),
                            callback.clone(),
                            title,
                            selection.clone(),
                            clock.clone(),
                        )
                    })
                },
            ) {
                Ok(window_handle) => {
                    *opened_window_for_app.borrow_mut() = Some(window_handle);
                    cx.activate(true);
                }
                Err(error) => {
                    *startup_error_for_app.borrow_mut() = Some(error.to_string());
                }
            }
        });

        let startup_result = match startup_error.borrow_mut().take() {
            Some(error) => Err(Error::from_reason(format!(
                "Failed to open the GPUI window: {error}"
            ))),
            None => opened_window
                .borrow_mut()
                .take()
                .ok_or_else(|| Error::from_reason("GPUI did not open the application window")),
        };
        let window_handle = match startup_result {
            Ok(window_handle) => window_handle,
            Err(error) => {
                app_handle.update(|cx| cx.quit());
                if platform.pump_events() {
                    MAC_PLATFORM.with(|stored| {
                        *stored.borrow_mut() = Some(platform.clone());
                    });
                }
                return Err(error);
            }
        };

        MAC_PLATFORM.with(|stored| {
            *stored.borrow_mut() = Some(platform);
        });
        GPUI_APP.with(|a| {
            *a.borrow_mut() = Some(app_handle);
        });
        GPUI_WINDOW.with(|w| {
            *w.borrow_mut() = Some(window_handle);
        });

        *self.initialized.lock().unwrap() = true;
        self.event_callback.lock().unwrap().take();
        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn init_threaded(&self, options: Option<WindowOptions>) -> Result<()> {
        let options = options.unwrap_or_default();
        if *self.initialized.lock().unwrap() {
            return Err(Error::from_reason("Renderer is already initialized"));
        }

        let title = options
            .title
            .clone()
            .unwrap_or_else(|| "GPUI Vue".to_string());
        let (command_sender, command_receiver) = mpsc::unbounded();
        let (open_sender, open_receiver) = sync_channel(1);
        threaded_app_sender()?
            .unbounded_send(ThreadedAppCommand::Open(OpenWindowRequest {
                tree: self.tree.clone(),
                callback: self.event_callback_for_view(),
                title,
                options,
                selection: self.selection.clone(),
                clock: self.clock.clone(),
                initialized: self.initialized.clone(),
                commands: command_receiver,
                response: open_sender,
            }))
            .map_err(|_| Error::from_reason("The shared GPUI UI thread is not running"))?;

        match open_receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(Error::from_reason(error)),
            Err(RecvTimeoutError::Timeout) => {
                return Err(Error::from_reason(
                    "Timed out waiting for GPUI to open the window",
                ));
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::from_reason(
                    "The shared GPUI UI thread stopped while opening the window",
                ));
            }
        }

        *self.ui_commands.lock().unwrap() = Some(command_sender);
        self.event_callback.lock().unwrap().take();
        Ok(())
    }

    // ── Mutation API ─────────────────────────────────────────────────

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn create_element(&self, id: f64, element_type: String) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.create_element(id, element_type);
        Ok(())
    }

    /// Destroy an element and all descendants. Returns array of destroyed IDs
    /// so JS can clean up event handlers for the entire subtree.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn destroy_element(&self, id: f64) -> Result<Vec<f64>> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock().unwrap();
        let destroyed = tree.destroy_element(id);
        Ok(destroyed.iter().map(|&id| id as f64).collect())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn append_child(&self, parent_id: f64, child_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.append_child(parent_id, child_id);
        Ok(())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn remove_child(&self, parent_id: f64, child_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.remove_child(parent_id, child_id);
        Ok(())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn insert_before(&self, parent_id: f64, child_id: f64, before_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let before_id = to_element_id(before_id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.insert_before(parent_id, child_id, before_id);
        Ok(())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_style(&self, id: f64, style_json: String) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree
            .lock()
            .unwrap()
            .set_style_json(id, style_json.as_bytes())
            .map_err(|error| Error::from_reason(format!("Failed to parse style: {error}")))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_text(&self, id: f64, content: String) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.set_text(id, content);
        Ok(())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_event_listener(&self, id: f64, event_type: String, has_handler: bool) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.set_event_listener(id, event_type, has_handler);
        Ok(())
    }

    /// Set the root element (called from appendChildToContainer).
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_root(&self, id: f64) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock().unwrap();
        tree.root_id = Some(id);
        Ok(())
    }

    /// Set a custom prop on an element (for non-div/text elements like input, editor, diff).
    /// Key is the prop name, value is JSON-encoded.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_custom_prop(&self, id: f64, key: String, value_json: String) -> Result<()> {
        let id = to_element_id(id)?;
        let value: serde_json::Value = serde_json::from_str(&value_json)
            .map_err(|e| Error::from_reason(format!("Failed to parse custom prop value: {}", e)))?;
        let mut tree = self.tree.lock().unwrap();
        tree.set_custom_prop(id, key, value);
        Ok(())
    }

    /// Get a custom prop value from an element. Returns JSON string or null.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_custom_prop(&self, id: f64, key: String) -> Result<Option<String>> {
        let id = to_element_id(id)?;
        let tree = self.tree.lock().unwrap();
        Ok(tree
            .get_custom_prop(id, &key)
            .map(|v| serde_json::to_string(v).unwrap_or_default()))
    }

    /// Signal that a batch of mutations is complete. Triggers re-render.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn commit_mutations(&self) -> Result<()> {
        self.request_invalidate()
    }

    /// Apply a batch of mutations in a single FFI call.
    ///
    /// Accepts a JSON array of mutation tuples. Each tuple is an array where
    /// the first element is the operation name (string) and remaining elements
    /// are the arguments:
    ///
    ///   ["createElement",    id, "type"]
    ///   ["destroyElement",   id]
    ///   ["appendChild",      parentId, childId]
    ///   ["removeChild",      parentId, childId]
    ///   ["insertBefore",     parentId, childId, beforeId]
    ///   ["setStyle",         id, { ...style } | "{styleJson}"]
    ///   ["setText",          id, "content"]
    ///   ["setEventListener", id, "eventType", true|false]
    ///   ["setRoot",          id]
    ///   ["setCustomProp",      id, "key", value | "{valueJson}"]
    ///   ["setCustomPropValue", id, "key", value]
    ///
    /// Returns accumulated destroyed IDs from all destroyElement ops.
    /// Acquires the tree mutex ONCE for the entire batch.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn apply_batch(&self, json: String) -> Result<Vec<f64>> {
        let mut tree = self.tree.lock().unwrap();
        let destroyed =
            apply_batch_to_tree(&mut tree, json.as_bytes()).map_err(Error::from_reason)?;
        drop(tree);
        self.request_invalidate()?;
        Ok(destroyed)
    }

    // ── Frame loop ───────────────────────────────────────────────────

    /// Pump the native event loop. Returns false after the last window closes.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn tick(&self) -> Result<bool> {
        let initialized = *self.initialized.lock().unwrap();
        if !initialized {
            return Err(Error::from_reason(
                "Renderer not initialized. Call init() first.",
            ));
        }
        if *self.headless.lock().unwrap() {
            return Ok(true);
        }

        #[cfg(target_os = "macos")]
        {
            let running = MAC_PLATFORM.with(|p| {
                p.borrow()
                    .as_ref()
                    .map(|platform| platform.pump_events())
                    .unwrap_or(false)
            });
            return Ok(running);
        }

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return Ok(true);

        #[cfg(target_family = "wasm")]
        return Ok(true);

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason(
            "The production GPUI Vue renderer does not support this operating system",
        ))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn is_initialized(&self) -> bool {
        *self.initialized.lock().unwrap()
    }

    /// Whether JavaScript must drive the native event loop with tick().
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn requires_tick(&self) -> bool {
        !*self.headless.lock().unwrap() && cfg!(target_os = "macos")
    }

    /// The paintable size of the window in logical pixels, excluding any
    /// platform title bar. This used to answer a hardcoded 800x600, so anything
    /// that turned a mouse position into layout coordinates pointed at the
    /// wrong place on every window that was not exactly that size.
    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn get_window_size(&self) -> Result<WindowSize> {
        #[cfg(target_os = "macos")]
        return update_window(|_view, window, _cx| {
            let size = window.viewport_size();
            WindowSize {
                width: f32::from(size.width) as f64,
                height: f32::from(size.height) as f64,
            }
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetWindowSize { response })?;
            return recv_ui_response(receiver, "the window size query");
        }

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, |_view, window, _cx| {
            let size = window.viewport_size();
            WindowSize {
                width: f32::from(size.width) as f64,
                height: f32::from(size.height) as f64,
            }
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"));

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason(
            "The production GPUIX renderer does not support this operating system",
        ))
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn get_window_insets(&self) -> Result<WindowInsets> {
        #[cfg(target_os = "macos")]
        return update_window(|_view, window, _cx| WindowInsets::from_gpui(window.insets()));

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, |_view, window, _cx| {
            WindowInsets::from_gpui(window.insets())
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"));

        #[cfg(not(any(target_os = "macos", target_family = "wasm")))]
        Ok(WindowInsets::default())
    }

    /// Stop the native event loop and release the window. This explicit
    /// lifecycle hook lets Vue unmount or hot-remount without keeping Node's
    /// process alive.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn close(&self) -> Result<()> {
        if !*self.initialized.lock().unwrap() {
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            GPUI_APP.with(|app| {
                if let Some(app) = app.borrow().as_ref() {
                    app.update(|cx| cx.quit());
                }
            });
            GPUI_WINDOW.with(|window| window.borrow_mut().take());
            GPUI_APP.with(|app| app.borrow_mut().take());
            MAC_PLATFORM.with(|platform| platform.borrow_mut().take());
        }

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        if !*self.headless.lock().unwrap() {
            if let Some(sender) = self.ui_commands.lock().unwrap().take() {
                let (close_sender, close_receiver) = sync_channel(1);
                sender
                    .unbounded_send(UiCommand::Close {
                        response: close_sender,
                    })
                    .map_err(|_| Error::from_reason("The GPUI UI thread is not running"))?;
                match close_receiver.recv_timeout(Duration::from_secs(2)) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => return Err(Error::from_reason(error)),
                    Err(RecvTimeoutError::Timeout) => {
                        return Err(Error::from_reason(
                            "Timed out waiting for GPUI to close the window",
                        ));
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err(Error::from_reason(
                            "The GPUI UI thread stopped while closing the window",
                        ));
                    }
                }
            }
        }

        #[cfg(target_family = "wasm")]
        if let Some(entry) = WEB_APPS.with(|apps| apps.borrow_mut().remove(&self.web_renderer_id)) {
            if let Some(window) = *entry.window.borrow() {
                entry.app.update(|cx| {
                    window
                        .update(cx, |_view, window, _cx| window.remove_window())
                        .ok();
                });
            }
        }

        *self.headless.lock().unwrap() = false;
        *self.initialized.lock().unwrap() = false;
        Ok(())
    }

    /// `"hidden"` | `"minimal"` | `"full"`. Paints into the scene after layout.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_debug_frame_overlay(&self, mode: String) -> Result<String> {
        let mode = parse_debug_frame_overlay_mode(&mode)?;
        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, _cx| {
            window.set_debug_frame_overlay_mode(mode);
            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            self.send_ui_command(UiCommand::SetDebugFrameOverlay(mode))?;
            return self.debug_frame_overlay_mode();
        }

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| {
            window.set_debug_frame_overlay_mode(mode);
            cx.notify();
            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"));

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    /// Hidden → minimal → full → hidden.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn cycle_debug_frame_overlay(&self) -> Result<String> {
        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, _cx| {
            window.cycle_debug_frame_overlay_mode();
            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::CycleDebugFrameOverlay { response })?;
            return recv_ui_response(receiver, "the debug frame overlay query");
        }

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, |_view, window, cx| {
            window.cycle_debug_frame_overlay_mode();
            cx.notify();
            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"));

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_debug_frame_overlay(&self) -> Result<String> {
        self.debug_frame_overlay_mode()
    }

    fn debug_frame_overlay_mode(&self) -> Result<String> {
        #[cfg(target_os = "macos")]
        return update_window(|_view, window, _cx| {
            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetDebugFrameOverlay { response })?;
            recv_ui_response(receiver, "the debug frame overlay query")
        }

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, |_view, window, _cx| {
            debug_frame_overlay_mode_name(window.debug_frame_overlay_mode()).to_string()
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"));

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    /// Clears the last 1000 draw samples. Frame count stays.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn reset_debug_frame_overlay_stats(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window(|_view, window, _cx| {
            window.reset_debug_frame_overlay_stats();
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::ResetDebugFrameOverlayStats);

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, |_view, window, _cx| {
            window.reset_debug_frame_overlay_stats();
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    /// Same numbers as the on-screen overlay: current, p90, p99, max, frames.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn get_debug_frame_overlay_stats(&self) -> Result<DebugFrameOverlayStats> {
        #[cfg(target_os = "macos")]
        return update_window(|_view, window, _cx| {
            debug_frame_overlay_stats_js(window.debug_frame_overlay_stats())
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetDebugFrameOverlayStats { response })?;
            match receiver.recv_timeout(Duration::from_secs(2)) {
                Ok(stats) => Ok(stats),
                Err(RecvTimeoutError::Timeout) => Err(Error::from_reason(
                    "Timed out after 2 seconds waiting for debug frame overlay stats",
                )),
                Err(RecvTimeoutError::Disconnected) => Err(Error::from_reason(
                    "The GPUI UI thread stopped during the debug frame overlay stats query",
                )),
            }
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn set_window_title(&self, title: String) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window(move |view, window, cx| {
            view.window_title = title;
            cx.notify();
            window.refresh();
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::SetWindowTitle(title));

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |view, window, cx| {
            view.window_title = title;
            cx.notify();
            window.refresh();
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason(
            "The production GPUI Vue renderer does not support this operating system",
        ))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn focus_element(&self, element_id: f64) -> Result<()> {
        let id = to_element_id(element_id)?;
        #[cfg(target_os = "macos")]
        return update_window(move |view, window, cx| {
            view.reveal_virtual_list_ancestor(id);
            if let Some(handle) = view.focus_handles.get(&id) {
                handle.focus(window, cx);
            }
            cx.notify();
            window.refresh();
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::FocusElement(id));

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |view, window, cx| {
            view.reveal_virtual_list_ancestor(id);
            if let Some(handle) = view.focus_handles.get(&id) {
                handle.focus(window, cx);
            }
            cx.notify();
            window.refresh();
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn blur(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, _cx| window.blur());

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::Blur);

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, _cx| {
            window.blur()
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    // ── Selection API ────────────────────────────────────────────────

    /// The current text selection joined in document order, or null.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_selected_text(&self) -> Option<String> {
        self.selection.lock().selected_text()
    }

    /// Drop the current selection and request a repaint.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn clear_selection(&self) -> Result<()> {
        self.selection.lock().clear();
        self.request_invalidate()
    }

    // ── Scroll API ───────────────────────────────────────────────────
    // GpuiView syncs scroll handles and virtual list states to thread-local maps.

    /// Set the scroll offset of a scrollable element.
    /// x and y are negative pixel values (scroll down = more negative y).
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn scroll_to(&self, element_id: f64, x: f64, y: f64) -> Result<()> {
        let id = to_element_id(element_id)?;
        #[cfg(any(target_os = "macos", target_family = "wasm"))]
        if !VIRTUAL_LIST_STATES.with(|cell| {
            let states = cell.borrow();
            let Some(state) = states.get(&id) else {
                return false;
            };
            state.set_offset_from_scrollbar(gpui::point(gpui::px(x as f32), gpui::px(y as f32)));
            true
        }) {
            SCROLL_HANDLES.with(|cell| {
                let handles = cell.borrow();
                if let Some(handle) = handles.get(&id) {
                    handle.set_offset(gpui::point(gpui::px(x as f32), gpui::px(y as f32)));
                }
            });
        }
        #[cfg(target_os = "macos")]
        return invalidate_window();

        #[cfg(target_family = "wasm")]
        return invalidate_web_window(self.web_renderer_id);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::ScrollTo {
            id,
            x: x as f32,
            y: y as f32,
        });

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    /// Scroll a child into view by its index in the children list.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn scroll_to_item(&self, element_id: f64, index: f64) -> Result<()> {
        let id = to_element_id(element_id)?;
        let index = index as usize;
        #[cfg(any(target_os = "macos", target_family = "wasm"))]
        if !VIRTUAL_LIST_STATES.with(|cell| {
            let states = cell.borrow();
            let Some(state) = states.get(&id) else {
                return false;
            };
            state.scroll_to(gpui::ListOffset {
                item_ix: index,
                offset_in_item: gpui::px(0.0),
            });
            true
        }) {
            SCROLL_HANDLES.with(|cell| {
                let handles = cell.borrow();
                if let Some(handle) = handles.get(&id) {
                    handle.scroll_to_item(index);
                }
            });
        }
        #[cfg(target_os = "macos")]
        return invalidate_window();

        #[cfg(target_family = "wasm")]
        return invalidate_web_window(self.web_renderer_id);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::ScrollToItem { id, index });

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    /// Get the current scroll offset of a scrollable element.
    /// Returns [x, y] or null if the element has no scroll handle.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_scroll_offset(&self, element_id: f64) -> Result<Option<Vec<f64>>> {
        let id = to_element_id(element_id)?;
        #[cfg(any(target_os = "macos", target_family = "wasm"))]
        return Ok(VIRTUAL_LIST_STATES
            .with(|cell| {
                cell.borrow().get(&id).map(|state| {
                    let offset = state.scroll_px_offset_for_scrollbar();
                    vec![
                        f64::from(f32::from(offset.x)),
                        f64::from(f32::from(offset.y)),
                    ]
                })
            })
            .or_else(|| {
                SCROLL_HANDLES.with(|cell| {
                    let handles = cell.borrow();
                    handles.get(&id).map(|handle| {
                        let offset = handle.offset();
                        vec![
                            f64::from(f32::from(offset.x)),
                            f64::from(f32::from(offset.y)),
                        ]
                    })
                })
            }));

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetScrollOffset { id, response })?;
            return Ok(
                recv_ui_response(receiver, "the GPUI scroll query")?.map(|[x, y]| vec![x, y])
            );
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_automation_tree(&self) -> Result<String> {
        self.request_invalidate()?;
        let bounds = self.automation_bounds()?;
        let tree = self.tree.lock().unwrap();
        let json = tree.to_automation_json(&bounds);
        serde_json::to_string(&json)
            .map_err(|e| Error::from_reason(format!("JSON serialization failed: {}", e)))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_element_bounds(&self, id: f64) -> Result<Option<Vec<f64>>> {
        let id = to_element_id(id)?;
        Ok(self
            .element_bounds(id)?
            .map(|bounds| vec![bounds.x, bounds.y, bounds.width, bounds.height]))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_all_text(&self) -> Vec<String> {
        let tree = self.tree.lock().unwrap();
        let mut texts = Vec::new();
        if let Some(root_id) = tree.root_id {
            collect_text(root_id, &tree, &mut texts);
        }
        texts
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_painted_text(&self) -> Vec<String> {
        crate::text::painted_text()
    }

    /// Every highlight wash painted in the last frame, in paint order.
    ///
    /// A quad is invisible to `getPaintedText()`, so this is the only way to
    /// assert on `highlight` without a screenshot.
    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn get_painted_highlights(&self) -> Vec<crate::element_tree::HighlightMatch> {
        crate::text::painted_highlights()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Simulate space-separated keystrokes through the focused element's input pipeline.
    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_keystrokes(&self, keystrokes: String) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window_without_view(move |window, cx| {
            crate::automation::dispatch_keystrokes(window, cx, &keystrokes)
        })?
        .map_err(Error::from_reason);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_key_input(KeyInput::Keystrokes(keystrokes));

        #[cfg(target_family = "wasm")]
        return update_web_window_without_view(self.web_renderer_id, move |window, cx| {
            crate::automation::dispatch_keystrokes(window, cx, &keystrokes)
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"))?
        .map_err(Error::from_reason);

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = keystrokes;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_key_down(&self, keystroke: String, is_held: Option<bool>) -> Result<()> {
        let is_held = is_held.unwrap_or(false);

        #[cfg(target_os = "macos")]
        return update_window_without_view(move |window, cx| {
            crate::automation::dispatch_key_down(window, cx, &keystroke, is_held)
        })?
        .map_err(Error::from_reason);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_key_input(KeyInput::Down { keystroke, is_held });

        #[cfg(target_family = "wasm")]
        return update_web_window_without_view(self.web_renderer_id, move |window, cx| {
            crate::automation::dispatch_key_down(window, cx, &keystroke, is_held)
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"))?
        .map_err(Error::from_reason);

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = (keystroke, is_held);
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_key_up(&self, keystroke: String) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window_without_view(move |window, cx| {
            crate::automation::dispatch_key_up(window, cx, &keystroke)
        })?
        .map_err(Error::from_reason);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_key_input(KeyInput::Up(keystroke));

        #[cfg(target_family = "wasm")]
        return update_web_window_without_view(self.web_renderer_id, move |window, cx| {
            crate::automation::dispatch_key_up(window, cx, &keystroke)
        })?
        .ok_or_else(|| Error::from_reason("GPUI web window is not initialized"))?
        .map_err(Error::from_reason);

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = keystroke;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    /// `modifiers` uses the `press()` syntax: "cmd", "cmd-shift", "alt".
    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_click(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let button = button.unwrap_or(0);
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());

        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, cx| {
            crate::automation::dispatch_click(window, cx, x, y, button, modifiers);
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_mouse_input(MouseInput::Click {
            x,
            y,
            button,
            modifiers,
        });

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| {
            crate::automation::dispatch_click(window, cx, x, y, button, modifiers);
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = (x, y, button);
            Err(Error::from_reason(
                "The production GPUIX renderer does not support this operating system",
            ))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_mouse_down(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let button = button.unwrap_or(0);
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());

        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, cx| {
            crate::automation::dispatch_mouse_down(window, cx, x, y, button, modifiers);
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_mouse_input(MouseInput::Down {
            x,
            y,
            button,
            modifiers,
        });

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| {
            crate::automation::dispatch_mouse_down(window, cx, x, y, button, modifiers);
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = (x, y, button);
            Err(Error::from_reason(
                "The production GPUIX renderer does not support this operating system",
            ))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_mouse_up(
        &self,
        x: f64,
        y: f64,
        button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let button = button.unwrap_or(0);
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());

        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, cx| {
            crate::automation::dispatch_mouse_up(window, cx, x, y, button, modifiers);
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_mouse_input(MouseInput::Up {
            x,
            y,
            button,
            modifiers,
        });

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| {
            crate::automation::dispatch_mouse_up(window, cx, x, y, button, modifiers);
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = (x, y, button);
            Err(Error::from_reason(
                "The production GPUIX renderer does not support this operating system",
            ))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_mouse_move(
        &self,
        x: f64,
        y: f64,
        pressed_button: Option<u32>,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());

        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, cx| {
            crate::automation::dispatch_mouse_move(window, cx, x, y, pressed_button, modifiers);
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_mouse_input(MouseInput::Move {
            x,
            y,
            pressed_button,
            modifiers,
        });

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| {
            crate::automation::dispatch_mouse_move(window, cx, x, y, pressed_button, modifiers);
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = (x, y, pressed_button);
            Err(Error::from_reason(
                "The production GPUIX renderer does not support this operating system",
            ))
        }
    }

    /// Dispatch a wheel event through the same GPUI hit test the trackpad uses.
    /// Deltas are pixels: negative `delta_y` scrolls down, negative `delta_x`
    /// pans right, matching `TestGpuiRenderer::simulate_scroll_wheel`.
    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_scroll_wheel(
        &self,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        modifiers: Option<String>,
    ) -> Result<()> {
        let modifiers = crate::automation::parse_modifiers(modifiers.as_deref());

        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, cx| {
            crate::automation::dispatch_scroll_wheel(window, cx, x, y, delta_x, delta_y, modifiers);
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.dispatch_mouse_input(MouseInput::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            modifiers,
        });

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| {
            crate::automation::dispatch_scroll_wheel(window, cx, x, y, delta_x, delta_y, modifiers);
        })
        .map(|_| ());

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = (x, y, delta_x, delta_y, modifiers);
            Err(Error::from_reason(
                "The production GPUIX renderer does not support this operating system",
            ))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn clock_pause(&self) -> Result<f64> {
        #[cfg(target_os = "macos")]
        return update_window(move |view, _window, cx| {
            let now_ms = view.clock.pause();
            cx.notify();
            now_ms
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.control_clock(ClockControl::Pause);

        #[cfg(target_family = "wasm")]
        {
            let now_ms = self.clock.pause();
            self.request_invalidate()?;
            return Ok(now_ms);
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn clock_set(&self, now_ms: f64) -> Result<f64> {
        #[cfg(target_os = "macos")]
        return update_window(move |view, _window, cx| {
            let now_ms = view.clock.set_ms(now_ms);
            cx.notify();
            now_ms
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.control_clock(ClockControl::Set(now_ms));

        #[cfg(target_family = "wasm")]
        {
            let now_ms = self.clock.set_ms(now_ms);
            self.request_invalidate()?;
            return Ok(now_ms);
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = now_ms;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn clock_fast_forward(&self, delta_ms: f64) -> Result<f64> {
        #[cfg(target_os = "macos")]
        return update_window(move |view, _window, cx| {
            let now_ms = view.clock.fast_forward_ms(delta_ms);
            cx.notify();
            now_ms
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.control_clock(ClockControl::FastForward(delta_ms));

        #[cfg(target_family = "wasm")]
        {
            let now_ms = self.clock.fast_forward_ms(delta_ms);
            self.request_invalidate()?;
            return Ok(now_ms);
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        {
            let _ = delta_ms;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn clock_resume(&self) -> Result<f64> {
        #[cfg(target_os = "macos")]
        return update_window(move |view, _window, cx| {
            let now_ms = view.clock.resume();
            cx.notify();
            now_ms
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.control_clock(ClockControl::Resume);

        #[cfg(target_family = "wasm")]
        {
            let now_ms = self.clock.resume();
            self.request_invalidate()?;
            return Ok(now_ms);
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn timeline_get_state(&self) -> TimelineState {
        TimelineState::from(self.clock.snapshot())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn timeline_play(&self) -> Result<TimelineState> {
        self.clock.resume();
        self.invalidate_timeline()?;
        Ok(self.timeline_get_state())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn timeline_pause(&self) -> Result<TimelineState> {
        self.clock.pause();
        self.invalidate_timeline()?;
        Ok(self.timeline_get_state())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn timeline_seek(&self, current_time_ms: f64) -> Result<TimelineState> {
        validate_timeline_time(current_time_ms)?;
        self.clock.seek_ms(current_time_ms);
        self.invalidate_timeline()?;
        Ok(self.timeline_get_state())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn timeline_set_playback_rate(&self, playback_rate: f64) -> Result<TimelineState> {
        if !playback_rate.is_finite() || playback_rate <= 0.0 || playback_rate > 100.0 {
            return Err(Error::from_reason(
                "timeline playbackRate must be finite and greater than 0 (maximum 100)",
            ));
        }
        self.clock.set_playback_rate(playback_rate);
        self.invalidate_timeline()?;
        Ok(self.timeline_get_state())
    }

    fn invalidate_timeline(&self) -> Result<()> {
        if *self.initialized.lock().unwrap() {
            self.request_invalidate()?;
        }
        Ok(())
    }

    /// Configure the bounded decoded-audio queue. Frames are interleaved f32 PCM.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn configure_audio(
        &self,
        sample_rate: u32,
        channels: u32,
        capacity_frames: Option<u32>,
    ) -> Result<AudioBufferState> {
        if !(8_000..=384_000).contains(&sample_rate) {
            return Err(Error::from_reason("audio sampleRate must be 8000..384000"));
        }
        if !(1..=32).contains(&channels) {
            return Err(Error::from_reason("audio channels must be 1..32"));
        }
        let capacity = capacity_frames.unwrap_or(sample_rate.saturating_mul(2));
        if capacity == 0 || capacity > sample_rate.saturating_mul(60) {
            return Err(Error::from_reason(
                "audio capacityFrames must hold between one frame and 60 seconds",
            ));
        }
        let mut audio = self.audio.lock().unwrap();
        audio.configure(sample_rate, channels, capacity as usize);
        Ok(AudioBufferState::from(audio.snapshot()))
    }

    /// Enqueue one decoded chunk without JSON serialization or per-sample FFI calls.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    #[cfg(not(target_family = "wasm"))]
    pub fn enqueue_audio_frames(&self, samples: Float32Array) -> Result<AudioBufferState> {
        self.enqueue_audio_slice(samples.as_ref())
    }

    fn enqueue_audio_slice(&self, samples: &[f32]) -> Result<AudioBufferState> {
        if samples.len() > 20_000_000 {
            return Err(Error::from_reason(
                "an audio chunk may contain at most 20M samples",
            ));
        }
        let mut audio = self.audio.lock().unwrap();
        audio.push(samples).map_err(Error::from_reason)?;
        Ok(AudioBufferState::from(audio.snapshot()))
    }

    /// Consume up to `maxFrames` from a native audio backend or custom host.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    #[cfg(not(target_family = "wasm"))]
    pub fn dequeue_audio_frames(&self, max_frames: u32) -> Float32Array {
        self.dequeue_audio_vec(max_frames).into()
    }

    fn dequeue_audio_vec(&self, max_frames: u32) -> Vec<f32> {
        self.audio.lock().unwrap().pop(max_frames as usize)
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn clear_audio_frames(&self) -> AudioBufferState {
        let mut audio = self.audio.lock().unwrap();
        audio.clear();
        AudioBufferState::from(audio.snapshot())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_audio_buffer_state(&self) -> AudioBufferState {
        AudioBufferState::from(self.audio.lock().unwrap().snapshot())
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn capture_screenshot(&self, path: String) -> Result<()> {
        #[cfg(all(target_os = "macos", feature = "test-support"))]
        {
            let image = update_window(move |_view, window, cx| {
                cx.notify();
                window.refresh();
                window.render_to_image()
            })?
            .map_err(|e| Error::from_reason(format!("Screenshot capture failed: {}", e)))?;
            image
                .save(&path)
                .map_err(|e| Error::from_reason(format!("Failed to save screenshot: {}", e)))?;
            Ok(())
        }

        #[cfg(all(target_os = "windows", feature = "test-support"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::CaptureScreenshot { path, response })?;
            return recv_ui_response(receiver, "screenshot capture")?.map_err(Error::from_reason);
        }

        #[cfg(not(all(
            feature = "test-support",
            any(target_os = "macos", target_os = "windows")
        )))]
        {
            let _ = path;
            Err(Error::from_reason(
                "captureScreenshot needs a test-support build on macOS or Windows",
            ))
        }
    }
}

fn collect_text(id: u64, tree: &RetainedTree, texts: &mut Vec<String>) {
    if let Some(element) = tree.elements.get(&id) {
        if let Some(ref content) = element.content {
            texts.push(content.clone());
        }
        for &child_id in &element.children {
            collect_text(child_id, tree, texts);
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
impl Drop for GpuiRenderer {
    fn drop(&mut self) {
        self.ui_commands.lock().unwrap().take();
    }
}

// ── GPUI View ────────────────────────────────────────────────────────

pub(crate) struct GpuiView {
    pub(crate) tree: Arc<Mutex<RetainedTree>>,
    pub(crate) event_callback: Option<EventCallback>,
    pub(crate) window_title: String,
    /// Persistent FocusHandles keyed by element ID.
    /// Created lazily for elements with keyboard or focus/blur listeners.
    /// Handles persist across renders so GPUI maintains focus state.
    pub(crate) focus_handles: HashMap<u64, gpui::FocusHandle>,
    /// Active focus/blur subscriptions keyed by element and event type.
    pub(crate) focus_subscriptions: HashMap<(u64, String), gpui::Subscription>,
    /// Registry for custom element types (input, editor, diff, etc.).
    /// Stores factories (one per type) and live instances (one per element ID).
    pub(crate) custom_registry: CustomElementRegistry,
    /// Persistent ScrollHandles keyed by element ID.
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
        }
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

        let tree_arc = self.tree.clone();
        let tree = tree_arc.lock().unwrap();
        let window_start = self
            .virtual_lists
            .get(&list_id)
            .map(|entry| entry.window_start)
            .unwrap_or(0);
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
                        &Theme::dark(),
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
                "__gpui_vue_virtual_row_{}_{}",
                list_id, expected_child_id
            )))
            .w_full()
            .track_focus(&focus_handle)
            .child(child)
            .into_any_element()
    }

    pub(crate) fn scroll_virtual_list_to_item(&self, id: u64, index: usize) -> bool {
        let Some(entry) = self.virtual_lists.get(&id) else {
            return false;
        };
        entry.state.scroll_to(gpui::ListOffset {
            item_ix: index,
            offset_in_item: gpui::px(0.0),
        });
        emit_event_full(&self.event_callback, id, "visibleRange", |payload| {
            payload.start_index = Some(index as f64);
            payload.end_index = Some((index + 1) as f64);
        });
        true
    }

    pub(crate) fn set_virtual_list_offset(&self, id: u64, x: f32, y: f32) -> bool {
        let Some(entry) = self.virtual_lists.get(&id) else {
            return false;
        };
        entry
            .state
            .set_offset_from_scrollbar(gpui::point(gpui::px(x), gpui::px(y)));
        true
    }

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
        let tree = tree_arc.lock().unwrap();
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
        self.scroll_virtual_list_to_item(list_id, index)
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
        match style.user_select.as_deref() {
            Some("none") => self.selectable = false,
            Some("text") | Some("auto") => self.selectable = true,
            _ => {}
        }
        if let Some(color) = style
            .selection_color
            .as_deref()
            .and_then(crate::color::parse_color_rgba)
        {
            self.selection_wash = color.into();
        }
        self
    }
}

fn json_usize(value: &serde_json::Value) -> Option<usize> {
    value
        .as_u64()
        .map(|n| n as usize)
        .or_else(|| {
            value
                .as_f64()
                .filter(|n| *n >= 0.0 && n.is_finite())
                .map(|n| n as usize)
        })
        .or_else(|| value.as_i64().filter(|n| *n >= 0).map(|n| n as usize))
}

fn window_start_from_element(element: &crate::retained_tree::RetainedElement) -> usize {
    element
        .custom_props
        .get("windowStart")
        .and_then(json_usize)
        .unwrap_or(0)
}

#[derive(Clone, Copy, PartialEq)]
struct VirtualListConfig {
    alignment: gpui::ListAlignment,
    follow_tail: bool,
    overdraw: f32,
    estimated_item_height: Option<f32>,
    item_count: Option<usize>,
}

impl VirtualListConfig {
    fn from_element(element: &crate::retained_tree::RetainedElement) -> Self {
        let prop = |key: &str| element.custom_props.get(key);
        let alignment = match prop("alignment").and_then(serde_json::Value::as_str) {
            Some("bottom") => gpui::ListAlignment::Bottom,
            _ => gpui::ListAlignment::Top,
        };
        let follow_tail = prop("followTail")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let overdraw = prop("overdraw")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(512.0)
            .max(0.0) as f32;
        let estimated_item_height = prop("estimatedItemHeight")
            .and_then(serde_json::Value::as_f64)
            .filter(|height| *height > 0.0)
            .map(|height| height as f32);
        let item_count = estimated_item_height.and_then(|_| prop("itemCount").and_then(json_usize));
        Self {
            alignment,
            follow_tail,
            overdraw,
            estimated_item_height,
            item_count,
        }
    }

    fn logical_count(self, child_len: usize) -> usize {
        self.item_count.unwrap_or(child_len)
    }

    fn make_state(
        self,
        item_count: usize,
        focus_handles: &[Option<gpui::FocusHandle>],
    ) -> gpui::ListState {
        let mut state = gpui::ListState::new(item_count, self.alignment, gpui::px(self.overdraw));
        if focus_handles.len() == item_count {
            state.splice_focusable(0..item_count, focus_handles.iter().cloned());
        } else {
            state.splice_focusable(0..item_count, (0..item_count).map(|_| None));
        }
        if let Some(height) = self.estimated_item_height {
            state = state.with_uniform_item_height(gpui::px(height));
        }
        if self.follow_tail {
            state.set_follow_mode(gpui::FollowMode::Tail);
        }
        state
    }
}

struct VirtualListEntry {
    state: gpui::ListState,
    config: VirtualListConfig,
    window_start: usize,
    child_ids: Vec<u64>,
    child_revisions: Vec<u64>,
    row_focus_handles: Vec<Option<gpui::FocusHandle>>,
    seen_rows: HashSet<u64>,
}

impl VirtualListEntry {
    fn new(
        config: VirtualListConfig,
        window_start: usize,
        child_ids: Vec<u64>,
        child_revisions: Vec<u64>,
        row_focus_handles: Vec<Option<gpui::FocusHandle>>,
    ) -> Self {
        let item_count = config.logical_count(child_ids.len());
        let state = config.make_state(item_count, &row_focus_handles);
        if row_focus_handles.len() != item_count {
            for (offset, handle) in row_focus_handles.iter().enumerate() {
                if handle.is_some() {
                    let logical = window_start + offset;
                    if logical < item_count {
                        state.splice_focusable(
                            logical..logical + 1,
                            std::iter::once(handle.clone()),
                        );
                    }
                }
            }
        }
        Self {
            state,
            config,
            window_start,
            child_ids,
            child_revisions,
            row_focus_handles,
            seen_rows: HashSet::new(),
        }
    }

    fn child_at(&self, logical_index: usize) -> Option<u64> {
        logical_index
            .checked_sub(self.window_start)
            .and_then(|offset| self.child_ids.get(offset).copied())
    }

    fn logical_index_of(&self, child_id: u64) -> Option<usize> {
        self.child_ids
            .iter()
            .position(|id| *id == child_id)
            .map(|offset| self.window_start + offset)
    }

    fn sync(
        &mut self,
        config: VirtualListConfig,
        window_start: usize,
        child_ids: Vec<u64>,
        child_revisions: Vec<u64>,
        focusable_rows: &HashSet<u64>,
        cx: &mut gpui::Context<GpuiView>,
    ) {
        let focus_unchanged = self.child_ids == child_ids
            && self.row_focus_handles.len() == child_ids.len()
            && self
                .child_ids
                .iter()
                .zip(&self.row_focus_handles)
                .all(|(id, handle)| handle.is_some() == focusable_rows.contains(id));
        if self.config == config
            && self.window_start == window_start
            && focus_unchanged
            && self.child_revisions == child_revisions
        {
            return;
        }

        let old_rows: HashMap<u64, (u64, Option<gpui::FocusHandle>)> = self
            .child_ids
            .iter()
            .copied()
            .zip(self.child_revisions.iter().copied())
            .zip(self.row_focus_handles.iter().cloned())
            .map(|((id, revision), focus_handle)| (id, (revision, focus_handle)))
            .collect();
        let row_focus_handles: Vec<Option<gpui::FocusHandle>> = child_ids
            .iter()
            .map(|id| {
                focusable_rows.contains(id).then(|| {
                    old_rows
                        .get(id)
                        .and_then(|(_, focus_handle)| focus_handle.clone())
                        .unwrap_or_else(|| cx.focus_handle())
                })
            })
            .collect();
        if self.config != config {
            let scroll_top = self.state.logical_scroll_top();
            let should_follow =
                config.follow_tail && (!self.config.follow_tail || self.state.is_following_tail());
            let mut replacement = Self::new(
                config,
                window_start,
                child_ids,
                child_revisions,
                row_focus_handles,
            );
            replacement.seen_rows = std::mem::take(&mut self.seen_rows);
            replacement
                .seen_rows
                .retain(|id| replacement.child_ids.contains(id));
            if !should_follow {
                replacement.state.scroll_to(scroll_top);
            }
            *self = replacement;
            return;
        }

        // gpui anchors a list on a logical item, so splicing rows in at the
        // front keeps the rows already on screen and pushes the new ones above
        // the viewport. A browser anchors too, but suppresses it at scrollTop 0,
        // so a prepend is visible. Match the browser: remember a list pinned to
        // the top and put it back after the splice.
        //
        // While the content is shorter than the viewport gpui re-anchors to
        // item 0 every layout, so the drift only appears once the list
        // overflows. That is why `example-app` looked stuck at two rows.
        //
        // The guard is `is_following_tail()`, not `config.follow_tail`: a
        // following list that does not fill its viewport also ends layout
        // anchored at {0, 0}, and `scroll_to` would call `stop_following` on it.
        // Once the user scrolls up to the top, following is already stopped, so
        // a top-aligned `followTail` list still gets the browser behaviour.
        let top = self.state.logical_scroll_top();
        let was_pinned_to_top = matches!(config.alignment, gpui::ListAlignment::Top)
            && !self.state.is_following_tail()
            && top.item_ix == 0
            && top.offset_in_item <= gpui::px(0.0);

        // A windowed list's children are a sliding viewport. Splicing by
        // child position would treat a scroll as a rewrite of items 0..N.
        if config.item_count.is_none() && self.child_ids != child_ids {
            let prefix = self
                .child_ids
                .iter()
                .zip(&child_ids)
                .take_while(|(old, new)| old == new)
                .count();
            let suffix = self.child_ids[prefix..]
                .iter()
                .rev()
                .zip(child_ids[prefix..].iter().rev())
                .take_while(|(old, new)| old == new)
                .count();
            self.state.splice_focusable(
                prefix..self.child_ids.len().saturating_sub(suffix),
                row_focus_handles[prefix..row_focus_handles.len().saturating_sub(suffix)]
                    .iter()
                    .cloned(),
            );
            if let Some(height) = config.estimated_item_height {
                self.state = self
                    .state
                    .clone()
                    .with_uniform_item_height(gpui::px(height));
            }
        }

        for (offset, (&id, focus_handle)) in child_ids.iter().zip(&row_focus_handles).enumerate() {
            let logical = window_start + offset;
            let focusability_changed = old_rows
                .get(&id)
                .is_some_and(|(_, old_handle)| old_handle.is_some() != focus_handle.is_some());
            if focusability_changed {
                self.state
                    .splice_focusable(logical..logical + 1, std::iter::once(focus_handle.clone()));
            }
        }

        let mut changed_start = None;
        for (offset, (&id, &revision)) in child_ids.iter().zip(&child_revisions).enumerate() {
            let logical = window_start + offset;
            let changed = old_rows
                .get(&id)
                .is_some_and(|(old_revision, _)| *old_revision != revision);
            match (changed_start, changed) {
                (None, true) => changed_start = Some(logical),
                (Some(start), false) => {
                    self.state.remeasure_items(start..logical);
                    changed_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = changed_start {
            self.state
                .remeasure_items(start..window_start + child_ids.len());
        }
        self.remeasure_unknown_rows(window_start, &child_ids, &old_rows);
        if was_pinned_to_top {
            self.state.scroll_to(gpui::ListOffset::default());
        }

        self.window_start = window_start;
        self.child_ids = child_ids;
        self.child_revisions = child_revisions;
        self.row_focus_handles = row_focus_handles;
    }

    fn remeasure_unknown_rows(
        &mut self,
        window_start: usize,
        child_ids: &[u64],
        known: &HashMap<u64, (u64, Option<gpui::FocusHandle>)>,
    ) {
        let mut range_start = None;
        for (offset, id) in child_ids.iter().enumerate() {
            let logical = window_start + offset;
            let is_new = !known.contains_key(id);
            match (range_start, is_new) {
                (None, true) => range_start = Some(logical),
                (Some(start), false) => {
                    self.state.remeasure_items(start..logical);
                    range_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = range_start {
            self.state
                .remeasure_items(start..window_start + child_ids.len());
        }
    }
}

impl GpuiView {
    /// Sync focus handles with the current element tree.
    /// Creates handles for new focusable elements, subscribes on_focus/on_blur,
    /// and cleans up handles for destroyed elements.
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
                .and_then(|value| value.as_i64())
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
        self.focus_handles
            .retain(|id, _| tree.elements.get(id).is_some_and(&needs_focus));
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

        // Clone Arc so we don't borrow self.tree — frees self for focus_handles access.
        let tree_arc = self.tree.clone();
        let tree = tree_arc.lock().unwrap();
        let callback = self.event_callback.clone();

        // Sync focus handles before building elements.
        self.sync_focus_handles(&tree, &callback, window, cx);

        // Ensure custom element instances are destroyed when their IDs disappear.
        self.custom_registry
            .prune_missing(|id| tree.elements.contains_key(&id));

        // Clean up scroll handles for destroyed elements (IDs removed from tree).
        // Scrollability-based cleanup (element still exists but style changed
        // from scroll to non-scroll) is handled inside build_div().
        self.scroll_handles
            .retain(|id, _| tree.elements.contains_key(id));
        self.virtual_lists
            .retain(|id, _| tree.elements.contains_key(id));
        self.motion_states
            .retain(|id, _| tree.elements.contains_key(id));

        // Build the element tree. custom_registry, focus_handles, and scroll_handles
        // are different fields of self, so Rust allows borrowing all simultaneously.
        let theme = Theme::dark();
        let now = self.clock.now();
        let mut motion_active = false;
        // Pruned by DECLARATION, not existence: an element that drops its
        // `highlight` prop keeps living, and its cached group list holds a copy
        // of every string in its subtree.
        self.highlights.retain(|id, _| {
            tree.elements
                .get(id)
                .is_some_and(|element| element.custom_props.contains_key("highlight"))
        });
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
                    inherited: Inherited::root(&theme),
                    highlights: &mut self.highlights,
                    highlight_events: &mut highlight_events,
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

        if motion_active && self.clock.is_playing() {
            window.request_animation_frame();
        }

        result
    }
}

// ── Element builders ─────────────────────────────────────────────────

pub(crate) fn build_element(
    id: u64,
    ctx: &mut BuildCtx,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<GpuiView>,
) -> gpui::AnyElement {
    use gpui::IntoElement;

    let Some(element) = ctx.tree.elements.get(&id) else {
        return gpui::Empty.into_any_element();
    };

    let animated_style = if let Some(source) = element.custom_props.get("motion") {
        let state = match ctx.motion_states.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                match crate::motion::MotionState::new(source, ctx.now) {
                    Ok(state) => entry.insert(state),
                    Err(error) => {
                        log::warn!("Invalid motion description for element {id}: {error}");
                        entry.insert(crate::motion::MotionState::invalid(source, ctx.now))
                    }
                }
            }
        };
        if let Err(error) = state.sync(source, ctx.now) {
            log::warn!("Invalid motion update for element {id}: {error}");
        }
        state.is_valid().then(|| {
            let frame = state.frame(ctx.now);
            *ctx.motion_active |= frame.active;
            // `Arc<StyleDesc>` is shared, so the animated frame is applied to a
            // copy. Mutating through the pointer would restyle every element
            // that declared the same style.
            let mut resolved = element.style.as_deref().cloned().unwrap_or_default();
            frame.style.apply_to(&mut resolved);
            resolved
        })
    } else {
        ctx.motion_states.remove(&id);
        None
    };
    let style = animated_style.as_ref().or(element.style.as_deref());

    // Inheritable style resolves once here so both built-ins and custom
    // elements see the same cascade.
    let parent_inherited = ctx.inherited.clone();
    ctx.inherited = parent_inherited.clone().descend(style);

    // A `highlight` here replaces any ancestor's: the nearest declaration wins,
    // and `GroupList::collect` skips nested declarations so an ancestor never
    // resolves or counts matches that will not paint.
    if let Some(value) = element.custom_props.get("highlight") {
        let has_listener = element.events.contains("highlight");
        let resolved = resolve_highlight(
            ctx.highlights,
            ctx.tree,
            id,
            value,
            &Theme::dark(),
            has_listener,
        );
        if let Some((_, Some(total))) = &resolved {
            ctx.highlight_events.push((id, *total));
        }
        ctx.inherited.highlight = resolved.map(|(context, _)| context);
    }

    let built = match element.element_type.as_str() {
        "div" => {
            ctx.custom_registry.destroy(id);
            build_div(element, style, ctx, window, cx)
        }
        "text" => {
            ctx.custom_registry.destroy(id);
            build_text(element, style, ctx, window, cx)
        }
        "virtual-list" => {
            ctx.custom_registry.destroy(id);
            build_virtual_list(element, ctx, window, cx)
        }

        // Polymorphic dispatch for all custom elements.
        custom_type => {
            let custom_children: Vec<gpui::AnyElement> = element
                .children
                .iter()
                .copied()
                .filter(|child_id| ctx.tree.elements.contains_key(child_id))
                .map(|child_id| build_element(child_id, ctx, window, cx))
                .collect();
            let inherited = ctx.inherited.clone();
            let render_ctx = CustomRenderContext {
                id,
                events: &element.events,
                event_callback: ctx.event_callback,
                focus_handle: ctx.focus_handles.get(&id),
                style,
                children: custom_children,
                selection: ctx.selection.clone(),
                selectable: inherited.selectable,
                selection_wash: inherited.selection_wash,
                highlight_set: inherited.highlight.clone(),
            };
            ctx.custom_registry
                .render(custom_type, &element.custom_props, render_ctx, window, cx)
        }
    };

    ctx.inherited = parent_inherited;
    built
}

fn build_virtual_list(
    element: &crate::retained_tree::RetainedElement,
    ctx: &mut BuildCtx,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<GpuiView>,
) -> gpui::AnyElement {
    use gpui::prelude::*;

    let child_ids: Vec<u64> = element
        .children
        .iter()
        .copied()
        .filter(|child_id| ctx.tree.elements.contains_key(child_id))
        .collect();
    let child_revisions: Vec<u64> = child_ids
        .iter()
        .filter_map(|child_id| {
            ctx.tree
                .elements
                .get(child_id)
                .map(|child| child.subtree_revision)
        })
        .collect();
    let focusable_rows: HashSet<u64> = ctx
        .focus_handles
        .keys()
        .filter_map(|element_id| virtual_row_ancestor(ctx.tree, element.id, *element_id))
        .collect();
    let focused_row = ctx
        .focus_handles
        .iter()
        .find_map(|(element_id, handle)| {
            handle
                .is_focused(window)
                .then(|| virtual_row_ancestor(ctx.tree, element.id, *element_id))
                .flatten()
        })
        .or_else(|| {
            ctx.focus_handles.keys().find_map(|element_id| {
                ctx.tree
                    .elements
                    .get(element_id)
                    .is_some_and(|element| element.auto_focus)
                    .then(|| virtual_row_ancestor(ctx.tree, element.id, *element_id))
                    .flatten()
            })
        });
    let config = VirtualListConfig::from_element(element);
    let window_start = if config.item_count.is_some() {
        window_start_from_element(element)
    } else {
        0
    };
    let list_state = match ctx.virtual_lists.entry(element.id) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            entry.get_mut().sync(
                config,
                window_start,
                child_ids.clone(),
                child_revisions,
                &focusable_rows,
                cx,
            );
            let entry = entry.into_mut();
            if let Some(row_id) = focused_row.filter(|row_id| !entry.seen_rows.contains(row_id)) {
                if let Some(index) = entry.logical_index_of(row_id) {
                    entry.state.scroll_to(gpui::ListOffset {
                        item_ix: index,
                        offset_in_item: gpui::px(0.0),
                    });
                }
            }
            entry.state.clone()
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            let row_focus_handles = child_ids
                .iter()
                .map(|id| focusable_rows.contains(id).then(|| cx.focus_handle()))
                .collect();
            let entry = entry.insert(VirtualListEntry::new(
                config,
                window_start,
                child_ids.clone(),
                child_revisions,
                row_focus_handles,
            ));
            if let Some(row_id) = focused_row {
                if let Some(index) = entry.logical_index_of(row_id) {
                    entry.state.scroll_to(gpui::ListOffset {
                        item_ix: index,
                        offset_in_item: gpui::px(0.0),
                    });
                }
            }
            entry.state.clone()
        }
    };

    if element.events.contains("visibleRange") {
        let callback = ctx.event_callback.clone();
        let list_id = element.id;
        list_state.set_scroll_handler(move |event, _window, _cx| {
            emit_event_full(&callback, list_id, "visibleRange", |payload| {
                payload.start_index = Some(event.visible_range.start as f64);
                payload.end_index = Some(event.visible_range.end as f64);
            });
        });
    }

    let list_id = element.id;
    // Cloned, not copied: gpui runs this processor once per requested row, so
    // the captured value must survive every call.
    let inherited = ctx.inherited.clone();
    let render_item = cx.processor(move |view, index: usize, window, cx| {
        let Some(entry) = view.virtual_lists.get(&list_id) else {
            return unmounted_virtual_row(1.0);
        };
        let Some(child_id) = entry.child_at(index) else {
            // Empty measures as 0 and poisons ListState. Keep the estimate.
            return unmounted_virtual_row(entry.config.estimated_item_height.unwrap_or(1.0));
        };
        view.build_virtual_child(list_id, index, child_id, inherited.clone(), window, cx)
    });
    let mut list =
        gpui::list(list_state, render_item).with_sizing_behavior(gpui::ListSizingBehavior::Auto);
    if let Some(style) = element.style.as_deref() {
        list = apply_styles(list, style);
    }
    list.into_any_element()
}

fn unmounted_virtual_row(height: f32) -> gpui::AnyElement {
    use gpui::prelude::*;
    gpui::div().h(gpui::px(height.max(1.0))).w_full().into_any()
}

fn virtual_row_ancestor(tree: &RetainedTree, list_id: u64, element_id: u64) -> Option<u64> {
    let mut current = element_id;
    loop {
        let parent = tree.elements.get(&current)?.parent?;
        if parent == list_id {
            return Some(current);
        }
        current = parent;
    }
}

pub(crate) fn build_div(
    element: &crate::retained_tree::RetainedElement,
    style: Option<&StyleDesc>,
    ctx: &mut BuildCtx,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<GpuiView>,
) -> gpui::AnyElement {
    use gpui::prelude::*;

    let element_id_str = format!("__gpui_vue_{}", element.id);
    let mut el = gpui::div().id(gpui::SharedString::from(element_id_str));

    if let Some(style) = style {
        el = apply_styles(el, style);

        // ── Pseudo-selector styles (hover / active) ──────────────────
        // GPUI's .hover() and .active() take a closure that receives a
        // StyleRefinement and returns it with modifications. Since
        // StyleRefinement implements Styled, we can reuse apply_styles().
        if let Some(ref hover_style) = style.hover {
            el = el.hover(|refinement| apply_styles(refinement, hover_style));
        }
        if let Some(ref active_style) = style.active {
            el = el.active(|refinement| apply_styles(refinement, active_style));
        }

        if crate::style::should_occlude(style) {
            // BlockMouse (occlude) stops the hit test, so the parent scroller
            // never sees the wheel. HTML does not work that way: a wheel over
            // an absolutely positioned card still scrolls the ancestor. Only
            // `pointerEvents: "auto"` opts into stealing it. Everything else
            // uses BlockMouseExceptScroll.
            //
            // Absolute used to steal it too. That made a pannable canvas
            // impossible: every absolutely placed item (a timeline clip, a
            // graph node) ended the hit test before the pan listener ran.
            // `<anchored>` still occludes through its own `occlude` prop, so
            // menus and tooltips are unaffected.
            el = if style.pointer_events.as_deref() == Some("auto") {
                el.occlude()
            } else {
                el.block_mouse_except_scroll()
            };
        }
    }

    // ── Overflow: scroll ─────────────────────────────────────────────
    // overflow_scroll() requires StatefulInteractiveElement (only on Stateful<Div>),
    // so we handle it here rather than in apply_styles (which takes E: Styled).
    //
    // CSS precedence: axis-specific props (overflowX/Y) override the shorthand
    // (overflow). E.g. { overflow: "scroll", overflowY: "hidden" } → scroll X only.
    //
    // overflow-x only works as a flex viewport. Default display is Block, so a
    // wide child fills the parent instead of overflowing. Zed's code-block path:
    // flex + min_w_0 on the scroller, flex_none on the child.
    let mut overflow_x_only = false;
    if let Some(style) = style {
        // Resolve each axis: axis-specific overrides shorthand.
        let resolved_x = style.overflow_x.as_deref().or(style.overflow.as_deref());
        let resolved_y = style.overflow_y.as_deref().or(style.overflow.as_deref());

        let needs_scroll_x = resolved_x == Some("scroll");
        let needs_scroll_y = resolved_y == Some("scroll");

        if needs_scroll_x && needs_scroll_y {
            el = el.overflow_scroll();
            // GPUI zeroes the smaller of the two deltas by default, so one
            // diagonal wheel moves one axis. A browser moves both, and a
            // two-axis container is exactly where a user expects that.
            el.style().allow_concurrent_scroll = Some(true);
        } else if needs_scroll_x {
            overflow_x_only = true;
            el = el
                .flex()
                .min_w_0()
                .overflow_x_scroll()
                .restrict_scroll_to_axis();
        } else if needs_scroll_y {
            el = el.overflow_y_scroll();
        }

        // Attach a persistent ScrollHandle when scrolling is enabled.
        // The handle persists across renders (stored in GpuiView::scroll_handles)
        // so GPUI maintains the scroll offset between frames.
        if needs_scroll_x || needs_scroll_y {
            let handle = ctx
                .scroll_handles
                .entry(element.id)
                .or_insert_with(gpui::ScrollHandle::new);
            el = el.track_scroll(handle);
        } else {
            // Element is no longer scrollable — remove stale handle.
            ctx.scroll_handles.remove(&element.id);
        }
    } else {
        // No style at all — remove stale handle if it existed.
        ctx.scroll_handles.remove(&element.id);
    }

    // If a FocusHandle was pre-created for this element (by sync_focus_handles),
    // attach it via track_focus. This makes the element focusable — clicking it
    // or tabbing to it gives it keyboard focus. The handle persists across renders
    // because it's stored in GpuiView::focus_handles.
    if style.and_then(|style| style.position.as_deref()).is_none() {
        el = el.relative();
    }
    el = el.child(crate::automation::bounds_tracker(
        element.id,
        selection_start_flag(style),
    ));

    if let Some(handle) = ctx.focus_handles.get(&element.id) {
        el = el.track_focus(handle);
    }
    if let Some(tab_index) = element
        .custom_props
        .get("tabIndex")
        .and_then(|value| value.as_i64())
        .and_then(|index| isize::try_from(index).ok())
    {
        el = el.tab_index(tab_index).tab_stop(tab_index >= 0);
    }

    // Wire up events.
    // Some events (on_hover, on_click) require a stateful element (.id()),
    // which we already set above. Others (on_mouse_down, on_key_down) work
    // on any InteractiveElement.
    for event_type in &element.events {
        let id = element.id;
        let callback = ctx.event_callback.clone();
        match event_type.as_str() {
            // ── Click ────────────────────────────────────────────
            // Primary button only, like the DOM. Right and middle clicks go to
            // `onAuxClick`, and `onMouseDown` sees every button.
            "click" => {
                el = el.on_click(move |click_event, _window, _cx| {
                    emit_event_full(&callback, id, "click", |p| {
                        let (x, y) = point_to_xy(click_event.position());
                        p.x = Some(x);
                        p.y = Some(y);
                        p.modifiers = Some(click_event.modifiers().into());
                        p.click_count = Some(click_event.click_count() as u32);
                        p.is_right_click = Some(click_event.is_right_click());
                    });
                });
            }

            // ── Aux click (non-primary), like the DOM `auxclick` ──
            "auxClick" => {
                el = el.on_aux_click(move |click_event, _window, _cx| {
                    emit_event_full(&callback, id, "auxClick", |p| {
                        let (x, y) = point_to_xy(click_event.position());
                        p.x = Some(x);
                        p.y = Some(y);
                        p.modifiers = Some(click_event.modifiers().into());
                        p.click_count = Some(click_event.click_count() as u32);
                        p.is_right_click = Some(click_event.is_right_click());
                    });
                });
            }

            // ── Mouse down (all buttons) ─────────────────────────
            "mouseDown" => {
                // Wire all three buttons so JS gets right-click, middle-click, etc.
                for &button in &[
                    gpui::MouseButton::Left,
                    gpui::MouseButton::Middle,
                    gpui::MouseButton::Right,
                ] {
                    let callback = callback.clone();
                    el = el.on_mouse_down(button, move |mouse_event, _window, _cx| {
                        emit_event_full(&callback, id, "mouseDown", |p| {
                            let (x, y) = point_to_xy(mouse_event.position);
                            p.x = Some(x);
                            p.y = Some(y);
                            p.button = Some(mouse_button_to_u32(mouse_event.button));
                            p.click_count = Some(mouse_event.click_count as u32);
                            p.modifiers = Some(mouse_event.modifiers.into());
                        });
                    });
                }
            }

            // ── Mouse up (all buttons) ───────────────────────────
            "mouseUp" => {
                for &button in &[
                    gpui::MouseButton::Left,
                    gpui::MouseButton::Middle,
                    gpui::MouseButton::Right,
                ] {
                    let callback = callback.clone();
                    el = el.on_mouse_up(button, move |mouse_event, _window, _cx| {
                        emit_event_full(&callback, id, "mouseUp", |p| {
                            let (x, y) = point_to_xy(mouse_event.position);
                            p.x = Some(x);
                            p.y = Some(y);
                            p.button = Some(mouse_button_to_u32(mouse_event.button));
                            p.click_count = Some(mouse_event.click_count as u32);
                            p.modifiers = Some(mouse_event.modifiers.into());
                        });
                    });
                }
            }

            // ── Mouse move ───────────────────────────────────────
            "mouseMove" => {
                el = el.on_mouse_move(move |mouse_event, _window, _cx| {
                    emit_event_full(&callback, id, "mouseMove", |p| {
                        let (x, y) = point_to_xy(mouse_event.position);
                        p.x = Some(x);
                        p.y = Some(y);
                        p.modifiers = Some(mouse_event.modifiers.into());
                        p.pressed_button = mouse_event.pressed_button.map(mouse_button_to_u32);
                    });
                });
            }

            // ── Hover (mouseEnter + mouseLeave) ──────────────────
            // GPUI's on_hover fires with true on enter, false on leave.
            // We split into two distinct event types for the Vue side.
            "mouseEnter" | "mouseLeave" => {
                // Only wire once even if both mouseEnter and mouseLeave are registered.
                // Check if we already wired on_hover via the other event.
                let has_enter = element.events.contains("mouseEnter");
                let has_leave = element.events.contains("mouseLeave");
                // Wire on first encounter (mouseEnter sorts before mouseLeave).
                if event_type.as_str() == "mouseEnter" || !has_enter {
                    let callback_enter = if has_enter {
                        ctx.event_callback.clone()
                    } else {
                        None
                    };
                    let callback_leave = if has_leave {
                        ctx.event_callback.clone()
                    } else {
                        None
                    };
                    el = el.on_hover(move |&is_hovered, _window, _cx| {
                        if is_hovered {
                            emit_event_full(&callback_enter, id, "mouseEnter", |p| {
                                p.hovered = Some(true);
                            });
                        } else {
                            emit_event_full(&callback_leave, id, "mouseLeave", |p| {
                                p.hovered = Some(false);
                            });
                        }
                    });
                }
            }

            // ── Mouse down outside ───────────────────────────────
            // Fires when the user clicks OUTSIDE this element.
            // Critical for "click outside to close" pattern (dropdowns, modals).
            "mouseDownOutside" => {
                el = el.on_mouse_down_out(move |mouse_event, _window, _cx| {
                    emit_event_full(&callback, id, "mouseDownOutside", |p| {
                        let (x, y) = point_to_xy(mouse_event.position);
                        p.x = Some(x);
                        p.y = Some(y);
                        p.button = Some(mouse_button_to_u32(mouse_event.button));
                        p.modifiers = Some(mouse_event.modifiers.into());
                    });
                });
            }

            // ── Scroll wheel ─────────────────────────────────────
            "scroll" => {
                el = el.on_scroll_wheel(move |scroll_event, _window, _cx| {
                    emit_event_full(&callback, id, "scroll", |p| {
                        let (x, y) = point_to_xy(scroll_event.position);
                        p.x = Some(x);
                        p.y = Some(y);
                        p.modifiers = Some(scroll_event.modifiers.into());
                        p.precise = Some(scroll_event.delta.precise());

                        // Convert ScrollDelta to pixel values.
                        // For Lines delta, we use a default line height of 20px.
                        let line_height = gpui::px(20.0);
                        let pixel_delta = scroll_event.delta.pixel_delta(line_height);
                        p.delta_x = Some(f64::from(f32::from(pixel_delta.x)));
                        p.delta_y = Some(f64::from(f32::from(pixel_delta.y)));

                        p.touch_phase = Some(match scroll_event.touch_phase {
                            gpui::TouchPhase::Started => "started".to_string(),
                            gpui::TouchPhase::Moved => "moved".to_string(),
                            gpui::TouchPhase::Ended => "ended".to_string(),
                            gpui::TouchPhase::Cancelled => "cancelled".to_string(),
                        });
                    });
                });
            }

            // ── Key down ─────────────────────────────────────────
            // Requires .focusable() (set above). Element must be focused
            // (clicked or tabbed to) for these to fire.
            "keyDown" => {
                el = el.on_key_down(move |key_event, _window, _cx| {
                    emit_event_full(&callback, id, "keyDown", |p| {
                        p.key = Some(key_event.keystroke.key.clone());
                        p.key_char = key_event.keystroke.key_char.clone();
                        p.is_held = Some(key_event.is_held);
                        p.modifiers = Some(key_event.keystroke.modifiers.into());
                    });
                });
            }

            // ── Key up ───────────────────────────────────────────
            "keyUp" => {
                el = el.on_key_up(move |key_event, _window, _cx| {
                    emit_event_full(&callback, id, "keyUp", |p| {
                        p.key = Some(key_event.keystroke.key.clone());
                        p.key_char = key_event.keystroke.key_char.clone();
                        p.modifiers = Some(key_event.keystroke.modifiers.into());
                    });
                });
            }

            // ── Focus / Blur ─────────────────────────────────────
            // Event emission is handled by FocusHandle subscriptions
            // set up in GpuiView::sync_focus_handles(). The handle is
            // attached to this element via .track_focus() above.
            "focus" | "blur" => {}

            _ => {}
        }
    }

    if element.events.contains("mouseDown") && element.events.contains("mouseMove") {
        el = el.capture_pointer();
    }

    // Text content — selectable, same as a <text> leaf.
    if let Some(ref content) = element.content {
        el = el.child(text_content(element, content, ctx));
    }

    // Children
    let child_ids: Vec<u64> = element.children.clone();
    for child_id in child_ids {
        let child = build_element(child_id, ctx, window, cx);
        el = if overflow_x_only {
            el.child(gpui::div().flex_none().child(child))
        } else {
            el.child(child)
        };
    }

    el.into_any_element()
}

/// A selectable text run owned by `element`. Runs are left to gpui so the
/// text keeps inheriting colour, weight and family from ancestor styles.
///
/// The run's group is its parent host element, because Vue makes a separate
/// host node for every interpolated string. `<text>Hello {name}!</text>` is one
/// logical line painted as three runs that all share the parent's id.
/// A `userSelect: "none"` run still paints highlight washes, because a browser
/// still finds that text with Ctrl+F. Element chrome that must never be found,
/// such as a code gutter, uses `chrome_text` instead.
fn text_content(
    element: &crate::retained_tree::RetainedElement,
    content: &str,
    ctx: &BuildCtx,
) -> gpui::AnyElement {
    selectable_text(crate::text::SelectableText {
        group: crate::text::search::group_id(ctx.tree, element.id),
        selectable: ctx.inherited.selectable,
        highlight: ctx
            .inherited
            .highlight
            .clone()
            .map(crate::text::HighlightSource::Resolved),
        ..crate::text::SelectableText::new(
            element.id,
            0,
            gpui::SharedString::from(content.to_string()),
            None,
            ctx.selection.clone(),
            ctx.inherited.selection_wash,
        )
    })
}

pub(crate) fn build_text(
    element: &crate::retained_tree::RetainedElement,
    style: Option<&StyleDesc>,
    ctx: &mut BuildCtx,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<GpuiView>,
) -> gpui::AnyElement {
    use gpui::prelude::*;

    // Fast path: plain text leaf without style. It still goes through
    // `text_content` so the glyphs land in the selection registry — the old
    // raw-string return was the reason text was not selectable.
    if style.is_none() && element.children.is_empty() {
        let content = element.content.clone().unwrap_or_default();
        return gpui::div()
            .relative()
            .child(crate::automation::bounds_tracker(element.id, None))
            .child(text_content(element, &content, ctx))
            .into_any_element();
    }

    // The full style set, exactly as `<div>` gets it. `<text>` used to apply a
    // text-only subset, so `padding`, `width` and every layout prop on a text
    // node were silently dropped — a hole with no error and no warning.
    let mut el = gpui::div();
    if let Some(style) = style {
        el = apply_styles(el, style);
    }
    if style.and_then(|style| style.position.as_deref()).is_none() {
        el = el.relative();
    }
    el = el.child(crate::automation::bounds_tracker(
        element.id,
        selection_start_flag(style),
    ));

    if let Some(ref content) = element.content {
        el = el.child(text_content(element, content, ctx));
    }

    let child_ids: Vec<u64> = element.children.clone();
    for child_id in child_ids {
        el = el.child(build_element(child_id, ctx, window, cx));
    }

    el.into_any_element()
}

/// Explicit `userSelect` on this node. `None` means inherit; the ancestor
/// that set the value already owns the start region.
fn selection_start_flag(style: Option<&StyleDesc>) -> Option<bool> {
    match style.and_then(|style| style.user_select.as_deref()) {
        Some("none") => Some(false),
        Some("text") | Some("auto") => Some(true),
        _ => None,
    }
}

// ── Style application ────────────────────────────────────────────────

pub(crate) fn apply_width<E: gpui::Styled>(el: E, dim: &crate::style::DimensionValue) -> E {
    match dim {
        crate::style::DimensionValue::Pixels(v) => el.w(gpui::px(*v as f32)),
        crate::style::DimensionValue::Percentage(v) if *v >= 0.999 => el.w_full(),
        crate::style::DimensionValue::Percentage(v) => el.w(gpui::relative(*v as f32)),
        crate::style::DimensionValue::Auto => el,
    }
}

pub(crate) fn apply_height<E: gpui::Styled>(el: E, dim: &crate::style::DimensionValue) -> E {
    match dim {
        crate::style::DimensionValue::Pixels(v) => el.h(gpui::px(*v as f32)),
        crate::style::DimensionValue::Percentage(v) if *v >= 0.999 => el.h_full(),
        crate::style::DimensionValue::Percentage(v) => el.h(gpui::relative(*v as f32)),
        crate::style::DimensionValue::Auto => el,
    }
}

pub(crate) fn apply_styles<E: gpui::Styled>(mut el: E, style: &StyleDesc) -> E {
    match style.display.as_deref() {
        Some("flex") => el = el.flex(),
        Some("grid") => el = el.grid(),
        _ => {}
    }
    if let Some(cols) = style.grid_template_columns {
        let count = cols.round().clamp(1.0, 64.0) as u16;
        el = match style.grid_column_min.as_deref() {
            Some("min-content") => el.grid_cols_min_content(count),
            Some("max-content") => el.grid_cols_max_content(count),
            _ => el.grid_cols(count),
        };
    }
    if let Some(rows) = style.grid_template_rows {
        let count = rows.round().clamp(1.0, 64.0) as u16;
        el = match style.grid_row_min.as_deref() {
            Some("min-content") => el.grid_rows_min_content(count),
            Some("max-content") => el.grid_rows_max_content(count),
            _ => el.grid_rows(count),
        };
    }
    if style.flex_direction.as_deref() == Some("column") {
        el = el.flex_col();
    }
    if style.flex_direction.as_deref() == Some("row") {
        el = el.flex_row();
    }
    match style.flex_wrap.as_deref() {
        Some("wrap") => el = el.flex_wrap(),
        Some("wrap-reverse") => el = el.flex_wrap_reverse(),
        Some("nowrap") => el = el.flex_nowrap(),
        _ => {}
    }
    if let Some(grow) = style.flex_grow {
        el.style().flex_grow = Some(grow as f32);
    }
    if let Some(shrink) = style.flex_shrink {
        el.style().flex_shrink = Some(shrink as f32);
    }
    if let Some(basis) = style.flex_basis {
        el = el.flex_basis(gpui::px(basis as f32));
    }
    match style.align_items.as_deref() {
        Some("center") => el = el.items_center(),
        Some("start") | Some("flex-start") => el = el.items_start(),
        Some("end") | Some("flex-end") => el = el.items_end(),
        _ => {}
    }
    match style.align_content.as_deref() {
        Some("center") => el = el.content_center(),
        Some("start") | Some("flex-start") => el = el.content_start(),
        Some("end") | Some("flex-end") => el = el.content_end(),
        Some("between") | Some("space-between") => el = el.content_between(),
        Some("around") | Some("space-around") => el = el.content_around(),
        Some("evenly") | Some("space-evenly") => el = el.content_evenly(),
        Some("stretch") => el = el.content_stretch(),
        Some("normal") => el = el.content_normal(),
        _ => {}
    }
    match style.justify_content.as_deref() {
        Some("center") => el = el.justify_center(),
        Some("start") | Some("flex-start") => el = el.justify_start(),
        Some("end") | Some("flex-end") => el = el.justify_end(),
        Some("between") | Some("space-between") => el = el.justify_between(),
        Some("around") | Some("space-around") => el = el.justify_around(),
        _ => {}
    }
    match style.align_self.as_deref() {
        Some("center") => {
            el.style().align_self = Some(gpui::AlignItems::Center);
        }
        Some("start") | Some("flex-start") => {
            el.style().align_self = Some(gpui::AlignItems::FlexStart);
        }
        Some("end") | Some("flex-end") => {
            el.style().align_self = Some(gpui::AlignItems::FlexEnd);
        }
        Some("stretch") => {
            el.style().align_self = Some(gpui::AlignItems::Stretch);
        }
        Some("baseline") => {
            el.style().align_self = Some(gpui::AlignItems::Baseline);
        }
        _ => {}
    }
    if let Some(gap) = style.gap {
        el = el.gap(gpui::px(gap as f32));
    }
    // Per-axis gaps were in the style type and implemented nowhere. They come
    // after `gap` so the axis value wins, matching CSS shorthand order.
    if let Some(gap) = style.row_gap {
        el = el.gap_y(gpui::px(gap as f32));
    }
    if let Some(gap) = style.column_gap {
        el = el.gap_x(gpui::px(gap as f32));
    }
    if let Some(ref w) = style.width {
        el = apply_width(el, w);
    }
    if let Some(ref h) = style.height {
        el = apply_height(el, h);
    }
    if let Some(ref min_w) = style.min_width {
        match min_w {
            crate::style::DimensionValue::Pixels(v) => el = el.min_w(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.min_w(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(ref min_h) = style.min_height {
        match min_h {
            crate::style::DimensionValue::Pixels(v) => el = el.min_h(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.min_h(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(ref max_w) = style.max_width {
        match max_w {
            crate::style::DimensionValue::Pixels(v) => el = el.max_w(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.max_w(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(ref max_h) = style.max_height {
        match max_h {
            crate::style::DimensionValue::Pixels(v) => el = el.max_h(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.max_h(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(p) = style.padding {
        el = el.p(gpui::px(p as f32));
    }
    if let Some(pt) = style.padding_top {
        el = el.pt(gpui::px(pt as f32));
    }
    if let Some(pr) = style.padding_right {
        el = el.pr(gpui::px(pr as f32));
    }
    if let Some(pb) = style.padding_bottom {
        el = el.pb(gpui::px(pb as f32));
    }
    if let Some(pl) = style.padding_left {
        el = el.pl(gpui::px(pl as f32));
    }
    if let Some(m) = style.margin {
        el = el.m(gpui::px(m as f32));
    }
    if let Some(mt) = style.margin_top {
        el = el.mt(gpui::px(mt as f32));
    }
    if let Some(mr) = style.margin_right {
        el = el.mr(gpui::px(mr as f32));
    }
    if let Some(mb) = style.margin_bottom {
        el = el.mb(gpui::px(mb as f32));
    }
    if let Some(ml) = style.margin_left {
        el = el.ml(gpui::px(ml as f32));
    }
    // Taffy has no viewport-fixed position, and GPUI has no scrolling document,
    // so "fixed" lays out exactly like "absolute". `should_occlude` already
    // treated the two the same; without this arm a "fixed" box stayed in flow.
    match style.position.as_deref() {
        Some("absolute") | Some("fixed") => el = el.absolute(),
        Some("relative") => el = el.relative(),
        _ => {}
    }
    if let Some(top) = style.top {
        el = el.top(gpui::px(top as f32));
    }
    if let Some(right) = style.right {
        el = el.right(gpui::px(right as f32));
    }
    if let Some(bottom) = style.bottom {
        el = el.bottom(gpui::px(bottom as f32));
    }
    if let Some(left) = style.left {
        el = el.left(gpui::px(left as f32));
    }
    if let Some(ref bg) = style
        .background_color
        .as_ref()
        .or(style.background.as_ref())
    {
        if let Some(color) = crate::color::parse_color_rgba(bg) {
            el = el.bg(color);
        }
    }
    if let Some(ref color) = style.color {
        if let Some(color) = crate::color::parse_color_rgba(color) {
            el = el.text_color(color);
        }
    }
    if let Some(size) = style.font_size {
        el = el.text_size(gpui::px(size as f32));
    }
    if let Some(ref family) = style.font_family {
        el = el.font_family(family.clone());
    }
    if let Some(ref weight) = style.font_weight {
        el = el.font_weight(parse_font_weight(weight));
    }
    // `textAlign` was in the style type but implemented nowhere.
    match style.text_align.as_deref() {
        Some("center") => el = el.text_center(),
        Some("right") => el = el.text_right(),
        Some("left") | Some("start") => el = el.text_left(),
        _ => {}
    }
    match style.white_space.as_deref() {
        Some("nowrap") => el = el.whitespace_nowrap(),
        Some("normal") => el = el.whitespace_normal(),
        _ => {}
    }
    match style.text_overflow.as_deref() {
        Some("ellipsis") => el = el.text_ellipsis(),
        Some("ellipsis-start") => el = el.text_ellipsis_start(),
        _ => {}
    }
    if let Some(clamp) = style.line_clamp {
        if clamp >= 1.0 {
            el = el.line_clamp(clamp as usize);
        }
    }
    // `line_height` was accepted by the style type but never applied, so
    // multi-line text always used gpui's default leading.
    if let Some(line_height) = style.line_height {
        if line_height > 0.0 {
            el = el.line_height(gpui::px(line_height as f32));
        }
    }
    if let Some(radius) = style.border_radius {
        el = el.rounded(gpui::px(radius as f32));
    }
    // Apply corner longhands after the shorthand so the explicit corner wins.
    if let Some(radius) = style.border_top_left_radius {
        el = el.rounded_tl(gpui::px(radius as f32));
    }
    if let Some(radius) = style.border_top_right_radius {
        el = el.rounded_tr(gpui::px(radius as f32));
    }
    if let Some(radius) = style.border_bottom_left_radius {
        el = el.rounded_bl(gpui::px(radius as f32));
    }
    if let Some(radius) = style.border_bottom_right_radius {
        el = el.rounded_br(gpui::px(radius as f32));
    }
    // `borderWidth: 0` must clear a border, not be ignored: an element that
    // draws its own border needs a way for the caller to remove it.
    if let Some(width) = style.border_width {
        el = el.border(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_top_width {
        el = el.border_t(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_right_width {
        el = el.border_r(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_bottom_width {
        el = el.border_b(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_left_width {
        el = el.border_l(gpui::px(width.max(0.0) as f32));
    }
    if let Some(ref color) = style.border_color {
        if let Some(color) = crate::color::parse_color_rgba(color) {
            el = el.border_color(color);
        }
    }
    if let Some(ref shadow) = style.box_shadow {
        if let Some(color) = crate::color::parse_color_rgba(&shadow.color) {
            let shadow = gpui::BoxShadow::new(
                gpui::px(shadow.offset_x as f32),
                gpui::px(shadow.offset_y as f32),
                color.into(),
            )
            .blur_radius(gpui::px(shadow.blur_radius.max(0.0) as f32))
            .spread_radius(gpui::px(shadow.spread_radius as f32));
            el = el.shadow(vec![shadow]);
        }
    }
    if let Some(opacity) = style.opacity {
        el = el.opacity(opacity as f32);
    }
    if let Some(cursor) = style.cursor.as_deref().and_then(crate::style::parse_cursor) {
        el = el.cursor(cursor);
    }
    // Overflow: hidden is on the Styled trait, so we handle it here.
    // overflow: "scroll" requires StatefulInteractiveElement — handled in build_div().
    // CSS precedence: axis-specific (overflowX/Y) overrides the shorthand (overflow).
    {
        let resolved_x = style.overflow_x.as_deref().or(style.overflow.as_deref());
        let resolved_y = style.overflow_y.as_deref().or(style.overflow.as_deref());
        // Only apply hidden here — scroll is handled in build_div.
        if resolved_x == Some("hidden") && resolved_y == Some("hidden") {
            el = el.overflow_hidden();
        } else if resolved_x == Some("hidden") {
            el = el.overflow_x_hidden();
        } else if resolved_y == Some("hidden") {
            el = el.overflow_y_hidden();
        }
    }

    el
}

// ── Event emission ───────────────────────────────────────────────────

/// Helper to convert a GPUI Point<Pixels> to (f64, f64).
pub(crate) fn point_to_xy(p: gpui::Point<gpui::Pixels>) -> (f64, f64) {
    (f64::from(f32::from(p.x)), f64::from(f32::from(p.y)))
}

/// Convert GPUI MouseButton to our u32 encoding: 0=left, 1=middle, 2=right.
pub(crate) fn mouse_button_to_u32(button: gpui::MouseButton) -> u32 {
    match button {
        gpui::MouseButton::Left => 0,
        gpui::MouseButton::Middle => 1,
        gpui::MouseButton::Right => 2,
        gpui::MouseButton::Navigate(_) => 3,
    }
}

/// General-purpose event emitter. Builds a default EventPayload, lets the
/// caller customize it via a closure, then sends it through the callback.
/// Production: queues on Node.js event loop via ThreadsafeFunction.
/// Tests: pushes to a synchronous Vec for drainEvents().
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

// ── Batch processing ─────────────────────────────────────────────

/// Parsed batch operation — typed enum for atomic validation.
/// All ops are parsed and validated BEFORE any tree mutation occurs.
/// This prevents partial application on malformed batches.
enum BatchOp<'a> {
    CreateElement {
        id: u64,
        element_type: String,
    },
    DestroyElement {
        id: u64,
    },
    AppendChild {
        parent_id: u64,
        child_id: u64,
    },
    RemoveChild {
        parent_id: u64,
        child_id: u64,
    },
    InsertBefore {
        parent_id: u64,
        child_id: u64,
        before_id: u64,
    },
    /// The payload stays as raw JSON until apply time.
    ///
    /// Two reasons. A parsed `StyleDesc` is ~1.4 KB, and a `Vec<BatchOp>` is as
    /// wide as its widest variant, so inlining one made a 220k-op mount reserve
    /// over 300 MB before it parsed a single op. And the tree hash-conses
    /// styles by content, so it needs the bytes: hashing ~110 bytes is far
    /// cheaper than building 80 `Option` fields and throwing 99.8% of them away.
    SetStyle {
        id: u64,
        style: &'a serde_json::value::RawValue,
    },
    SetText {
        id: u64,
        content: String,
    },
    SetEventListener {
        id: u64,
        event_type: String,
        has_handler: bool,
    },
    SetRoot {
        id: u64,
    },
    SetCustomProp {
        id: u64,
        key: String,
        value: serde_json::Value,
    },
}

/// A batch failure. The message names the op index, so it survives the trip
/// back to JS as a plain `Error`.
pub type BatchResult<T> = std::result::Result<T, String>;

/// Decode the batch straight from its JSON bytes into `Vec<BatchOp>`.
///
/// There is deliberately no `Vec<serde_json::Value>` in between. That tree cost
/// a `String` per key and per value, every payload was then deep-cloned out of
/// it, and `from_value` parsed the clone a second time, so one style was
/// allocated three times. A 220k-op mount made 1.5M allocations that way.
///
/// Everything the `Value` version guaranteed still holds, and each one is
/// load-bearing:
///
/// * an unknown opcode is a hard error, not a skipped op. Silently ignoring one
///   would let a JS/Rust version skew desync the tree instead of throwing
/// * ids go through `raw_element_id`, so non-finite, negative, fractional and
///   out-of-safe-range values are still rejected
/// * `hasHandler` is accepted as a bool or a number
/// * errors still name the op index. `serde_json` reports a byte offset, which
///   is useless when you are chasing a desync
fn parse_batch_ops(bytes: &[u8]) -> BatchResult<Vec<BatchOp<'_>>> {
    serde_json::from_slice::<BatchOps>(bytes)
        .map(|batch| batch.0)
        .map_err(|error| format!("Failed to parse batch: {error}"))
}

struct BatchOps<'a>(Vec<BatchOp<'a>>);

impl<'de> serde::Deserialize<'de> for BatchOps<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct OpsVisitor;

        impl<'de> serde::de::Visitor<'de> for OpsVisitor {
            type Value = BatchOps<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an array of mutation tuples")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<BatchOps<'de>, A::Error> {
                let mut ops = Vec::with_capacity(seq.size_hint().unwrap_or(64));
                loop {
                    // The index is attached here because this is the only place
                    // that knows it.
                    let index = ops.len();
                    match seq.next_element::<BatchOp<'de>>() {
                        Ok(Some(op)) => ops.push(op),
                        Ok(None) => break,
                        Err(error) => {
                            return Err(serde::de::Error::custom(format!(
                                "Batch op {index}: {error}"
                            )))
                        }
                    }
                }
                Ok(BatchOps(ops))
            }
        }

        deserializer.deserialize_seq(OpsVisitor)
    }
}

/// A string argument, borrowed from the input when the JSON has no escapes.
///
/// The owned copy happens exactly once, on the way into the `BatchOp`. The
/// `Value` path allocated twice: into `Value::String`, then into the op.
struct StrArg<'a>(std::borrow::Cow<'a, str>);

impl<'de> serde::Deserialize<'de> for StrArg<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use std::borrow::Cow;
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = StrArg<'de>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string")
            }
            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> std::result::Result<StrArg<'de>, E> {
                Ok(StrArg(Cow::Borrowed(v)))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<StrArg<'de>, E> {
                Ok(StrArg(Cow::Owned(v.to_owned())))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<StrArg<'de>, E> {
                Ok(StrArg(Cow::Owned(v)))
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// A legacy `setCustomProp` payload: a JSON string gets decoded, anything else
/// is taken as-is. `setCustomPropValue` skips this and stores the raw value.
struct LegacyPropArg(serde_json::Value);

impl<'de> serde::Deserialize<'de> for LegacyPropArg {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let serde_json::Value::String(encoded) = &value {
            return Ok(LegacyPropArg(
                serde_json::from_str(encoded).unwrap_or_else(|_| value.clone()),
            ));
        }
        Ok(LegacyPropArg(value))
    }
}

/// `hasHandler` arrives as a bool from the reconciler and as a non-negative
/// integer from hand-written batches. That is exactly what `as_bool()` then
/// `as_u64()` accepted before, so a negative or fractional number stays an
/// error rather than quietly meaning `true`.
struct BoolArg(bool);

impl<'de> serde::Deserialize<'de> for BoolArg {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BoolArg;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a boolean or a non-negative integer")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<BoolArg, E> {
                Ok(BoolArg(v))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<BoolArg, E> {
                Ok(BoolArg(v != 0))
            }
        }
        deserializer.deserialize_any(V)
    }
}

fn next_arg<'de, A, T>(seq: &mut A, what: &str) -> std::result::Result<T, A::Error>
where
    A: serde::de::SeqAccess<'de>,
    T: serde::Deserialize<'de>,
{
    seq.next_element()?
        .ok_or_else(|| serde::de::Error::custom(format!("missing {what}")))
}

/// Read an element id. Ids cross napi as JS numbers, so they are read as `f64`
/// and validated exactly as `batch_id` did.
fn next_id<'de, A: serde::de::SeqAccess<'de>>(
    seq: &mut A,
    what: &str,
) -> std::result::Result<u64, A::Error> {
    let raw: f64 = next_arg(seq, what)?;
    raw_element_id(raw).map_err(serde::de::Error::custom)
}

impl<'de> serde::Deserialize<'de> for BatchOp<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct V;

        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BatchOp<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a [opcode, ...args] mutation tuple")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<BatchOp<'de>, A::Error> {
                let name: StrArg<'de> = next_arg(&mut seq, "op name")?;
                let op = match name.0.as_ref() {
                    "createElement" => BatchOp::CreateElement {
                        id: next_id(&mut seq, "id")?,
                        element_type: next_arg::<A, StrArg>(&mut seq, "element type")?
                            .0
                            .into_owned(),
                    },
                    "destroyElement" => BatchOp::DestroyElement {
                        id: next_id(&mut seq, "id")?,
                    },
                    "appendChild" => BatchOp::AppendChild {
                        parent_id: next_id(&mut seq, "parent id")?,
                        child_id: next_id(&mut seq, "child id")?,
                    },
                    "removeChild" => BatchOp::RemoveChild {
                        parent_id: next_id(&mut seq, "parent id")?,
                        child_id: next_id(&mut seq, "child id")?,
                    },
                    "insertBefore" => BatchOp::InsertBefore {
                        parent_id: next_id(&mut seq, "parent id")?,
                        child_id: next_id(&mut seq, "child id")?,
                        before_id: next_id(&mut seq, "before id")?,
                    },
                    "setStyle" => BatchOp::SetStyle {
                        id: next_id(&mut seq, "id")?,
                        style: next_arg(&mut seq, "style")?,
                    },
                    "setText" => BatchOp::SetText {
                        id: next_id(&mut seq, "id")?,
                        content: next_arg::<A, StrArg>(&mut seq, "text")?.0.into_owned(),
                    },
                    "setEventListener" => BatchOp::SetEventListener {
                        id: next_id(&mut seq, "id")?,
                        event_type: next_arg::<A, StrArg>(&mut seq, "event type")?
                            .0
                            .into_owned(),
                        has_handler: next_arg::<A, BoolArg>(&mut seq, "hasHandler")?.0,
                    },
                    "setRoot" => BatchOp::SetRoot {
                        id: next_id(&mut seq, "id")?,
                    },
                    "setCustomProp" => BatchOp::SetCustomProp {
                        id: next_id(&mut seq, "id")?,
                        key: next_arg::<A, StrArg>(&mut seq, "prop key")?.0.into_owned(),
                        value: next_arg::<A, LegacyPropArg>(&mut seq, "custom prop value")?.0,
                    },
                    "setCustomPropValue" => BatchOp::SetCustomProp {
                        id: next_id(&mut seq, "id")?,
                        key: next_arg::<A, StrArg>(&mut seq, "prop key")?.0.into_owned(),
                        value: next_arg(&mut seq, "custom prop value")?,
                    },
                    other => {
                        return Err(serde::de::Error::custom(format!(
                            "unknown operation: {other:?}"
                        )))
                    }
                };
                // Trailing arguments are tolerated, as they were when the op was
                // an indexed array.
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(op)
            }
        }

        deserializer.deserialize_seq(V)
    }
}

/// Turn one raw `setStyle` payload into a shared style.
///
/// The reconciler always sends an object. A legacy batch can send the same
/// object as a JSON *string*, so that is unwrapped to the bytes the interner
/// should see. Anything else, `null` included, is handed to `StyleDesc` and
/// rejected there. Doing this here, rather than in the deserializer, keeps the
/// raw bytes available for the content hash.
fn intern_style_payload(
    styles: &mut StyleTable,
    payload: &serde_json::value::RawValue,
) -> BatchResult<Arc<StyleDesc>> {
    // A `RawValue` always holds exactly one complete JSON value, so this is
    // never empty and never a fragment.
    let raw = payload.get().trim();
    if raw.starts_with('"') {
        let encoded: String = serde_json::from_str(raw).map_err(|error| error.to_string())?;
        styles.intern(encoded.as_bytes())
    } else {
        styles.intern(raw.as_bytes())
    }
}

/// Resolve every `setStyle` payload in the batch, in op order.
///
/// This is the last fallible step, so it runs before the apply loop and borrows
/// only the style table. The borrow checker then proves no element was touched
/// when it returns `Err`, which is what makes a batch atomic. An earlier
/// version interned inside the apply loop, so a malformed style at the end of a
/// batch left everything before it applied and then threw.
fn resolve_styles(
    styles: &mut StyleTable,
    ops: &[BatchOp<'_>],
) -> BatchResult<Vec<Arc<StyleDesc>>> {
    let mut resolved = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        if let BatchOp::SetStyle { style, .. } = op {
            let shared = intern_style_payload(styles, style)
                .map_err(|error| format!("Batch op {index} setStyle parse error: {error}"))?;
            resolved.push(shared);
        }
    }
    Ok(resolved)
}

/// Apply a batch of mutation tuples to a RetainedTree.
/// Shared between GpuiRenderer::apply_batch and TestGpuiRenderer::apply_batch.
/// Returns accumulated destroyed IDs (as f64) from all destroyElement ops.
///
/// ATOMIC: the batch is decoded and every style is resolved before a single
/// element is touched. If any op is malformed the tree is left unchanged and an
/// error is returned. Nothing after that point can fail, so JS and Rust cannot
/// desync when a batch is retried.
///
/// Batch format: JSON array of tuples [opcode, ...args].
/// See GpuiRenderer::apply_batch for opcode documentation.
///
/// Public so `examples/bench_serde.rs` times this exact function. A replica in
/// the bench would drift, and the numbers would then describe code nobody runs.
pub fn apply_batch_to_tree(tree: &mut RetainedTree, bytes: &[u8]) -> BatchResult<Vec<f64>> {
    // Phase 1: decode. No mutation.
    let parsed = parse_batch_ops(bytes)?;

    // Phase 2: resolve styles. Touches the style table only; a failure here
    // sweeps back out whatever this call interned.
    let styles = resolve_styles(&mut tree.styles, &parsed).inspect_err(|_| tree.styles.sweep())?;
    let mut styles = styles.into_iter();

    // Phase 3: apply. Cannot fail.
    let mut destroyed_ids: Vec<f64> = Vec::new();
    for batch_op in parsed {
        match batch_op {
            BatchOp::CreateElement { id, element_type } => {
                tree.create_element(id, element_type);
            }
            BatchOp::DestroyElement { id } => {
                let destroyed = tree.destroy_element(id);
                destroyed_ids.extend(destroyed.iter().map(|&id| id as f64));
            }
            BatchOp::AppendChild {
                parent_id,
                child_id,
            } => {
                tree.append_child(parent_id, child_id);
            }
            BatchOp::RemoveChild {
                parent_id,
                child_id,
            } => {
                tree.remove_child(parent_id, child_id);
            }
            BatchOp::InsertBefore {
                parent_id,
                child_id,
                before_id,
            } => {
                tree.insert_before(parent_id, child_id, before_id);
            }
            BatchOp::SetStyle { id, .. } => {
                let shared = styles.next().expect("one resolved style per setStyle op");
                tree.set_style(id, shared);
            }
            BatchOp::SetText { id, content } => {
                tree.set_text(id, content);
            }
            BatchOp::SetEventListener {
                id,
                event_type,
                has_handler,
            } => {
                tree.set_event_listener(id, event_type, has_handler);
            }
            BatchOp::SetRoot { id } => {
                tree.root_id = Some(id);
            }
            BatchOp::SetCustomProp { id, key, value } => {
                tree.set_custom_prop(id, key, value);
            }
        }
    }

    // Release styles nothing references any more. Without this a dragged
    // element, which produces a distinct style every frame, would grow the
    // table for as long as the app runs. The element count is what catches the
    // opposite case, a batch that destroyed most of the tree.
    let live_elements = tree.elements.len();
    tree.styles.maybe_sweep(live_elements);

    Ok(destroyed_ids)
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
    if !value.is_finite() || value < 0.0 || value > 315_576_000_000.0 {
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
        ..Default::default()
    }
}

#[cfg(test)]
mod highlight_cache_tests {
    use super::*;

    fn tree_with_text() -> RetainedTree {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        tree.create_element(2, "text".to_string());
        tree.append_child(1, 2);
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
            let mut events: Vec<_> = element.events.iter().cloned().collect();
            events.sort();
            let mut props: Vec<_> = element.custom_props.iter().collect();
            props.sort_by(|(a, _), (b, _)| a.cmp(b));
            out += &format!(
                "{id} type={} text={:?} style={:?} children={:?} parent={:?} events={events:?} props={props:?} rev={}/{}\n",
                element.element_type,
                element.content,
                element.style.as_deref(),
                element.children,
                element.parent,
                element.subtree_revision,
                element.search_revision,
            );
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
            (r#"[42]"#, "a non-array op"),
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
