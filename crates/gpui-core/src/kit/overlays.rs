//! Overlay lifetimes follow their retained hosts, including removal and type reuse.
use super::{
    content::Content,
    elements::{boolean, change, kit_text, size, string},
};
use crate::custom_elements::{
    CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    custom_element_id,
};
use crate::renderer::GpuiView;
use gpui::{AnyElement, Context, Entity, Window, prelude::*};
use gpui_component::{Disableable as _, Root, Sizable as _};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-popover",
        &[
            "label",
            "open",
            "defaultOpen",
            "overlayClosable",
            "appearance",
            "anchor",
            "disabled",
            "size",
        ],
    ),
    (
        "kit-dialog",
        &[
            "open",
            "title",
            "dialogWidth",
            "overlay",
            "overlayClosable",
            "closeButton",
            "keyboard",
        ],
    ),
    (
        "kit-sheet",
        &[
            "open",
            "title",
            "placement",
            "sheetSize",
            "overlay",
            "overlayClosable",
            "resizable",
        ],
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
        Box::new(Overlay {
            name: self.name,
            props: self.props,
            content: None,
            revision: None,
            owner: None,
            was_open: false,
            config: Rc::new(RefCell::new(HashMap::new())),
            title: Rc::new(RefCell::new(gpui_component::text::Text::from(""))),
        })
    }
}
struct Overlay {
    name: &'static str,
    props: &'static [&'static str],
    content: Option<Entity<Content>>,
    revision: Option<u64>,
    owner: Option<Rc<()>>,
    was_open: bool,
    config: Rc<RefCell<HashMap<String, Value>>>,
    title: Rc<RefCell<gpui_component::text::Text>>,
}
fn flag(props: &HashMap<String, Value>, key: &str) -> bool {
    props.get(key).and_then(Value::as_bool).unwrap_or(true)
}
#[allow(
    clippy::cast_possible_truncation,
    reason = "validated GPUI pixel value"
)]
fn pixels(props: &HashMap<String, Value>, key: &str, default: f32) -> gpui::Pixels {
    gpui::px(
        props
            .get(key)
            .and_then(Value::as_f64)
            .map_or(default, |v| v as f32),
    )
}
impl CustomElement for Overlay {
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
        self.owner = None;
        self.content = None;
        self.revision = None;
        self.was_open = false;
    }
    #[allow(
        clippy::too_many_lines,
        reason = "each native builder stays beside its prop and event mapping"
    )]
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        window: &mut Window,
        cx: &mut Context<GpuiView>,
    ) -> AnyElement {
        let id = ctx.id;
        let Some(render) = ctx.render_children.clone() else {
            return gpui::Empty.into_any_element();
        };
        let content = self
            .content
            .get_or_insert_with(|| {
                cx.new(|_| Content {
                    render: render.clone(),
                })
            })
            .clone();
        if self.revision != Some(ctx.revision) {
            content.update(cx, |content, cx| {
                content.render = render;
                cx.notify();
            });
            self.revision = Some(ctx.revision);
        }
        if self.name == "kit-sheet"
            && self.config.borrow().get("placement") != ctx.props.get("placement")
        {
            self.was_open = false;
        }
        self.config.borrow_mut().clone_from(ctx.props);
        *self.title.borrow_mut() = kit_text(&ctx, 0, string(&ctx, "title"));
        let callback = ctx.event_callback.clone();
        if self.name == "kit-popover" {
            let mut popover =
                gpui_component::popover::Popover::new(custom_element_id("gpui-kit", id))
                    .trigger(
                        gpui_component::button::Button::new(custom_element_id("kit-trigger", id))
                            .disabled(boolean(&ctx, "disabled"))
                            .with_size(size(&ctx))
                            .child(ctx.text(0, string(&ctx, "label"), None)),
                    )
                    .content(move |_, _, _| content.clone())
                    .default_open(boolean(&ctx, "defaultOpen"))
                    .overlay_closable(flag(ctx.props, "overlayClosable"))
                    .appearance(flag(ctx.props, "appearance"))
                    .anchor(match string(&ctx, "anchor").as_str() {
                        "top-right" => gpui::Anchor::TopRight,
                        "bottom-left" => gpui::Anchor::BottomLeft,
                        "bottom-right" => gpui::Anchor::BottomRight,
                        "top-center" => gpui::Anchor::TopCenter,
                        "bottom-center" => gpui::Anchor::BottomCenter,
                        _ => gpui::Anchor::TopLeft,
                    })
                    .on_open_change(move |open, _, _| change(&callback, id, *open));
            if let Some(open) = ctx.props.get("open").and_then(Value::as_bool) {
                popover = popover.open(open);
            }
            return super::elements::surface(&ctx)
                .child(popover)
                .into_any_element();
        }
        let open = boolean(&ctx, "open");
        if open && !self.was_open {
            let owner = Rc::new(());
            let weak = Rc::downgrade(&owner);
            self.owner = Some(owner);
            let config = self.config.clone();
            let title = self.title.clone();
            if self.name == "kit-dialog" {
                Root::update(window, cx, |root, window, cx| {
                    root.open_dialog(
                        move |dialog, _, _| {
                            let config = config.borrow();
                            let content = content.clone();
                            let callback = callback.clone();
                            dialog
                                .title(title.borrow().clone())
                                .width(pixels(&config, "dialogWidth", 480.))
                                .overlay(flag(&config, "overlay"))
                                .overlay_closable(flag(&config, "overlayClosable"))
                                .close_button(flag(&config, "closeButton"))
                                .keyboard(flag(&config, "keyboard"))
                                .content(move |body, _, _| body.child(content.clone()))
                                .on_close(move |_, _, _| change(&callback, id, false))
                        },
                        window,
                        cx,
                    );
                    root.own_last_dialog(weak);
                });
            } else {
                let placement = match string(&ctx, "placement").as_str() {
                    "left" => gpui_component::Placement::Left,
                    "top" => gpui_component::Placement::Top,
                    "bottom" => gpui_component::Placement::Bottom,
                    _ => gpui_component::Placement::Right,
                };
                Root::update(window, cx, |root, window, cx| {
                    root.open_sheet_at(
                        placement,
                        move |sheet, _, _| {
                            let config = config.borrow();
                            let callback = callback.clone();
                            sheet
                                .title(title.borrow().clone())
                                .size(pixels(&config, "sheetSize", 350.))
                                .overlay(flag(&config, "overlay"))
                                .overlay_closable(flag(&config, "overlayClosable"))
                                .resizable(flag(&config, "resizable"))
                                .child(content.clone())
                                .on_close(move |_, _, _| change(&callback, id, false))
                        },
                        window,
                        cx,
                    );
                    root.own_sheet(weak);
                });
            }
        } else if !open {
            self.owner = None;
        }
        self.was_open = open;
        gpui::Empty.into_any_element()
    }
}
