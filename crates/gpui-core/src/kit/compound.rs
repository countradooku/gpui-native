//! Compound Kit controls compose retained children with typed item metadata.

use super::elements::{boolean, change, integer, kit_text, size, string};
use crate::custom_elements::{
    CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    custom_element_id,
};
use crate::renderer::GpuiView;
use gpui::{AnyElement, Context, Window, prelude::*};
use gpui_component::Sizable as _;
use serde_json::Value;

pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    ("kit-tabs", &["items", "selectedIndex", "variant", "size"]),
    (
        "kit-radio-group",
        &["items", "selectedIndex", "disabled", "size"],
    ),
    (
        "kit-accordion",
        &[
            "items",
            "openIndices",
            "multiple",
            "bordered",
            "disabled",
            "size",
        ],
    ),
    (
        "kit-description-list",
        &["items", "columns", "vertical", "bordered", "size"],
    ),
    ("kit-breadcrumb", &["items"]),
    ("kit-alert", &["title", "message", "variant"]),
    ("kit-empty", &["title", "description"]),
    ("kit-icon", &["name", "size"]),
    ("kit-kbd", &["keystroke"]),
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
        })
    }
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
}
fn text(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .or_else(|| item.as_str())
        .unwrap_or_default()
        .to_owned()
}
fn flag(item: &Value, key: &str) -> bool {
    item.get(key).and_then(Value::as_bool).unwrap_or(false)
}
impl CustomElement for Control {
    fn set_prop(&mut self, _: &str, _: Value) {}
    fn supported_props(&self) -> &'static [&'static str] {
        self.props
    }
    fn supported_events(&self) -> &'static [&'static str] {
        &[
            "change",
            "mouseEnter",
            "mouseLeave",
            "keyDown",
            "keyUp",
            "focus",
            "blur",
            "click",
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
        let id = ctx.id;
        let native_id = custom_element_id("gpui-kit", id);
        let callback = ctx.event_callback.clone();
        let items = ctx
            .props
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut children = std::mem::take(&mut ctx.children).into_iter();
        let component = match self.name {
            "kit-tabs" => {
                let tabs = items.iter().enumerate().map(|(i, item)| {
                    gpui_component::tab::Tab::new()
                        .disabled(flag(item, "disabled"))
                        .child(ctx.text(i, text(item, "label"), None))
                });
                let mut bar = gpui_component::tab::TabBar::new(native_id)
                    .selected_index(integer(&ctx, "selectedIndex", 0))
                    .children(tabs)
                    .with_size(size(&ctx));
                bar = match string(&ctx, "variant").as_str() {
                    "pill" => bar.pill(),
                    "outline" => bar.outline(),
                    "segmented" => bar.segmented(),
                    _ => bar.underline(),
                };
                let selected = integer(&ctx, "selectedIndex", 0);
                gpui::div()
                    .flex()
                    .flex_col()
                    .child(bar.on_click(move |i, _, _| change(&callback, id, *i)))
                    .children(children.nth(selected))
                    .into_any_element()
            }
            "kit-radio-group" => {
                let group = gpui_component::radio::RadioGroup::new(native_id)
                    .selected_index(
                        ctx.props
                            .get("selectedIndex")
                            .and_then(Value::as_u64)
                            .and_then(|v| usize::try_from(v).ok()),
                    )
                    .disabled(boolean(&ctx, "disabled"));
                group
                    .children(items.iter().enumerate().map(|(i, item)| {
                        gpui_component::radio::Radio::new(("radio", i))
                            .with_size(size(&ctx))
                            .disabled(flag(item, "disabled"))
                            .label(kit_text(&ctx, i, text(item, "label")))
                    }))
                    .on_change(move |i, _, _| change(&callback, id, *i))
                    .into_any_element()
            }
            "kit-accordion" => {
                let mut accordion = gpui_component::accordion::Accordion::new(native_id)
                    .multiple(boolean(&ctx, "multiple"))
                    .bordered(
                        ctx.props
                            .get("bordered")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    )
                    .disabled(boolean(&ctx, "disabled"))
                    .with_size(size(&ctx));
                for (i, item) in items.iter().enumerate() {
                    let child = children.next();
                    let open = ctx
                        .props
                        .get("openIndices")
                        .and_then(Value::as_array)
                        .is_some_and(|v| v.contains(&Value::from(i)));
                    accordion =
                        accordion.item(|entry| {
                            entry
                                .title(ctx.text(i * 2, text(item, "title"), None))
                                .open(open)
                                .disabled(flag(item, "disabled"))
                                .children(child.or_else(|| {
                                    Some(ctx.text(i * 2 + 1, text(item, "content"), None))
                                }))
                        });
                }
                accordion
                    .on_toggle_click(move |indices, _, _| {
                        change(&callback, id, serde_json::json!(indices));
                    })
                    .into_any_element()
            }
            "kit-description-list" => {
                let mut list = if boolean(&ctx, "vertical") {
                    gpui_component::description_list::DescriptionList::vertical()
                } else {
                    gpui_component::description_list::DescriptionList::horizontal()
                };
                list = list
                    .columns(integer(&ctx, "columns", 1))
                    .bordered(boolean(&ctx, "bordered"))
                    .with_size(size(&ctx));
                list.children(items.iter().enumerate().map(|(i, item)| {
                    gpui_component::description_list::DescriptionItem::new(ctx.text(
                        i * 2,
                        text(item, "label"),
                        None,
                    ))
                    .value(
                        children
                            .next()
                            .unwrap_or_else(|| ctx.text(i * 2 + 1, text(item, "value"), None)),
                    )
                }))
                .into_any_element()
            }
            "kit-breadcrumb" => gpui_component::breadcrumb::Breadcrumb::new()
                .children(items.iter().enumerate().map(|(i, item)| {
                    let callback = callback.clone();
                    gpui_component::breadcrumb::BreadcrumbItem::new(kit_text(
                        &ctx,
                        i,
                        text(item, "label"),
                    ))
                    .disabled(flag(item, "disabled"))
                    .on_click(move |_, _, _| change(&callback, id, i))
                }))
                .into_any_element(),
            "kit-alert" => {
                use gpui_component::alert::{Alert, AlertVariant};
                let variant = match string(&ctx, "variant").as_str() {
                    "info" => AlertVariant::Info,
                    "success" => AlertVariant::Success,
                    "warning" => AlertVariant::Warning,
                    "error" => AlertVariant::Error,
                    _ => AlertVariant::Default,
                };
                Alert::new(native_id, kit_text(&ctx, 0, string(&ctx, "message")))
                    .title(kit_text(&ctx, 1, string(&ctx, "title")))
                    .with_variant(variant)
                    .into_any_element()
            }
            "kit-empty" => {
                use gpui_component::empty::{
                    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyTitle,
                };
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .title(EmptyTitle::new().child(ctx.text(
                                0,
                                string(&ctx, "title"),
                                None,
                            )))
                            .description(EmptyDescription::new().child(ctx.text(
                                1,
                                string(&ctx, "description"),
                                None,
                            ))),
                    )
                    .content(EmptyContent::new().children(children))
                    .into_any_element()
            }
            "kit-icon" => {
                let path = format!("icons/{}.svg", string(&ctx, "name"));
                gpui_component::Icon::default()
                    .path(path)
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            "kit-kbd" => gpui::Keystroke::parse(&string(&ctx, "keystroke")).map_or_else(
                |_| gpui::Empty.into_any_element(),
                |key| gpui_component::kbd::Kbd::new(key).into_any_element(),
            ),
            _ => unreachable!("registered compound Kit control"),
        };
        super::elements::surface(&ctx)
            .child(component)
            .into_any_element()
    }
}
