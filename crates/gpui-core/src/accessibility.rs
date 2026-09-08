//! GPUI accessibility props from framework custom props.
//!
//! A node reaches AccessKit only with both an `.id(...)` and a `.role(...)`.
//! GPUI Native already sets the id. This module sets the role and ARIA fields.
//! Prop names match React DOM: `aria-label`, not `ariaLabel`.

use std::collections::HashMap;

use gpui::prelude::*;
use gpui::{AccessibleAction, Role};

use crate::element_tree::EventPayload;
use crate::renderer::{EventCallback, emit_event_full};

/// Apply `role` and `aria-*` from the adapter, plus a default role when the app
/// did not set one. `none` / `presentation` produce no node.
pub(crate) fn apply_accessibility<E>(
    mut el: E,
    props: &HashMap<String, serde_json::Value>,
    default_role: Option<Role>,
) -> E
where
    E: StatefulInteractiveElement,
{
    let explicit = props.get("role").and_then(|value| value.as_str());
    if matches!(explicit, Some("none" | "presentation")) {
        return el;
    }

    let role = explicit
        .and_then(role_from_aria)
        .or(default_role)
        .filter(|role| *role != Role::GenericContainer);
    let Some(role) = role else {
        return el;
    };
    el = el.role(role);

    if let Some(id) = string_prop(props, "aria-id") {
        el = el.accessibility_id(id);
    }
    if let Some(label) = string_prop(props, "aria-label") {
        el = el.aria_label(label);
    }
    if let Some(description) = string_prop(props, "aria-description") {
        el = el.aria_description(description);
    }
    if let Some(value) = string_prop(props, "aria-valuetext") {
        el = el.aria_value(value);
    }
    if let Some(expanded) = bool_prop(props, "aria-expanded") {
        el = el.aria_expanded(expanded);
    }
    if let Some(selected) = bool_prop(props, "aria-selected") {
        el = el.aria_selected(selected);
    }
    if let Some(level) = usize_prop(props, "aria-level") {
        el = el.aria_level(level);
    }
    el
}

/// `aria-label`, or `alt` when that is empty. Used by `<img>`.
pub(crate) fn apply_image_label<E>(
    el: E,
    props: &HashMap<String, serde_json::Value>,
    alt: &str,
) -> E
where
    E: StatefulInteractiveElement,
{
    if string_prop(props, "aria-label").is_some() || alt.is_empty() {
        return el;
    }
    el.aria_label(alt.to_string())
}

/// `VoiceOver` Press. GPUI auto-adds Click only for `.on_click()`. GPUI Native
/// click is `on_mouse_up`, so register the action by hand and emit the
/// same JS `click` event.
pub(crate) fn apply_a11y_click<E>(
    el: E,
    events: &crate::retained_tree::EventSet,
    element_id: u64,
    callback: Option<&EventCallback>,
) -> E
where
    E: StatefulInteractiveElement,
{
    if !events.contains("click") {
        return el;
    }
    let callback = callback.cloned();
    el.on_a11y_action(AccessibleAction::Click, move |_data, _window, _cx| {
        emit_event_full(
            &callback,
            element_id,
            "click",
            |payload: &mut EventPayload| {
                payload.button = Some(0);
                payload.click_count = Some(1);
                payload.is_right_click = Some(false);
            },
        );
    })
}

fn string_prop(props: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn bool_prop(props: &HashMap<String, serde_json::Value>, key: &str) -> Option<bool> {
    props.get(key).and_then(serde_json::Value::as_bool)
}

fn usize_prop(props: &HashMap<String, serde_json::Value>, key: &str) -> Option<usize> {
    props
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
}

/// ARIA token → AccessKit role. Unknown tokens are ignored, like a bad style.
fn role_from_aria(token: &str) -> Option<Role> {
    Some(match token {
        "alert" => Role::Alert,
        "alertdialog" => Role::AlertDialog,
        "application" => Role::Application,
        "article" => Role::Article,
        "banner" => Role::Banner,
        "blockquote" => Role::Blockquote,
        "button" => Role::Button,
        "caption" => Role::Caption,
        "cell" => Role::Cell,
        "checkbox" => Role::CheckBox,
        "code" => Role::Code,
        "columnheader" => Role::ColumnHeader,
        "combobox" => Role::ComboBox,
        "complementary" => Role::Complementary,
        "contentinfo" => Role::ContentInfo,
        "definition" => Role::Definition,
        "deletion" => Role::ContentDeletion,
        "dialog" => Role::Dialog,
        "document" => Role::Document,
        "emphasis" => Role::Emphasis,
        "feed" => Role::Feed,
        "figure" => Role::Figure,
        "form" => Role::Form,
        "generic" | "none" | "presentation" | "subscript" | "superscript" => Role::GenericContainer,
        "grid" => Role::Grid,
        "gridcell" => Role::GridCell,
        "group" => Role::Group,
        "heading" => Role::Heading,
        "img" => Role::Image,
        "insertion" => Role::ContentInsertion,
        "link" => Role::Link,
        "list" => Role::List,
        "listbox" => Role::ListBox,
        "listitem" => Role::ListItem,
        "log" => Role::Log,
        "main" => Role::Main,
        "marquee" => Role::Marquee,
        "math" => Role::Math,
        "menu" => Role::Menu,
        "menubar" => Role::MenuBar,
        "menuitem" => Role::MenuItem,
        "menuitemcheckbox" => Role::MenuItemCheckBox,
        "menuitemradio" => Role::MenuItemRadio,
        "meter" => Role::Meter,
        "navigation" => Role::Navigation,
        "note" => Role::Note,
        "option" => Role::ListBoxOption,
        "paragraph" => Role::Paragraph,
        "progressbar" => Role::ProgressIndicator,
        "radio" => Role::RadioButton,
        "radiogroup" => Role::RadioGroup,
        "region" => Role::Region,
        "row" => Role::Row,
        "rowgroup" => Role::RowGroup,
        "rowheader" => Role::RowHeader,
        "scrollbar" => Role::ScrollBar,
        "search" => Role::Search,
        "searchbox" => Role::SearchInput,
        "separator" => Role::Splitter,
        "slider" => Role::Slider,
        "spinbutton" => Role::SpinButton,
        "status" => Role::Status,
        "strong" => Role::Strong,
        "switch" => Role::Switch,
        "tab" => Role::Tab,
        "table" => Role::Table,
        "tablist" => Role::TabList,
        "tabpanel" => Role::TabPanel,
        "term" => Role::Term,
        "textbox" => Role::TextInput,
        "time" => Role::Time,
        "timer" => Role::Timer,
        "toolbar" => Role::Toolbar,
        "tooltip" => Role::Tooltip,
        "tree" => Role::Tree,
        "treegrid" => Role::TreeGrid,
        "treeitem" => Role::TreeItem,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::role_from_aria;
    use gpui::Role;

    #[test]
    fn maps_common_aria_tokens() {
        assert_eq!(role_from_aria("button"), Some(Role::Button));
        assert_eq!(role_from_aria("heading"), Some(Role::Heading));
        assert_eq!(role_from_aria("textbox"), Some(Role::TextInput));
        assert_eq!(role_from_aria("img"), Some(Role::Image));
        assert_eq!(role_from_aria("option"), Some(Role::ListBoxOption));
        assert_eq!(role_from_aria("none"), Some(Role::GenericContainer));
        assert_eq!(role_from_aria("presentation"), Some(Role::GenericContainer));
        assert_eq!(role_from_aria("Button"), None);
        assert_eq!(role_from_aria("not-a-role"), None);
    }
}
