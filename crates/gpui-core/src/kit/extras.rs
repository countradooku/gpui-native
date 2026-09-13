//! Informational surfaces, hover content, notifications, and attachment slots.
use super::{
    content::Content,
    elements::{boolean, change, integer, kit_text, size, string, surface},
};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, Context, Entity, Window, prelude::*};
use gpui_component::{
    Sizable as _, WindowExt as _,
    attachment::{
        Attachment, AttachmentActions, AttachmentContent, AttachmentMedia, AttachmentStatus,
    },
    hover_card::HoverCard,
    label::Label,
    notification::{Notification, NotificationDelivery, NotificationType},
    shimmer::ShimmerText,
    tooltip::Tooltip,
};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    ("kit-label", &["label", "masked"]),
    (
        "kit-shimmer-text",
        &["label", "duration", "reverse", "once"],
    ),
    ("kit-tooltip", &["label"]),
    (
        "kit-hover-card",
        &["label", "openDelay", "closeDelay", "appearance"],
    ),
    (
        "kit-notification",
        &["open", "title", "message", "variant", "autohide"],
    ),
    (
        "kit-attachment",
        &["title", "description", "src", "status", "vertical", "size"],
    ),
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
            content: None,
            owner: None,
            was_open: false,
            revision: None,
            texts: Rc::new(RefCell::new(Vec::new())),
        })
    }
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    content: Option<Entity<Content>>,
    owner: Option<Rc<()>>,
    was_open: bool,
    revision: Option<u64>,
    texts: Rc<RefCell<Vec<gpui_component::text::Text>>>,
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
    fn destroy(&mut self) {
        self.content = None;
        self.owner = None;
        self.was_open = false;
        self.texts.borrow_mut().clear();
    }
    #[allow(
        clippy::too_many_lines,
        reason = "native informational surfaces share the same retained lifecycle"
    )]
    fn render(
        &mut self,
        mut ctx: CustomRenderContext,
        window: &mut Window,
        cx: &mut Context<GpuiView>,
    ) -> AnyElement {
        let id = ctx.id;
        let callback = ctx.event_callback.clone();
        let native_id = custom_element_id("gpui-kit", id);
        if let Some(render) = ctx.render_children.clone() {
            let content = self.content.get_or_insert_with(|| {
                cx.new(|_| Content {
                    render: render.clone(),
                })
            });
            content.update(cx, |content, cx| {
                content.render = render;
                if self.revision != Some(ctx.revision) {
                    cx.notify();
                }
            });
        }
        self.revision = Some(ctx.revision);
        let children = std::mem::take(&mut ctx.children);
        let component = match self.name {
            "kit-label" => {
                let text = string(&ctx, "label");
                let text = if boolean(&ctx, "masked") {
                    "•".repeat(text.chars().count())
                } else {
                    text
                };
                Label::new(text.clone())
                    .content(kit_text(&ctx, 0, text))
                    .into_any_element()
            }
            "kit-shimmer-text" => {
                let text = string(&ctx, "label");
                let opts = crate::text::SelectableText {
                    selectable: ctx.selectable,
                    highlight: ctx
                        .highlight_set
                        .clone()
                        .map(crate::text::HighlightSource::Native),
                    ..crate::text::SelectableText::new(
                        id,
                        0,
                        text.clone().into(),
                        None,
                        ctx.selection.clone(),
                        ctx.selection_wash,
                    )
                };
                ShimmerText::new(text)
                    .id(native_id)
                    .duration(std::time::Duration::from_millis(
                        u64::try_from(integer(&ctx, "duration", 2000)).unwrap_or(2000),
                    ))
                    .reverse(boolean(&ctx, "reverse"))
                    .once(boolean(&ctx, "once"))
                    .observe_paint(Rc::new(move |layout, window| {
                        crate::text::paint::observe_flow_text(&opts, layout, window);
                    }))
                    .into_any_element()
            }
            "kit-tooltip" => {
                let text = kit_text(&ctx, 0, string(&ctx, "label"));
                return surface(&ctx)
                    .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
                    .children(children)
                    .into_any_element();
            }
            "kit-hover-card" => {
                let Some(content) = self.content.clone() else {
                    return surface(&ctx).into_any_element();
                };
                HoverCard::new(native_id)
                    .trigger(kit_text(&ctx, 0, string(&ctx, "label")))
                    .content(move |_, _, _| content.clone())
                    .open_delay(std::time::Duration::from_millis(
                        u64::try_from(integer(&ctx, "openDelay", 600)).unwrap_or(600),
                    ))
                    .close_delay(std::time::Duration::from_millis(
                        u64::try_from(integer(&ctx, "closeDelay", 300)).unwrap_or(300),
                    ))
                    .appearance(
                        ctx.props
                            .get("appearance")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    )
                    .on_open_change(move |open, _, _| change(&callback, id, *open))
                    .into_any_element()
            }
            "kit-notification" => {
                *self.texts.borrow_mut() = vec![
                    kit_text(&ctx, 0, string(&ctx, "title")),
                    kit_text(&ctx, 1, string(&ctx, "message")),
                ];
                let open = boolean(&ctx, "open");
                if open && !self.was_open {
                    let owner = Rc::new(());
                    let alive = Rc::downgrade(&owner);
                    let texts = self.texts.clone();
                    let content = self.content.clone();
                    let note = Notification::new()
                        .id1::<Control>(native_id)
                        .owned_by(alive.clone())
                        .delivery(NotificationDelivery::InApp)
                        .autohide(
                            ctx.props
                                .get("autohide")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                        )
                        .with_type(match string(&ctx, "variant").as_str() {
                            "success" => NotificationType::Success,
                            "warning" => NotificationType::Warning,
                            "error" => NotificationType::Error,
                            _ => NotificationType::Info,
                        })
                        .content(move |_, _, _| {
                            if alive.upgrade().is_none() {
                                return gpui::Empty.into_any_element();
                            }
                            gpui::div()
                                .flex()
                                .flex_col()
                                .children(texts.borrow().clone())
                                .children(content.clone())
                                .into_any_element()
                        })
                        .on_close(move |_, _| change(&callback, id, false));
                    window.push_notification(note, cx);
                    self.owner = Some(owner);
                } else if !open {
                    self.owner = None;
                }
                self.was_open = open;
                return gpui::Empty.into_any_element();
            }
            "kit-attachment" => {
                let mut attachment = Attachment::new()
                    .id(native_id)
                    .with_size(size(&ctx))
                    .axis(if boolean(&ctx, "vertical") {
                        gpui::Axis::Vertical
                    } else {
                        gpui::Axis::Horizontal
                    })
                    .status(match string(&ctx, "status").as_str() {
                        "pending" => AttachmentStatus::Pending,
                        "uploading" => AttachmentStatus::Uploading,
                        "processing" => AttachmentStatus::Processing,
                        "failed" => AttachmentStatus::Failed,
                        _ => AttachmentStatus::Complete,
                    })
                    .content(
                        AttachmentContent::new()
                            .child(kit_text(&ctx, 0, string(&ctx, "title")))
                            .child(kit_text(&ctx, 1, string(&ctx, "description"))),
                    )
                    .actions(AttachmentActions::new().children(children))
                    .on_click(move |_, _, _| change(&callback, id, true));
                let src = string(&ctx, "src");
                if !src.is_empty() {
                    attachment = attachment.media(AttachmentMedia::new().src(src));
                }
                attachment.into_any_element()
            }
            _ => unreachable!("registered informational surfaces"),
        };
        surface(&ctx).child(component).into_any_element()
    }
}
