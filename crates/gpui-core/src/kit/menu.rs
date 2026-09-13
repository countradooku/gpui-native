//! Native dropdown and context menus keep keyboard navigation and dismissal in Kit.
use super::elements::{boolean, change, kit_text, size, string, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, Context, Window, prelude::*};
use gpui_component::{
    Disableable as _, Sizable as _,
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem},
};
use serde_json::Value;
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    ("kit-dropdown-menu", &["items", "label", "disabled", "size"]),
    ("kit-context-menu", &["items"]),
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
    fn render(
        &mut self,
        mut ctx: CustomRenderContext,
        _: &mut Window,
        _: &mut Context<GpuiView>,
    ) -> AnyElement {
        let id = ctx.id;
        let callback = ctx.event_callback.clone();
        let items = ctx
            .props
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let items = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let label = item
                    .as_str()
                    .or_else(|| item.get("label").and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_owned();
                (index, item.clone(), kit_text(&ctx, index, label))
            })
            .collect::<Vec<_>>();
        let builder = move |mut menu: PopupMenu, _: &mut Window, _: &mut Context<PopupMenu>| {
            for (index, item, text) in &items {
                if item.get("separator").and_then(Value::as_bool) == Some(true) {
                    menu = menu.separator();
                    continue;
                }
                let text = text.clone();
                let callback = callback.clone();
                let index = *index;
                menu = menu.item(
                    PopupMenuItem::element(move |_, _| text.clone())
                        .disabled(
                            item.get("disabled")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                        .checked(
                            item.get("checked")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                        .on_click(move |_, _, _| change(&callback, id, index)),
                );
            }
            menu
        };
        let children = std::mem::take(&mut ctx.children);
        if self.name == "kit-context-menu" {
            surface(&ctx)
                .children(children)
                .context_menu(builder)
                .into_any_element()
        } else {
            surface(&ctx)
                .child(
                    gpui_component::button::Button::new(custom_element_id("gpui-kit", id))
                        .disabled(boolean(&ctx, "disabled"))
                        .with_size(size(&ctx))
                        .child(ctx.text(items_len(&ctx), string(&ctx, "label"), None))
                        .children(children)
                        .dropdown_menu(builder),
                )
                .into_any_element()
        }
    }
}
fn items_len(ctx: &CustomRenderContext<'_>) -> usize {
    ctx.props
        .get("items")
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}
