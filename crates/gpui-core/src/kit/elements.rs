//! Native Kit adapters. Props are reapplied from the retained snapshot each frame.

use gpui::{AnyElement, Context, Window, prelude::*};
use gpui_component::{Disableable as _, Selectable as _, Sizable as _, Size};
use serde_json::Value;

use crate::custom_elements::{
    CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    custom_element_id,
};
use crate::renderer::{EventCallback, GpuiView, emit_event_full};

pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-button",
        &[
            "label", "variant", "size", "disabled", "selected", "loading", "outline", "compact",
            "tooltip",
        ],
    ),
    ("kit-checkbox", &["label", "checked", "disabled", "size"]),
    ("kit-switch", &["label", "checked", "disabled", "size"]),
    ("kit-radio", &["label", "checked", "disabled", "size"]),
    ("kit-toggle", &["label", "checked", "disabled", "size"]),
    ("kit-badge", &["count", "max", "dot", "size"]),
    ("kit-tag", &["label", "variant", "size", "outline"]),
    ("kit-spinner", &["size"]),
    ("kit-skeleton", &["secondary"]),
    ("kit-separator", &["vertical", "dashed", "label"]),
    ("kit-progress", &["value"]),
    ("kit-progress-circle", &["value"]),
    ("kit-rating", &["value", "max", "disabled", "size"]),
    (
        "kit-pagination",
        &[
            "page",
            "totalPages",
            "visiblePages",
            "compact",
            "disabled",
            "size",
        ],
    ),
    ("kit-link", &["href", "label", "disabled"]),
    ("kit-avatar", &["src", "name", "size"]),
    ("kit-group-box", &["title"]),
    ("kit-collapsible", &["open"]),
];

pub(crate) fn register(registry: &mut CustomElementRegistry) {
    for &(name, props) in COMPONENTS {
        registry.register(Box::new(Factory { name, props }));
    }
}

struct Factory {
    name: &'static str,
    props: &'static [&'static str],
}
impl CustomElementFactory for Factory {
    fn element_type(&self) -> &str {
        self.name
    }
    fn create(&self, _: u64) -> Box<dyn CustomElement> {
        Box::new(KitElement {
            name: self.name,
            props: self.props,
        })
    }
}
struct KitElement {
    name: &'static str,
    props: &'static [&'static str],
}

pub(super) fn string(ctx: &CustomRenderContext<'_>, key: &str) -> String {
    ctx.props
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
pub(super) fn boolean(ctx: &CustomRenderContext<'_>, key: &str) -> bool {
    ctx.props.get(key).and_then(Value::as_bool).unwrap_or(false)
}
pub(super) fn integer(ctx: &CustomRenderContext<'_>, key: &str, default: usize) -> usize {
    ctx.props
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(default)
}
pub(super) fn number(ctx: &CustomRenderContext<'_>, key: &str, default: f32) -> f32 {
    #[allow(clippy::cast_possible_truncation, reason = "GPUI geometry uses f32")]
    ctx.props
        .get(key)
        .and_then(Value::as_f64)
        .map_or(default, |v| v as f32)
}
#[allow(
    clippy::ref_option,
    reason = "matches the shared renderer event callback contract"
)]
pub(super) fn change(callback: &Option<EventCallback>, id: u64, value: impl Into<Value>) {
    emit_event_full(callback, id, "change", |event| {
        event.value = Some(value.into().to_string());
    });
}
pub(super) fn size(ctx: &CustomRenderContext<'_>) -> Size {
    match ctx.props.get("size").and_then(Value::as_str) {
        Some("xsmall") => Size::XSmall,
        Some("small") => Size::Small,
        Some("large") => Size::Large,
        _ => Size::Medium,
    }
}

pub(super) fn kit_text(
    ctx: &CustomRenderContext<'_>,
    sub: usize,
    value: String,
) -> gpui_component::text::Text {
    let value: gpui::SharedString = value.into();
    let selection = ctx.selection.clone();
    let highlight = ctx.highlight_set.clone();
    let selectable = ctx.selectable;
    let wash = ctx.selection_wash;
    let id = ctx.id;
    let content = value.clone();
    gpui_component::text::Text::custom(value, move || {
        crate::text::selectable_text(crate::text::SelectableText {
            selectable,
            highlight: highlight.clone().map(crate::text::HighlightSource::Native),
            ..crate::text::SelectableText::new(
                id,
                sub,
                content.clone(),
                None,
                selection.clone(),
                wash,
            )
        })
    })
}

impl CustomElement for KitElement {
    fn set_prop(&mut self, _: &str, _: Value) {}
    fn supported_props(&self) -> &'static [&'static str] {
        self.props
    }
    fn supported_events(&self) -> &'static [&'static str] {
        &[
            "click",
            "change",
            "mouseEnter",
            "mouseLeave",
            "focus",
            "blur",
            "keyDown",
            "keyUp",
            "auxClick",
            "mouseDown",
            "mouseUp",
            "mouseMove",
            "mouseDownOutside",
            "scroll",
            "highlight",
        ]
    }
    fn destroy(&mut self) {}

    #[allow(
        clippy::too_many_lines,
        reason = "each native builder stays beside its prop and event mapping"
    )]
    fn render(
        &mut self,
        mut ctx: CustomRenderContext,
        _: &mut Window,
        _: &mut Context<GpuiView>,
    ) -> AnyElement {
        use gpui_component::{button::ButtonVariants as _, tag::TagVariant};
        let id = ctx.id;
        let native_id = custom_element_id("gpui-kit", id);
        let callback = ctx.event_callback.clone();
        let disabled = boolean(&ctx, "disabled");
        let checked = boolean(&ctx, "checked");
        let control_size = size(&ctx);
        let label = string(&ctx, "label");
        let mut children = std::mem::take(&mut ctx.children);
        if !label.is_empty() {
            children.insert(0, ctx.text(0, label.clone(), None));
        }
        let component = match self.name {
            "kit-button" => {
                let mut button = gpui_component::button::Button::new(native_id)
                    .when_some(ctx.focus_handle, |this, handle| {
                        this.with_focus_handle(handle.clone())
                    })
                    .accessibility_label(label.clone())
                    .disabled(disabled)
                    .selected(boolean(&ctx, "selected"))
                    .loading(boolean(&ctx, "loading"))
                    .with_size(control_size);
                button = match string(&ctx, "variant").as_str() {
                    "primary" => button.primary(),
                    "secondary" => button.secondary(),
                    "danger" => button.danger(),
                    "success" => button.success(),
                    "warning" => button.warning(),
                    "info" => button.info(),
                    "ghost" => button.ghost(),
                    "link" => button.link(),
                    _ => button,
                };
                if boolean(&ctx, "outline") {
                    button = button.outline();
                }
                if boolean(&ctx, "compact") {
                    button = button.compact();
                }
                let tooltip = string(&ctx, "tooltip");
                if !tooltip.is_empty() {
                    button = button.tooltip(tooltip);
                }
                button
                    .children(children)
                    .on_click(move |_, _, _| emit_event_full(&callback, id, "click", |_| {}))
                    .into_any_element()
            }
            "kit-checkbox" => gpui_component::checkbox::Checkbox::new(native_id)
                .when_some(ctx.focus_handle, |this, handle| {
                    this.with_focus_handle(handle.clone())
                })
                .accessibility_label(label.clone())
                .checked(checked)
                .disabled(disabled)
                .with_size(control_size)
                .children(children)
                .on_change(move |v, _, _| change(&callback, id, *v))
                .into_any_element(),
            "kit-switch" => gpui_component::switch::Switch::new(native_id)
                .when_some(ctx.focus_handle, |this, handle| {
                    this.with_focus_handle(handle.clone())
                })
                .checked(checked)
                .disabled(disabled)
                .with_size(control_size)
                .label(kit_text(&ctx, 0, label))
                .on_change(move |v, _, _| change(&callback, id, *v))
                .into_any_element(),
            "kit-radio" => gpui_component::radio::Radio::new(native_id)
                .when_some(ctx.focus_handle, |this, handle| {
                    this.with_focus_handle(handle.clone())
                })
                .accessibility_label(label.clone())
                .checked(checked)
                .disabled(disabled)
                .with_size(control_size)
                .children(children)
                .on_change(move |v, _, _| change(&callback, id, *v))
                .into_any_element(),
            "kit-toggle" => gpui_component::button::Toggle::new(native_id)
                .when_some(ctx.focus_handle, |this, handle| {
                    this.with_focus_handle(handle.clone())
                })
                .checked(checked)
                .disabled(disabled)
                .with_size(control_size)
                .children(children)
                .on_click(move |v, _, _| change(&callback, id, *v))
                .into_any_element(),
            "kit-badge" => {
                let mut badge = gpui_component::badge::Badge::new()
                    .count(integer(&ctx, "count", 0))
                    .max(integer(&ctx, "max", 99))
                    .with_size(control_size);
                if boolean(&ctx, "dot") {
                    badge = badge.dot();
                }
                badge.children(children).into_any_element()
            }
            "kit-tag" => {
                let variant = match string(&ctx, "variant").as_str() {
                    "secondary" => TagVariant::Secondary,
                    "danger" => TagVariant::Danger,
                    "success" => TagVariant::Success,
                    "warning" => TagVariant::Warning,
                    "info" => TagVariant::Info,
                    _ => TagVariant::Primary,
                };
                let mut tag = gpui_component::tag::Tag::new()
                    .with_variant(variant)
                    .with_size(control_size);
                if boolean(&ctx, "outline") {
                    tag = tag.outline();
                }
                tag.children(children).into_any_element()
            }
            "kit-spinner" => gpui_component::spinner::Spinner::new()
                .with_size(control_size)
                .into_any_element(),
            "kit-skeleton" => {
                let mut skeleton = gpui_component::skeleton::Skeleton::new();
                if boolean(&ctx, "secondary") {
                    skeleton = skeleton.secondary();
                }
                skeleton.into_any_element()
            }
            "kit-separator" => {
                let mut separator = if boolean(&ctx, "vertical") {
                    gpui_component::separator::Separator::vertical()
                } else {
                    gpui_component::separator::Separator::horizontal()
                };
                if boolean(&ctx, "dashed") {
                    separator = separator.dashed();
                }
                if !label.is_empty() {
                    separator = separator.label(kit_text(&ctx, 0, label));
                }
                separator.into_any_element()
            }
            "kit-progress" => gpui_component::progress::Progress::new(native_id)
                .value(number(&ctx, "value", 0.))
                .into_any_element(),
            "kit-progress-circle" => gpui_component::progress::ProgressCircle::new(native_id)
                .value(number(&ctx, "value", 0.))
                .into_any_element(),
            "kit-rating" => gpui_component::rating::Rating::new(native_id)
                .max(integer(&ctx, "max", 5))
                .value(integer(&ctx, "value", 0))
                .disabled(disabled)
                .with_size(control_size)
                .on_click(move |v, _, _| change(&callback, id, *v))
                .into_any_element(),
            "kit-pagination" => {
                let mut pagination = gpui_component::pagination::Pagination::new(native_id)
                    .total_pages(integer(&ctx, "totalPages", 1))
                    .current_page(integer(&ctx, "page", 1))
                    .visible_pages(integer(&ctx, "visiblePages", 5))
                    .disabled(disabled)
                    .with_size(control_size);
                if boolean(&ctx, "compact") {
                    pagination = pagination.compact();
                }
                pagination
                    .on_click(move |v, _, _| change(&callback, id, *v))
                    .into_any_element()
            }
            "kit-link" => gpui_component::link::Link::new(native_id)
                .href(string(&ctx, "href"))
                .disabled(disabled)
                .children(children)
                .into_any_element(),
            "kit-avatar" => {
                let mut avatar = gpui_component::avatar::Avatar::new()
                    .name(string(&ctx, "name"))
                    .with_size(control_size);
                let src = string(&ctx, "src");
                if !src.is_empty() {
                    avatar = avatar.src(src);
                }
                avatar.into_any_element()
            }
            "kit-group-box" => gpui_component::group_box::GroupBox::new()
                .id(native_id)
                .title(ctx.text(1, string(&ctx, "title"), None))
                .children(children)
                .into_any_element(),
            "kit-collapsible" => gpui_component::collapsible::Collapsible::new()
                .motion_id(native_id)
                .open(boolean(&ctx, "open"))
                .content(gpui::div().children(children))
                .into_any_element(),
            _ => unreachable!("only registered Kit factories construct an adapter"),
        };
        // Kit owns activation; the wrapper owns retained layout, bounds, and accessibility.
        super::elements::surface(&ctx)
            .child(component)
            .into_any_element()
    }
}

/// Shared layout and accessibility for Kit roots. Activation stays on the control.
pub(super) fn surface(ctx: &CustomRenderContext<'_>) -> gpui::Stateful<gpui::Div> {
    let mut surface = gpui::div().id(custom_element_id("kit-host", ctx.id));
    if let Some(style) = ctx.style {
        surface = crate::renderer::apply_interactive_styles(surface, style);
    }
    surface = crate::accessibility::apply_accessibility(surface, ctx.props, None);
    if let Some(handle) = ctx.focus_handle
        && !matches!(
            ctx.element_type,
            "kit-button" | "kit-checkbox" | "kit-radio" | "kit-switch" | "kit-toggle"
        )
    {
        surface = surface.track_focus(handle);
    }
    let id = ctx.id;
    if ctx.events.contains("keyDown") {
        let callback = ctx.event_callback.clone();
        surface = surface.on_key_down(move |key, _, _| {
            emit_event_full(&callback, id, "keyDown", |p| {
                p.key = Some(key.keystroke.key.clone());
                p.key_char.clone_from(&key.keystroke.key_char);
                p.is_held = Some(key.is_held);
                p.modifiers = Some(key.keystroke.modifiers.into());
            });
        });
    }
    if ctx.events.contains("keyUp") {
        let callback = ctx.event_callback.clone();
        surface = surface.on_key_up(move |key, _, _| {
            emit_event_full(&callback, id, "keyUp", |p| {
                p.key = Some(key.keystroke.key.clone());
                p.key_char.clone_from(&key.keystroke.key_char);
                p.modifiers = Some(key.keystroke.modifiers.into());
            });
        });
    }
    let enter = ctx.events.contains("mouseEnter");
    let leave = ctx.events.contains("mouseLeave");
    if enter || leave {
        let callback = ctx.event_callback.clone();
        let id = ctx.id;
        surface = surface.on_hover(move |&hovered, _, _| {
            if (hovered && enter) || (!hovered && leave) {
                emit_event_full(
                    &callback,
                    id,
                    if hovered { "mouseEnter" } else { "mouseLeave" },
                    |event| event.hovered = Some(hovered),
                );
            }
        });
    }
    wire_pointer_events(surface, ctx).child(crate::automation::bounds_tracker(ctx.id, None))
}

#[allow(
    clippy::too_many_lines,
    reason = "keep related native prop and event mappings adjacent"
)]
fn wire_pointer_events(
    mut el: gpui::Stateful<gpui::Div>,
    ctx: &CustomRenderContext<'_>,
) -> gpui::Stateful<gpui::Div> {
    use crate::renderer::{mouse_button_to_u32, point_to_xy};
    let id = ctx.id;
    if !boolean(ctx, "disabled") && ctx.element_type != "kit-button" && ctx.events.contains("click")
    {
        let callback = ctx.event_callback.clone();
        el = el.on_mouse_up(gpui::MouseButton::Left, move |event, _, _| {
            emit_event_full(&callback, id, "click", |p| {
                let (x, y) = point_to_xy(event.position);
                p.x = Some(x);
                p.y = Some(y);
                p.button = Some(0);
                p.click_count = Some(event.click_count.try_into().unwrap_or(u32::MAX));
                p.modifiers = Some(event.modifiers.into());
            });
        });
    }
    for event_type in ctx.events.iter() {
        let callback = ctx.event_callback.clone();
        match event_type {
            "auxClick" => {
                el = el.on_aux_click(move |click_event, _window, _cx| {
                    emit_event_full(&callback, id, "auxClick", |p| {
                        let (x, y) = point_to_xy(click_event.position());
                        p.x = Some(x);
                        p.y = Some(y);
                        p.modifiers = Some(click_event.modifiers().into());
                        p.click_count =
                            Some(click_event.click_count().try_into().unwrap_or(u32::MAX));
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
                            p.click_count =
                                Some(mouse_event.click_count.try_into().unwrap_or(u32::MAX));
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
                            p.click_count =
                                Some(mouse_event.click_count.try_into().unwrap_or(u32::MAX));
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

            _ => {}
        }
    }
    el
}
