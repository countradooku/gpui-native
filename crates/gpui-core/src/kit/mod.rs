//! GPUI Kit initialization and window ownership shared by every host.

pub(crate) mod application;
pub(crate) mod charts;
pub(crate) mod choice;
pub(crate) mod collections;
pub(crate) mod compound;
pub(crate) mod content;
pub(crate) mod elements;
pub(crate) mod extras;
mod highlighter;
pub(crate) mod input;
pub(crate) mod layout;
pub(crate) mod lists;
pub(crate) mod menu;
pub(crate) mod overlays;
pub(crate) mod stateful;
pub(crate) mod table;
pub(crate) mod theme;
pub(crate) mod validation;
pub(crate) mod workspace;

use gpui::{AnyWindowHandle, AppContext, Context, Entity, Window, WindowHandle};
use gpui_component::Root;

use crate::renderer::GpuiView;

/// Keep the Kit root available to overlays while commands address the retained view.
#[derive(Clone, Copy)]
pub(crate) struct NativeWindow(WindowHandle<Root>);

impl NativeWindow {
    pub(crate) fn new(handle: WindowHandle<Root>) -> Self {
        Self(handle)
    }

    pub(crate) fn window_id(self) -> gpui::WindowId {
        self.0.window_id()
    }

    pub(crate) fn update<C: AppContext, R>(
        self,
        cx: &mut C,
        update: impl FnOnce(&mut GpuiView, &mut Window, &mut Context<GpuiView>) -> R,
    ) -> gpui::Result<R> {
        cx.update_window(self.into(), |root, window, cx| {
            let root = root
                .downcast::<Root>()
                .map_err(|_| std::io::Error::other("Kit window root changed"))?;
            let view = root
                .read(cx)
                .view()
                .clone()
                .downcast::<GpuiView>()
                .map_err(|_| std::io::Error::other("Kit window content changed"))?;
            Ok(view.update(cx, |view, cx| update(view, window, cx)))
        })?
    }

    #[cfg(all(
        feature = "test-support",
        any(target_os = "macos", target_os = "windows")
    ))]
    pub(crate) fn entity<C: AppContext>(self, cx: &C) -> gpui::Result<Entity<GpuiView>> {
        self.0
            .read_with(cx, |root, _| root.view().clone().downcast::<GpuiView>())?
            .map_err(|_| std::io::Error::other("Kit window content changed").into())
    }
}

impl From<NativeWindow> for AnyWindowHandle {
    fn from(window: NativeWindow) -> Self {
        window.0.into()
    }
}

pub(crate) fn wrap(
    view: Entity<GpuiView>,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Entity<Root> {
    cx.new(|cx| Root::new(view, window, cx).bordered(false))
}

pub(crate) fn init(cx: &mut gpui::App) {
    gpui_component::init(cx);
}
