//! Native composition primitives for application forms, navigation and messages.
use super::elements::{boolean, change, integer, kit_text, size, string, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, Context, Window, prelude::*};
use gpui_component::Sizable as _;
use serde_json::Value;
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    ("kit-form", &["items", "columns", "vertical", "size"]),
    ("kit-field", &["label", "description", "required"]),
    (
        "kit-stepper",
        &[
            "items",
            "selectedIndex",
            "vertical",
            "textCenter",
            "disabled",
            "size",
        ],
    ),
    (
        "kit-sidebar",
        &["items", "selectedIndex", "collapsed", "side"],
    ),
    ("kit-sidebar-header", &[]),
    ("kit-sidebar-footer", &[]),
    ("kit-sidebar-toggle", &["collapsed", "side"]),
    ("kit-status-bar", &[]),
    ("kit-clipboard", &["value", "tooltip"]),
    ("kit-message", &["alignment"]),
    ("kit-message-group", &[]),
    ("kit-message-header", &[]),
    ("kit-message-content", &[]),
    ("kit-message-footer", &[]),
    ("kit-bubble", &["alignment", "variant"]),
    ("kit-bubble-group", &[]),
    ("kit-marker", &["variant", "loading"]),
    ("kit-attachment-group", &[]),
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
        use gpui_component::{
            bubble::{Bubble, BubbleContent, BubbleGroup, BubbleVariant},
            form::{Field, Form},
            marker::{Marker, MarkerContent, MarkerVariant},
            message::{
                Message, MessageAlignment, MessageContent, MessageFooter, MessageGroup,
                MessageHeader,
            },
            sidebar::{
                Sidebar, SidebarFooter, SidebarHeader, SidebarMenu, SidebarMenuItem,
                SidebarToggleButton,
            },
            stepper::{Stepper, StepperItem},
        };
        let id = ctx.id;
        let native_id = custom_element_id("gpui-kit", id);
        let callback = ctx.event_callback.clone();
        let children = std::mem::take(&mut ctx.children);
        let mut children = children.into_iter();
        let items = ctx
            .props
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let alignment = match string(&ctx, "alignment").as_str() {
            "end" => MessageAlignment::End,
            _ => MessageAlignment::Start,
        };
        let side = if string(&ctx, "side") == "right" {
            gpui_component::Side::Right
        } else {
            gpui_component::Side::Left
        };
        let component = match self.name {
            "kit-form" => {
                let form = if boolean(&ctx, "vertical") {
                    Form::vertical()
                } else {
                    Form::horizontal()
                };
                form.columns(integer(&ctx, "columns", 1))
                    .with_size(size(&ctx))
                    .children(items.iter().enumerate().map(|(i, item)| {
                        let label = kit_text(&ctx, 2 * i, text(item, "label"));
                        let description = kit_text(&ctx, 2 * i + 1, text(item, "description"));
                        Field::new()
                            .label_fn(move |_, _| label.clone())
                            .description_fn(move |_, _| description.clone())
                            .required(
                                item.get("required")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            )
                            .children(children.next())
                    }))
                    .into_any_element()
            }
            "kit-field" => {
                let label = kit_text(&ctx, 0, string(&ctx, "label"));
                let description = kit_text(&ctx, 1, string(&ctx, "description"));
                Field::new()
                    .label_fn(move |_, _| label.clone())
                    .description_fn(move |_, _| description.clone())
                    .required(boolean(&ctx, "required"))
                    .children(children)
                    .into_any_element()
            }
            "kit-stepper" => Stepper::new(native_id)
                .selected_index(integer(&ctx, "selectedIndex", 0))
                .layout(if boolean(&ctx, "vertical") {
                    gpui::Axis::Vertical
                } else {
                    gpui::Axis::Horizontal
                })
                .text_center(boolean(&ctx, "textCenter"))
                .disabled(boolean(&ctx, "disabled"))
                .with_size(size(&ctx))
                .items(items.iter().enumerate().map(|(i, item)| {
                    StepperItem::new()
                        .disabled(
                            item.get("disabled")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                        .child(ctx.text(i, text(item, "label"), None))
                }))
                .on_click(move |i, _, _| change(&callback, id, *i))
                .into_any_element(),
            "kit-sidebar" => {
                let menu =
                    SidebarMenu::new().children(items.iter().enumerate().map(|(i, item)| {
                        let callback = callback.clone();
                        SidebarMenuItem::new(text(item, "label"))
                            .label_content(kit_text(&ctx, i, text(item, "label")))
                            .active(i == integer(&ctx, "selectedIndex", 0))
                            .disable(
                                item.get("disabled")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            )
                            .on_click(move |_, _, _| change(&callback, id, i))
                    }));
                Sidebar::new(native_id)
                    .side(side)
                    .collapsed(boolean(&ctx, "collapsed"))
                    .header(gpui::div().children(children.next()))
                    .child(menu)
                    .footer(gpui::div().children(children))
                    .into_any_element()
            }
            "kit-sidebar-header" => SidebarHeader::new().children(children).into_any_element(),
            "kit-sidebar-footer" => SidebarFooter::new().children(children).into_any_element(),
            "kit-sidebar-toggle" => {
                let collapsed = boolean(&ctx, "collapsed");
                SidebarToggleButton::new()
                    .side(side)
                    .collapsed(collapsed)
                    .on_click(move |_, _, _| change(&callback, id, !collapsed))
                    .into_any_element()
            }
            "kit-status-bar" => gpui_component::status_bar::StatusBar::new()
                .left(gpui::div().children(children.next()))
                .right(gpui::div().children(children))
                .into_any_element(),
            "kit-clipboard" => gpui_component::clipboard::Clipboard::new(native_id)
                .value(string(&ctx, "value"))
                .tooltip(string(&ctx, "tooltip"))
                .on_copied(move |_, _, _| change(&callback, id, true))
                .into_any_element(),
            "kit-message" => Message::new()
                .alignment(alignment)
                .content(MessageContent::new().children(children))
                .into_any_element(),
            "kit-message-group" => MessageGroup::new().children(children).into_any_element(),
            "kit-message-header" => MessageHeader::new().children(children).into_any_element(),
            "kit-message-content" => MessageContent::new().children(children).into_any_element(),
            "kit-message-footer" => MessageFooter::new().children(children).into_any_element(),
            "kit-bubble" => Bubble::new()
                .alignment(alignment)
                .with_variant(match string(&ctx, "variant").as_str() {
                    "outline" => BubbleVariant::Outline,
                    "ghost" => BubbleVariant::Ghost,
                    _ => BubbleVariant::Filled,
                })
                .content(BubbleContent::new().children(children))
                .into_any_element(),
            "kit-bubble-group" => BubbleGroup::new().children(children).into_any_element(),
            "kit-marker" => Marker::new()
                .id(native_id)
                .with_variant(match string(&ctx, "variant").as_str() {
                    "separator" => MarkerVariant::Separator,
                    _ => MarkerVariant::Plain,
                })
                .loading(boolean(&ctx, "loading"))
                .content(MarkerContent::new().children(children))
                .into_any_element(),
            "kit-attachment-group" => gpui_component::attachment::AttachmentGroup::new(native_id)
                .children(children)
                .into_any_element(),
            _ => unreachable!("registered Kit composition primitive"),
        };
        surface(&ctx).child(component).into_any_element()
    }
}
