//! The macOS application menu bar.
//!
//! GPUI never installs a main menu on its own, so `NSApp.mainMenu` stays nil
//! and macOS paints an empty menu bar. Worse, the standard key equivalents that
//! `AppKit` only provides through menu items are missing too, so a gpui-vue app
//! cannot be quit with cmd-q, hidden with cmd-h, or minimized with cmd-m.
//!
//! `gpui::App::set_menus` reads the key equivalent for each item out of the
//! keymap, so [`init`] must bind the keys before it sets the menus.
//!
//! There is deliberately no Edit menu. A menu key equivalent is consumed by
//! `AppKit` before the window sees the key event, so an Edit menu carrying cmd-c
//! would take the keystroke away from the window listener that
//! `crate::text::paint` installs for text selection, and from the per-focus
//! clipboard handling in `custom_elements::input`.

use gpui::{App, Menu, MenuItem, SystemMenuType};

gpui::actions!(
    gpuix_app,
    [
        /// Quit the application.
        Quit,
        /// Hide the application.
        Hide,
        /// Hide every other application.
        HideOthers,
        /// Unhide every other application.
        ShowAll,
        /// Minimize the focused window to the Dock.
        MinimizeWindow,
        /// Toggle the focused window between its standard and zoomed size.
        ZoomWindow,
        /// Close the focused window.
        CloseWindow,
    ]
);

/// Binds the standard macOS shortcuts, registers the app-level handlers, and
/// installs the menu bar. The window-level actions (`MinimizeWindow`,
/// `ZoomWindow`, `CloseWindow`) are handled by the root element in
/// `GpuiView::render`, which is the only place a `Window` exists.
pub(crate) fn init(app_name: &str, cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("cmd-q", Quit, None),
        gpui::KeyBinding::new("cmd-h", Hide, None),
        gpui::KeyBinding::new("cmd-alt-h", HideOthers, None),
        gpui::KeyBinding::new("cmd-m", MinimizeWindow, None),
        gpui::KeyBinding::new("cmd-w", CloseWindow, None),
    ]);

    cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
    cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
    cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());

    cx.set_menus(default_menus(app_name));
}

/// The App menu plus the Window menu, the minimum a macOS app is expected to
/// have. `create_menu_bar` gives the menu named `Window` to
/// `NSApplication.setWindowsMenu:`, which is what appends the window list.
pub(crate) fn default_menus(app_name: &str) -> Vec<Menu> {
    vec![
        Menu {
            name: app_name.to_string().into(),
            disabled: false,
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action(format!("Hide {app_name}"), Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action(format!("Quit {app_name}"), Quit),
            ],
        },
        // No "Enter Full Screen" item: AppKit prepends its own window-tiling
        // items, that one included, to whichever menu is given to
        // `setWindowsMenu:`. Adding ours produced two entries sharing a
        // shortcut.
        Menu {
            name: "Window".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Minimize", MinimizeWindow),
                MenuItem::action("Zoom", ZoomWindow),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_names(menu: &Menu) -> Vec<String> {
        menu.items
            .iter()
            .map(|item| match item {
                MenuItem::Separator => "-".to_string(),
                MenuItem::Submenu(submenu) => submenu.name.to_string(),
                MenuItem::SystemMenu(os_menu) => os_menu.name.to_string(),
                MenuItem::Action { name, .. } => name.to_string(),
            })
            .collect()
    }

    #[test]
    fn app_menu_is_named_after_the_app() {
        let menus = default_menus("Chat");
        assert_eq!(menus[0].name.as_ref(), "Chat");
        assert_eq!(
            item_names(&menus[0]),
            vec![
                "Services",
                "-",
                "Hide Chat",
                "Hide Others",
                "Show All",
                "-",
                "Quit Chat",
            ]
        );
    }

    // `create_menu_bar` only calls `setWindowsMenu:` for this exact name.
    #[test]
    fn window_menu_keeps_the_name_appkit_looks_for() {
        let menus = default_menus("Chat");
        assert_eq!(menus[1].name.as_ref(), "Window");
        assert_eq!(
            item_names(&menus[1]),
            vec!["Minimize", "Zoom", "-", "Close Window"]
        );
    }
}
