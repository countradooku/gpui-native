//! Entity-backed navigation collections with stable tree IDs and carousel state.
use super::elements::{boolean, change, integer, kit_text, string, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, Context, Entity, Subscription, Window, prelude::*};
use gpui_component::{
    carousel::{
        Carousel, CarouselContent, CarouselEvent, CarouselItem, CarouselNext, CarouselPagination,
        CarouselPaginationItem, CarouselPrevious, CarouselState,
    },
    tree::{Tree, TreeEvent, TreeItem, TreeState},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    ("kit-tree", &["items", "selectedId"]),
    (
        "kit-carousel",
        &[
            "selectedIndex",
            "vertical",
            "looping",
            "controls",
            "pagination",
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
        Box::new(Control {
            name: self.name,
            props: self.props,
            state: None,
            subscriptions: vec![],
            dirty: HashSet::new(),
            texts: Rc::new(RefCell::new(HashMap::new())),
            selected: Rc::new(RefCell::new(None)),
        })
    }
}
enum State {
    Tree(Entity<TreeState>),
    Carousel(Entity<CarouselState>),
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    state: Option<State>,
    subscriptions: Vec<Subscription>,
    dirty: HashSet<String>,
    texts: Rc<RefCell<HashMap<String, gpui_component::text::Text>>>,
    selected: Rc<RefCell<Option<String>>>,
}
fn items(
    values: &[Value],
    ctx: &CustomRenderContext<'_>,
    texts: &mut HashMap<String, gpui_component::text::Text>,
) -> Vec<TreeItem> {
    values
        .iter()
        .map(|value| {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let label = value
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or(&id)
                .to_owned();
            texts.insert(id.clone(), kit_text(ctx, texts.len(), label.clone()));
            TreeItem::new(id, label)
                .expanded(
                    value
                        .get("expanded")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .disabled(
                    value
                        .get("disabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                )
                .children(items(
                    value
                        .get("children")
                        .and_then(Value::as_array)
                        .map_or(&[], Vec::as_slice),
                    ctx,
                    texts,
                ))
        })
        .collect()
}
impl CustomElement for Control {
    fn native_focus_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        use gpui::Focusable as _;
        Some(match self.state.as_ref()? {
            State::Tree(_) => return None,
            State::Carousel(state) => state.focus_handle(cx),
        })
    }
    fn set_prop(&mut self, key: &str, _: Value) {
        self.dirty.insert(key.into());
    }
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
        self.subscriptions.clear();
        self.state = None;
        self.texts.borrow_mut().clear();
    }
    #[allow(
        clippy::too_many_lines,
        reason = "each native builder stays beside its prop and event mapping"
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
        let count = ctx.children.len();
        let created = self.state.is_none();
        if created {
            self.state = Some(if self.name == "kit-tree" {
                let state = cx.new(|cx| TreeState::new(cx));
                let changes = callback.clone();
                self.subscriptions
                    .push(cx.subscribe(&state, move |_, _, event, cx| {
                        let (kind, item) = match event {
                            TreeEvent::Expanded(id) => ("expanded", id),
                            TreeEvent::Collapsed(id) => ("collapsed", id),
                        };
                        change(&changes, id, json!({"type":kind,"id":item.as_ref()}));
                        cx.notify();
                    }));
                let selected = self.selected.clone();
                self.subscriptions
                    .push(cx.observe(&state, move |_, state, cx| {
                        let value = state
                            .read(cx)
                            .selected_item()
                            .map(|item| item.id.to_string());
                        if *selected.borrow() != value {
                            selected.borrow_mut().clone_from(&value);
                            change(&callback, id, json!({"type":"selected","id":value}));
                        }
                        cx.notify();
                    }));
                State::Tree(state)
            } else {
                let state = cx.new(|_| CarouselState::new(count));
                self.subscriptions
                    .push(cx.subscribe(&state, move |_, _, event, cx| {
                        let CarouselEvent::Change(index) = event;
                        change(&callback, id, *index);
                        cx.notify();
                    }));
                State::Carousel(state)
            });
        }
        let component = match self.state.as_ref() {
            Some(State::Tree(state)) => {
                let mut texts = HashMap::new();
                let values = items(
                    ctx.props
                        .get("items")
                        .and_then(Value::as_array)
                        .map_or(&[], Vec::as_slice),
                    &ctx,
                    &mut texts,
                );
                *self.texts.borrow_mut() = texts;
                state.update(cx, |state, cx| {
                    if created || self.dirty.contains("items") {
                        state.set_items(values, cx);
                    }
                    if created || self.dirty.contains("items") || self.dirty.contains("selectedId")
                    {
                        let selected = string(&ctx, "selectedId");
                        let index = state.index_of(&selected.clone().into());
                        state.set_selected_index(index, cx);
                        *self.selected.borrow_mut() =
                            state.selected_item().map(|item| item.id.to_string());
                    }
                });
                let texts = self.texts.clone();
                Tree::new(state, move |index, entry, _, _, _| {
                    let mut item = gpui_component::list::ListItem::new(("tree-row", index));
                    if let Some(text) = texts.borrow().get(entry.item().id.as_ref()) {
                        item = item.child(text.clone());
                    }
                    item
                })
                .into_any_element()
            }
            Some(State::Carousel(state)) => {
                state.update(cx, |state, cx| {
                    state.set_item_count(count, cx);
                    state.set_axis(
                        if boolean(&ctx, "vertical") {
                            gpui::Axis::Vertical
                        } else {
                            gpui::Axis::Horizontal
                        },
                        cx,
                    );
                    state.set_looping(boolean(&ctx, "looping"), cx);
                    if created || self.dirty.contains("selectedIndex") {
                        state.set_selected_index(integer(&ctx, "selectedIndex", 0), cx);
                    }
                });
                let children = std::mem::take(&mut ctx.children);
                let mut carousel =
                    Carousel::new(native_id, state).child(CarouselContent::new(state).children(
                        children.into_iter().enumerate().map(|(index, child)| {
                            CarouselItem::new(("slide", index), index, state).child(child)
                        }),
                    ));
                if ctx
                    .props
                    .get("controls")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
                {
                    carousel = carousel
                        .child(CarouselPrevious::new(state))
                        .child(CarouselNext::new(state));
                }
                if boolean(&ctx, "pagination") {
                    carousel =
                        carousel.child(CarouselPagination::new().children((0..count).map(
                            |index| CarouselPaginationItem::new(("page", index), index, state),
                        )));
                }
                carousel.into_any_element()
            }
            None => unreachable!("initialized collection state"),
        };
        self.dirty.clear();
        let _ = window;
        surface(&ctx).child(component).into_any_element()
    }
}
