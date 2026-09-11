//! Public N-API and WebAssembly renderer methods.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    reason = "intentional N-API/JavaScript/GPUI boundary representation"
)]

use super::backend::Backend as _;
use super::*;

/// The main GPUI renderer exposed to Node.js.
#[cfg_attr(not(target_family = "wasm"), napi)]
pub struct GpuiRenderer {
    pub(super) event_callback: Mutex<Option<EventCallback>>,
    pub(super) tree: Arc<Mutex<RetainedTree>>,
    pub(super) initialized: Arc<Mutex<bool>>,
    pub(super) headless: Mutex<bool>,
    /// Worker completions only mark dirty; `AppKit` is touched by the host tick.
    #[cfg(target_os = "macos")]
    pending_gpu_repaint: std::sync::atomic::AtomicBool,
    pub(super) window_size: Mutex<WindowSize>,
    /// Shared with the view so timeline controls avoid a UI-thread round trip.
    clock: crate::automation::AutomationClock,
    audio: Mutex<crate::audio::AudioFrameQueue>,
    /// Shared with `GpuiView` so napi methods can read the live selection
    /// without an App context. Paint and napi calls can use different threads.
    selection: SharedSelection,
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    pub(super) ui_commands: Mutex<Option<mpsc::UnboundedSender<UiCommand>>>,
    #[cfg(target_family = "wasm")]
    pub(super) web_renderer_id: u64,
}

#[cfg_attr(not(target_family = "wasm"), napi)]
impl GpuiRenderer {
    #[cfg_attr(
        not(target_family = "wasm"),
        napi(
            constructor,
            ts_args_type = "eventCallback?: (error: Error | null, event: EventPayload | null) => void"
        )
    )]
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
            #[cfg(target_os = "macos")]
            pending_gpu_repaint: std::sync::atomic::AtomicBool::new(false),
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

    /// Allocate a renderer-owned source for the canvas `source` prop.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn create_canvas_source(&self) -> Result<u32> {
        self.tree
            .lock()
            .canvas_frames
            .lock()
            .create()
            .map_err(Error::from_reason)
    }

    /// Reset published images and invalidate outstanding GPU publications.
    #[cfg(not(target_family = "wasm"))]
    #[napi]
    pub fn reset_canvas_source(&self, id: u32) -> Result<()> {
        self.tree
            .lock()
            .canvas_frames
            .lock()
            .reset(id)
            .map_err(Error::from_reason)
    }

    /// GPU snapshot presentation; rejects unsupported platforms and foreign devices.
    #[cfg(not(target_family = "wasm"))]
    #[napi]
    pub async fn present_canvas_texture(
        &self,
        id: u32,
        device: &crate::gpu_binding::NativeWgpuDevice,
        texture: u32,
        opaque: bool,
    ) -> Result<bool> {
        if cfg!(target_os = "windows") {
            return Err(Error::from_reason(
                "DirectX shared texture presentation is not implemented; select async-readback explicitly",
            ));
        }
        let frames = self.tree.lock().canvas_frames.clone();
        let source = {
            let frames = frames.lock();
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            if frames
                .shared_gpu
                .as_ref()
                .is_none_or(|engine| !Arc::ptr_eq(&engine.device, &device.engine.device))
            {
                return Err(Error::from_reason(
                    "Linux direct presentation requires this window's shared GPU device",
                ));
            }
            frames
                .direct
                .get(&id)
                .cloned()
                .ok_or_else(|| Error::from_reason("Unknown or destroyed canvas source"))?
        };
        let texture = device.engine.texture(texture).map_err(Error::from_reason)?;
        let presented = source
            .present(&device.engine, texture, opaque)
            .await
            .map_err(Error::from_reason)?;
        if presented && *self.initialized.lock() {
            #[cfg(target_os = "macos")]
            self.pending_gpu_repaint
                .store(true, std::sync::atomic::Ordering::Release);
            #[cfg(not(target_os = "macos"))]
            self.request_invalidate()?;
        }
        Ok(presented)
    }

    #[cfg(not(target_family = "wasm"))]
    #[napi]
    pub fn canvas_presentation(&self) -> String {
        if cfg!(target_os = "macos") {
            "metal"
        } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            "shared-wgpu"
        } else {
            "unsupported"
        }
        .into()
    }

    /// Acquire the Linux compositor device after the window has painted once.
    #[cfg(not(target_family = "wasm"))]
    #[napi]
    pub fn canvas_gpu_device(&self) -> Option<crate::gpu_binding::NativeWgpuDevice> {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            self.tree
                .lock()
                .canvas_frames
                .lock()
                .shared_gpu
                .clone()
                .map(crate::gpu_binding::NativeWgpuDevice::new)
        }
        #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
        {
            None
        }
    }

    /// Publish padded RGBA8 or BGRA8 pixels through the binary bridge.
    #[cfg(not(target_family = "wasm"))]
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
        self.present_canvas_slice(id, width, height, stride, pixels.as_ref(), bgra, opaque)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "flat binary frame signature shared by N-API and Wasm avoids JSON metadata in the presentation path"
    )]
    pub(super) fn present_canvas_slice(
        &self,
        id: u32,
        width: u32,
        height: u32,
        stride: u32,
        pixels: &[u8],
        bgra: bool,
        opaque: bool,
    ) -> Result<()> {
        let image = crate::gpu_canvas::decode_frame(width, height, stride, pixels, bgra, opaque)
            .map_err(Error::from_reason)?;
        let frames = self.tree.lock().canvas_frames.clone();
        frames
            .lock()
            .publish_image(id, image)
            .map_err(Error::from_reason)?;
        if *self.initialized.lock() {
            self.request_invalidate()?;
        }
        Ok(())
    }

    /// Release a source. Repeated destruction is harmless.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn destroy_canvas_source(&self, id: u32) -> Result<()> {
        self.tree.lock().canvas_frames.lock().destroy(id);
        if *self.initialized.lock() {
            self.request_invalidate()?;
        }
        Ok(())
    }

    /// Initialize GPUI using the native event-loop architecture for this OS.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn init(&self, options: Option<WindowOptions>) -> Result<()> {
        if *self.initialized.lock() {
            return Err(Error::from_reason("Renderer is already initialized"));
        }

        let headless = options
            .as_ref()
            .and_then(|options| options.headless)
            .unwrap_or(false);
        if let Some(options) = &options {
            let mut size = self.window_size.lock();
            size.width = options.width.unwrap_or(size.width);
            size.height = options.height.unwrap_or(size.height);
        }
        if headless {
            *self.headless.lock() = true;
            *self.initialized.lock() = true;
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
        let activate = options.focus.unwrap_or(true);
        let window_options = options.clone();
        let tree = self.tree.clone();
        let callback = self.event_callback_for_view();
        let callback_for_close = callback.clone();
        let selection = self.selection.clone();
        let clock = self.clock.clone();
        let initialized = self.initialized.clone();
        let window_slot = Rc::new(RefCell::new(None));
        let window_slot_for_launch = window_slot.clone();

        let application = gpui_platform::single_threaded_web();
        let application_handle = application.run_embedded(move |cx: &mut gpui::App| {
            crate::custom_elements::input::init(cx);
            crate::custom_elements::img::init(cx);
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
                            *initialized_for_close.lock() = false;
                            emit_event_full(&callback_for_close, 0, "windowClose", |_| {});
                        }
                    })
                    .detach();
                    *window_slot_for_launch.borrow_mut() = Some(window);
                    if activate {
                        cx.activate(true);
                    }
                }
                Err(error) => {
                    *initialized.lock() = false;
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
        *self.initialized.lock() = true;
        self.event_callback.lock().take();
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[allow(clippy::too_many_lines)]
    fn init_macos(&self, options: Option<WindowOptions>) -> Result<()> {
        let options = options.unwrap_or_default();

        {
            let initialized = self.initialized.lock();
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
        let activate = options.focus.unwrap_or(true);
        let window_options = options.clone();

        let platform = Rc::new(gpui_macos::MacPlatform::new_embedded());

        let tree = self.tree.clone();
        let callback = self.event_callback_for_view();
        let callback_for_close = callback.clone();

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
            crate::custom_elements::input::init(cx);
            crate::custom_elements::img::init(cx);
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
                    let window_id = window_handle.window_id();
                    cx.on_window_closed(move |_cx, closed_id| {
                        if closed_id == window_id {
                            emit_event_full(&callback_for_close, 0, "windowClose", |_| {});
                        }
                    })
                    .detach();
                    *opened_window_for_app.borrow_mut() = Some(window_handle);
                    if activate {
                        cx.activate(true);
                    }
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

        *self.initialized.lock() = true;
        self.event_callback.lock().take();
        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn init_threaded(&self, options: Option<WindowOptions>) -> Result<()> {
        let options = options.unwrap_or_default();
        if *self.initialized.lock() {
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

        *self.ui_commands.lock() = Some(command_sender);
        self.event_callback.lock().take();
        Ok(())
    }

    // ── Mutation API ─────────────────────────────────────────────────

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn create_element(&self, id: f64, element_type: String) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock();
        tree.create_element(id, element_type);
        Ok(())
    }

    /// Destroy an element and all descendants. Returns array of destroyed IDs
    /// so JS can clean up event handlers for the entire subtree.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn destroy_element(&self, id: f64) -> Result<Vec<f64>> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock();
        let destroyed = tree.destroy_element(id);
        Ok(destroyed.iter().map(|&id| id as f64).collect())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn append_child(&self, parent_id: f64, child_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let mut tree = self.tree.lock();
        tree.append_child(parent_id, child_id)
            .map_err(Error::from_reason)
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn remove_child(&self, parent_id: f64, child_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let mut tree = self.tree.lock();
        tree.remove_child(parent_id, child_id);
        Ok(())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn insert_before(&self, parent_id: f64, child_id: f64, before_id: f64) -> Result<()> {
        let parent_id = to_element_id(parent_id)?;
        let child_id = to_element_id(child_id)?;
        let before_id = to_element_id(before_id)?;
        let mut tree = self.tree.lock();
        tree.insert_before(parent_id, child_id, before_id)
            .map_err(Error::from_reason)
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_style(&self, id: f64, style_json: String) -> Result<()> {
        let id = to_element_id(id)?;
        self.tree
            .lock()
            .set_style_json(id, style_json.as_bytes())
            .map_err(|error| Error::from_reason(format!("Failed to parse style: {error}")))
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_text(&self, id: f64, content: String) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock();
        tree.set_text(id, content);
        Ok(())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_event_listener(&self, id: f64, event_type: String, has_handler: bool) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock();
        tree.set_event_listener(id, event_type, has_handler);
        Ok(())
    }

    /// Set the root element (called from appendChildToContainer).
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_root(&self, id: f64) -> Result<()> {
        let id = to_element_id(id)?;
        let mut tree = self.tree.lock();
        tree.set_root(id);
        Ok(())
    }

    /// Set a custom prop on an element (for non-div/text elements like input, editor, diff).
    /// Key is the prop name, value is JSON-encoded.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_custom_prop(&self, id: f64, key: String, value_json: String) -> Result<()> {
        let id = to_element_id(id)?;
        let value: serde_json::Value = serde_json::from_str(&value_json)
            .map_err(|e| Error::from_reason(format!("Failed to parse custom prop value: {e}")))?;
        let mut tree = self.tree.lock();
        tree.set_custom_prop(id, key, value);
        Ok(())
    }

    /// Get a custom prop value from an element. Returns JSON string or null.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_custom_prop(&self, id: f64, key: String) -> Result<Option<String>> {
        let id = to_element_id(id)?;
        let tree = self.tree.lock();
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
    /// ```text
    /// ["createElement",    id, "type"]
    /// ["destroyElement",   id]
    /// ["appendChild",      parentId, childId]
    /// ["removeChild",      parentId, childId]
    /// ["insertBefore",     parentId, childId, beforeId]
    /// ["setStyle",         id, { ...style } | "{styleJson}"]
    /// ["setText",          id, "content"]
    /// ["setEventListener", id, "eventType", true|false]
    /// ["setRoot",          id]
    /// ["setCustomProp",      id, "key", value | "{valueJson}"]
    /// ["setCustomPropValue", id, "key", value]
    /// ```
    ///
    /// Returns accumulated destroyed IDs from all destroyElement ops.
    /// Acquires the tree mutex ONCE for the entire batch.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn apply_batch(&self, json: String) -> Result<Vec<f64>> {
        let parsed = parse_batch_ops(json.as_bytes()).map_err(Error::from_reason)?;
        let mut tree = self.tree.lock();
        let destroyed =
            apply_parsed_batch_to_tree(&mut tree, parsed).map_err(Error::from_reason)?;
        drop(tree);
        self.request_invalidate()?;
        Ok(destroyed)
    }

    // ── Frame loop ───────────────────────────────────────────────────

    /// Pump the native event loop. Returns false after the last window closes.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn tick(&self) -> Result<bool> {
        let initialized = *self.initialized.lock();
        if !initialized {
            return Err(Error::from_reason(
                "Renderer not initialized. Call init() first.",
            ));
        }
        if *self.headless.lock() {
            return Ok(true);
        }

        #[cfg(target_os = "macos")]
        {
            if self
                .pending_gpu_repaint
                .swap(false, std::sync::atomic::Ordering::AcqRel)
            {
                self.request_invalidate()?;
            }
            let running = MAC_PLATFORM.with(|p| {
                p.borrow()
                    .as_ref()
                    .is_some_and(|platform| platform.pump_events())
            });
            Ok(running)
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
        *self.initialized.lock()
    }

    /// Whether JavaScript must drive the native event loop with `tick()`.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn requires_tick(&self) -> bool {
        !*self.headless.lock() && cfg!(target_os = "macos")
    }

    /// First-party renderers emit window lifecycle events through the normal
    /// callback, allowing JS to avoid timer-based liveness and size polling.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn supports_window_events(&self) -> bool {
        true
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
                width: f64::from(f32::from(size.width)),
                height: f64::from(f32::from(size.height)),
            }
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetWindowSize { response })?;
            recv_ui_response(receiver, "the window size query")
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
        self.tree.lock().canvas_frames.lock().clear();
        if !*self.initialized.lock() {
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
        if !*self.headless.lock()
            && let Some(sender) = self.ui_commands.lock().take()
        {
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

        *self.headless.lock() = false;
        *self.initialized.lock() = false;
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
            self.debug_frame_overlay_mode()
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
            recv_ui_response(receiver, "the debug frame overlay query")
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

    #[allow(clippy::unused_self)] // N-API instance method over thread-local renderer state.
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

    /// Bring the window forward and focus it. This also reveals a window opened
    /// with `show: false`.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn activate_window(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window(|_view, window, cx| {
            cx.activate(true);
            window.activate_window();
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::ActivateWindow);

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, |_view, window, cx| {
            cx.activate(true);
            window.activate_window();
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
    pub fn focus_next(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window_without_view(gpui::Window::focus_next);
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::FocusNext);
        #[cfg(target_family = "wasm")]
        return update_web_window_without_view(self.web_renderer_id, |window, cx| {
            window.focus_next(cx)
        })
        .map(|_| ());
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn focus_previous(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window_without_view(gpui::Window::focus_prev);
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::FocusPrevious);
        #[cfg(target_family = "wasm")]
        return update_web_window_without_view(self.web_renderer_id, |window, cx| {
            window.focus_prev(cx)
        })
        .map(|_| ());
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn set_window_key_events(&self, key_down: bool, key_up: bool, event_id: f64) -> Result<()> {
        let event_id = to_element_id(event_id)?;
        #[cfg(target_os = "macos")]
        return update_window(move |view, window, cx| {
            view.window_key_down = key_down;
            view.window_key_up = key_up;
            view.window_key_event_id = event_id;
            cx.notify();
            window.refresh();
        });
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        return self.send_ui_command(UiCommand::SetWindowKeyEvents {
            key_down,
            key_up,
            event_id,
        });
        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |view, window, cx| {
            view.window_key_down = key_down;
            view.window_key_up = key_up;
            view.window_key_event_id = event_id;
            cx.notify();
            window.refresh();
        })
        .map(|_| ());
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

    /// Scroll a child into view by index, optionally preserving a pixel offset
    /// inside that item. Negative offsets are valid for prepend restoration.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn scroll_to_item(
        &self,
        element_id: f64,
        index: f64,
        offset_in_item: Option<f64>,
    ) -> Result<()> {
        let id = to_element_id(element_id)?;
        let index = index as usize;
        let offset = offset_in_item.unwrap_or(0.0) as f32;
        #[cfg(any(target_os = "macos", target_family = "wasm"))]
        if !VIRTUAL_LIST_STATES.with(|cell| {
            if !cell.borrow().contains_key(&id) {
                return false;
            }
            queue_virtual_list_scroll(id, index, offset);
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
        return self.send_ui_command(UiCommand::ScrollToItem { id, index, offset });

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_family = "wasm"
        )))]
        Err(Error::from_reason("Unsupported operating system"))
    }

    /// Return `[itemIndex, offsetInItemPx, viewportHeightPx]` for a virtual
    /// list, or null for an ordinary element.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_list_scroll_top(&self, element_id: f64) -> Result<Option<Vec<f64>>> {
        let id = to_element_id(element_id)?;
        #[cfg(any(target_os = "macos", target_family = "wasm"))]
        return Ok(VIRTUAL_LIST_STATES.with(|cell| {
            cell.borrow().get(&id).map(|state| {
                let top = state.logical_scroll_top();
                vec![
                    top.item_ix as f64,
                    f64::from(f32::from(top.offset_in_item)),
                    f64::from(f32::from(state.viewport_bounds().size.height)),
                ]
            })
        }));

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetListScrollTop { id, response })?;
            Ok(recv_ui_response(receiver, "the GPUI list scroll query")?.map(|top| top.to_vec()))
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
            Ok(recv_ui_response(receiver, "the GPUI scroll query")?.map(|[x, y]| vec![x, y]))
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
        let tree = self.tree.lock();
        let json = tree.to_automation_json(&bounds);
        serde_json::to_string(&json)
            .map_err(|e| Error::from_reason(format!("JSON serialization failed: {e}")))
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
        let tree = self.tree.lock();
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
        self.dispatch_key_input(KeyInput::Keystrokes(keystrokes))
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_key_down(&self, keystroke: String, is_held: Option<bool>) -> Result<()> {
        let is_held = is_held.unwrap_or(false);

        self.dispatch_key_input(KeyInput::Down { keystroke, is_held })
    }

    #[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), napi)]
    pub fn simulate_key_up(&self, keystroke: String) -> Result<()> {
        self.dispatch_key_input(KeyInput::Up(keystroke))
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
        self.dispatch_mouse_input(MouseInput::Click {
            x,
            y,
            button,
            modifiers,
        })
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
        self.dispatch_mouse_input(MouseInput::Down {
            x,
            y,
            button,
            modifiers,
        })
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
        self.dispatch_mouse_input(MouseInput::Up {
            x,
            y,
            button,
            modifiers,
        })
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
        self.dispatch_mouse_input(MouseInput::Move {
            x,
            y,
            pressed_button,
            modifiers,
        })
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
        self.dispatch_mouse_input(MouseInput::Wheel {
            x,
            y,
            delta_x,
            delta_y,
            modifiers,
        })
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
        if *self.initialized.lock() {
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
        let mut audio = self.audio.lock();
        audio.configure(sample_rate, channels, capacity as usize);
        Ok(AudioBufferState::from(audio.snapshot()))
    }

    /// Enqueue one decoded chunk without JSON serialization or per-sample FFI calls.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    #[cfg(not(target_family = "wasm"))]
    pub fn enqueue_audio_frames(&self, samples: Float32Array) -> Result<AudioBufferState> {
        self.enqueue_audio_slice(samples.as_ref())
    }

    pub(super) fn enqueue_audio_slice(&self, samples: &[f32]) -> Result<AudioBufferState> {
        if samples.len() > 20_000_000 {
            return Err(Error::from_reason(
                "an audio chunk may contain at most 20M samples",
            ));
        }
        let mut audio = self.audio.lock();
        audio.push(samples).map_err(Error::from_reason)?;
        Ok(AudioBufferState::from(audio.snapshot()))
    }

    /// Consume up to `maxFrames` from a native audio backend or custom host.
    #[cfg_attr(not(target_family = "wasm"), napi)]
    #[cfg(not(target_family = "wasm"))]
    pub fn dequeue_audio_frames(&self, max_frames: u32) -> Float32Array {
        self.dequeue_audio_vec(max_frames).into()
    }

    pub(super) fn dequeue_audio_vec(&self, max_frames: u32) -> Vec<f32> {
        self.audio.lock().pop(max_frames as usize)
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn clear_audio_frames(&self) -> AudioBufferState {
        let mut audio = self.audio.lock();
        audio.clear();
        AudioBufferState::from(audio.snapshot())
    }

    #[cfg_attr(not(target_family = "wasm"), napi)]
    pub fn get_audio_buffer_state(&self) -> AudioBufferState {
        AudioBufferState::from(self.audio.lock().snapshot())
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
            .map_err(|e| Error::from_reason(format!("Screenshot capture failed: {e}")))?;
            image
                .save(&path)
                .map_err(|e| Error::from_reason(format!("Failed to save screenshot: {e}")))?;
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
