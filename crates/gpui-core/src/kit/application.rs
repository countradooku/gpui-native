//! Application composition: settings pages, anchored chat history and window chrome.
use super::elements::{integer, kit_text, number, size, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, Context, Entity, Window, prelude::*};
use gpui_component::{
    Sizable as _, TitleBar, WindowBorder,
    message_scroller::{MessageScroller, MessageScrollerState},
    setting::{SelectIndex, SettingGroup, SettingItem, SettingPage, Settings},
};
use serde_json::Value;
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-settings",
        &["pages", "defaultSelectedIndex", "sidebarWidth", "size"],
    ),
    (
        "kit-message-scroller",
        &["firstIndex", "scrollbar", "jumpButton"],
    ),
    ("kit-title-bar", &[]),
    ("kit-window-border", &["shadowSize", "resizeHitSize"]),
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
        Box::new(Control {
            name: self.name,
            props: self.props,
            scroller: None,
            first: 0,
            revision: None,
        })
    }
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    scroller: Option<Entity<MessageScrollerState>>,
    first: usize,
    revision: Option<u64>,
}
impl CustomElement for Control {
    fn set_prop(&mut self, _: &str, _: Value) {}
    fn supported_props(&self) -> &'static [&'static str] {
        self.props
    }
    fn supported_events(&self) -> &'static [&'static str] {
        &[
            "click",
            "auxClick",
            "mouseDown",
            "mouseUp",
            "mouseMove",
            "mouseDownOutside",
            "mouseEnter",
            "mouseLeave",
            "keyDown",
            "keyUp",
            "focus",
            "blur",
            "scroll",
            "highlight",
        ]
    }
    fn destroy(&mut self) {
        self.scroller = None;
        self.revision = None;
    }
    #[allow(
        clippy::too_many_lines,
        reason = "application-level builders share deferred framework content"
    )]
    fn render(
        &mut self,
        mut ctx: CustomRenderContext,
        _window: &mut Window,
        cx: &mut Context<GpuiView>,
    ) -> AnyElement {
        let id = custom_element_id("gpui-kit", ctx.id);
        let children = std::mem::take(&mut ctx.children);
        let component = match self.name {
            "kit-title-bar" => TitleBar::new().children(children).into_any_element(),
            "kit-window-border" => WindowBorder::new()
                .shadow_size(number(&ctx, "shadowSize", 12.))
                .resize_hit_size(number(&ctx, "resizeHitSize", 8.))
                .children(children)
                .into_any_element(),
            "kit-settings" => {
                let Some(render) = ctx.render_children.clone() else {
                    return surface(&ctx).into_any_element();
                };
                let mut settings = Settings::new(id)
                    .with_size(size(&ctx))
                    .sidebar_width(number(&ctx, "sidebarWidth", 220.))
                    .default_selected_index(SelectIndex {
                        page_ix: integer(&ctx, "defaultSelectedIndex", 0),
                        group_ix: None,
                    });
                let mut child = 0;
                let mut text_index = 0;
                for page in ctx
                    .props
                    .get("pages")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let mut native_page = SettingPage::new(field(page, "title"))
                        .description(field(page, "description"));
                    for group in page
                        .get("groups")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let mut native_group = SettingGroup::new()
                            .title(field(group, "title"))
                            .description(field(group, "description"));
                        for item in group
                            .get("items")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                        {
                            let title = kit_text(&ctx, text_index, field(item, "title"));
                            let description =
                                kit_text(&ctx, text_index + 1, field(item, "description"));
                            text_index += 2;
                            let index = child;
                            child += 1;
                            let render = render.clone();
                            let keywords = [field(item, "title"), field(item, "description")];
                            let disabled = item
                                .get("disabled")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            native_group = native_group.item(
                                SettingItem::render(move |_, window, cx| {
                                    gpui::div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_4()
                                        .child(
                                            gpui::div()
                                                .flex()
                                                .flex_col()
                                                .child(title.clone())
                                                .child(description.clone()),
                                        )
                                        .child(render(Some(index), window, cx))
                                })
                                .keywords(keywords)
                                .disabled(disabled),
                            );
                        }
                        native_page = native_page.group(native_group);
                    }
                    settings = settings.page(native_page);
                }
                settings.into_any_element()
            }
            "kit-message-scroller" => {
                let Some(render) = ctx.render_children.clone() else {
                    return surface(&ctx).into_any_element();
                };
                let first = integer(&ctx, "firstIndex", 0);
                let created = self.scroller.is_none();
                let state = self.scroller.get_or_insert_with(|| {
                    cx.new(|cx| MessageScrollerState::new(ctx.child_count, cx))
                });
                state.update(cx, |state, cx| {
                    let old = state.item_count();
                    if !created {
                        if first < self.first
                            && ctx.child_count >= old
                            && self.first - first == ctx.child_count - old
                        {
                            state.prepend(ctx.child_count - old, cx);
                        } else if first == self.first && ctx.child_count >= old {
                            if ctx.child_count > old {
                                state.append(ctx.child_count - old, cx);
                            }
                        } else if first != self.first || ctx.child_count != old {
                            state.reset(ctx.child_count, cx);
                        }
                        if self.revision != Some(ctx.revision) {
                            state.remeasure(cx);
                        }
                    }
                });
                self.first = first;
                MessageScroller::new(id, state.clone(), move |index, window, cx| {
                    render(Some(index), window, cx)
                })
                .scrollbar(
                    ctx.props
                        .get("scrollbar")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                )
                .jump_button(
                    ctx.props
                        .get("jumpButton")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                )
                .into_any_element()
            }
            _ => unreachable!("registered application composition"),
        };
        self.revision = Some(ctx.revision);
        surface(&ctx).child(component).into_any_element()
    }
}
fn field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
