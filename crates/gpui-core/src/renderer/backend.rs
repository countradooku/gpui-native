//! Platform backend operations shared by the public renderer API.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::wildcard_imports,
    reason = "the backend spans mutually exclusive target-specific parent APIs and numeric representations"
)]

use super::*;

pub(super) trait Backend {
    fn event_callback_for_view(&self) -> Option<EventCallback>;
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn send_ui_command(&self, command: UiCommand) -> Result<()>;
    fn dispatch_mouse_input(&self, input: MouseInput) -> Result<()>;
    fn dispatch_key_input(&self, input: KeyInput) -> Result<()>;
    fn automation_bounds(&self) -> Result<HashMap<u64, crate::automation::ElementBounds>>;
    fn element_bounds(&self, id: u64) -> Result<Option<crate::automation::ElementBounds>>;
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn control_clock(&self, control: ClockControl) -> Result<f64>;
    fn request_invalidate(&self) -> Result<()>;
}

impl Backend for GpuiRenderer {
    fn event_callback_for_view(&self) -> Option<EventCallback> {
        self.event_callback.lock().clone()
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
    fn send_ui_command(&self, command: UiCommand) -> Result<()> {
        self.ui_commands
            .lock()
            .as_ref()
            .ok_or_else(|| Error::from_reason("GPUI application is not initialized"))?
            .unbounded_send(command)
            .map_err(|_| Error::from_reason("The GPUI UI thread is not running"))
    }

    fn dispatch_mouse_input(&self, input: MouseInput) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window(move |_view, window, cx| match input {
            MouseInput::Click {
                x,
                y,
                button,
                modifiers,
            } => crate::automation::dispatch_click(window, cx, x, y, button, modifiers),
            MouseInput::Down {
                x,
                y,
                button,
                modifiers,
            } => crate::automation::dispatch_mouse_down(window, cx, x, y, button, modifiers),
            MouseInput::Up {
                x,
                y,
                button,
                modifiers,
            } => crate::automation::dispatch_mouse_up(window, cx, x, y, button, modifiers),
            MouseInput::Move {
                x,
                y,
                pressed_button,
                modifiers,
            } => {
                crate::automation::dispatch_mouse_move(window, cx, x, y, pressed_button, modifiers)
            }
            MouseInput::Wheel {
                x,
                y,
                delta_x,
                delta_y,
                modifiers,
            } => crate::automation::dispatch_scroll_wheel(
                window, cx, x, y, delta_x, delta_y, modifiers,
            ),
        });

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response_sender, response_receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::DispatchMouse {
                input,
                response: response_sender,
            })?;
            recv_ui_response(response_receiver, "the GPUI UI command")?.map_err(Error::from_reason)
        }

        #[cfg(target_family = "wasm")]
        return update_web_window(self.web_renderer_id, move |_view, window, cx| match input {
            MouseInput::Click {
                x,
                y,
                button,
                modifiers,
            } => crate::automation::dispatch_click(window, cx, x, y, button, modifiers),
            MouseInput::Down {
                x,
                y,
                button,
                modifiers,
            } => crate::automation::dispatch_mouse_down(window, cx, x, y, button, modifiers),
            MouseInput::Up {
                x,
                y,
                button,
                modifiers,
            } => crate::automation::dispatch_mouse_up(window, cx, x, y, button, modifiers),
            MouseInput::Move {
                x,
                y,
                pressed_button,
                modifiers,
            } => {
                crate::automation::dispatch_mouse_move(window, cx, x, y, pressed_button, modifiers)
            }
            MouseInput::Wheel {
                x,
                y,
                delta_x,
                delta_y,
                modifiers,
            } => crate::automation::dispatch_scroll_wheel(
                window, cx, x, y, delta_x, delta_y, modifiers,
            ),
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
            let _ = input;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    fn dispatch_key_input(&self, input: KeyInput) -> Result<()> {
        #[cfg(target_os = "macos")]
        return update_window_without_view(move |window, cx| match input {
            KeyInput::Keystrokes(keystrokes) => {
                crate::automation::dispatch_keystrokes(window, cx, &keystrokes)
            }
            KeyInput::Down { keystroke, is_held } => {
                crate::automation::dispatch_key_down(window, cx, &keystroke, is_held)
            }
            KeyInput::Up(keystroke) => crate::automation::dispatch_key_up(window, cx, &keystroke),
        })?
        .map_err(Error::from_reason);

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response_sender, response_receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::DispatchKey {
                input,
                response: response_sender,
            })?;
            recv_ui_response(response_receiver, "the GPUI key command")?.map_err(Error::from_reason)
        }

        #[cfg(target_family = "wasm")]
        return update_web_window_without_view(
            self.web_renderer_id,
            move |window, cx| match input {
                KeyInput::Keystrokes(keystrokes) => {
                    crate::automation::dispatch_keystrokes(window, cx, &keystrokes)
                }
                KeyInput::Down { keystroke, is_held } => {
                    crate::automation::dispatch_key_down(window, cx, &keystroke, is_held)
                }
                KeyInput::Up(keystroke) => {
                    crate::automation::dispatch_key_up(window, cx, &keystroke)
                }
            },
        )?
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
            let _ = input;
            Err(Error::from_reason("Unsupported operating system"))
        }
    }

    fn automation_bounds(&self) -> Result<HashMap<u64, crate::automation::ElementBounds>> {
        #[cfg(target_os = "macos")]
        return Ok(crate::automation::all_bounds());

        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "freebsd"))]
        {
            let (response, receiver) = sync_channel(1);
            self.send_ui_command(UiCommand::GetAutomationBounds { response })?;
            recv_ui_response(receiver, "the automation bounds query")
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
            recv_ui_response(receiver, "the element bounds query")
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
        if *self.headless.lock() {
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
}
