//! Searchable single and multiple selection using Kit's native list delegates.
use super::elements::{boolean, change, size, string};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, App, Context, Entity, SharedString, Subscription, Window, prelude::*};
use gpui_component::{
    Sizable as _,
    combobox::{Combobox, ComboboxEvent, ComboboxState},
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-select",
        &[
            "items",
            "value",
            "searchable",
            "placeholder",
            "searchPlaceholder",
            "disabled",
            "cleanable",
            "appearance",
            "size",
        ],
    ),
    (
        "kit-combobox",
        &[
            "items",
            "value",
            "multiple",
            "searchable",
            "placeholder",
            "searchPlaceholder",
            "disabled",
            "cleanable",
            "appearance",
            "size",
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
            subscription: None,
            dirty: HashSet::new(),
            texts: Rc::new(RefCell::new(Vec::new())),
        })
    }
}
#[derive(Clone)]
struct Item {
    label: SharedString,
    value: String,
    disabled: bool,
    index: usize,
    texts: Rc<RefCell<Vec<gpui_component::text::Text>>>,
}
impl SearchableListItem for Item {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &String {
        &self.value
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
    fn display_title(&self) -> Option<AnyElement> {
        self.texts
            .borrow()
            .get(self.index)
            .map(|text| text.clone().into_any_element())
    }
    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.display_title()
            .unwrap_or_else(|| gpui::Empty.into_any_element())
    }
}
type Items = SearchableVec<Item>;
enum State {
    Select(Entity<SelectState<Items>>),
    Combobox(Entity<ComboboxState<Items>>),
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    state: Option<State>,
    subscription: Option<Subscription>,
    dirty: HashSet<String>,
    texts: Rc<RefCell<Vec<gpui_component::text::Text>>>,
}
impl CustomElement for Control {
    fn native_focus_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        use gpui::Focusable as _;
        Some(match self.state.as_ref()? {
            State::Select(state) => state.focus_handle(cx),
            State::Combobox(state) => state.focus_handle(cx),
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
        self.subscription = None;
        self.state = None;
        self.texts.borrow_mut().clear();
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
        let callback = ctx.event_callback.clone();
        let values = ctx
            .props
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut texts = Vec::with_capacity(values.len());
        let items = Items::new(
            values
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let label = item
                        .as_str()
                        .or_else(|| item.get("label").and_then(Value::as_str))
                        .unwrap_or_default()
                        .to_owned();
                    let value = item
                        .get("value")
                        .and_then(Value::as_str)
                        .unwrap_or(&label)
                        .to_owned();
                    texts.push(super::elements::kit_text(&ctx, index, label.clone()));
                    Item {
                        label: label.into(),
                        value,
                        disabled: item
                            .get("disabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        index,
                        texts: self.texts.clone(),
                    }
                })
                .collect::<Vec<_>>(),
        );
        *self.texts.borrow_mut() = texts;
        if self.dirty.contains("searchable") || self.dirty.contains("multiple") {
            self.subscription = None;
            self.state = None;
        }
        let created = self.state.is_none();
        if created {
            self.state = Some(if self.name == "kit-select" {
                let state = cx.new(|cx| {
                    SelectState::new(items.clone(), None, window, cx)
                        .searchable(boolean(&ctx, "searchable"))
                });
                self.subscription = Some(cx.subscribe(&state, move |_, _, event, cx| {
                    let SelectEvent::Confirm(value) = event;
                    change(&callback, id, json!(value));
                    cx.notify();
                }));
                State::Select(state)
            } else {
                let multiple = boolean(&ctx, "multiple");
                let state = cx.new(|cx| {
                    ComboboxState::new(items.clone(), vec![], window, cx)
                        .multiple(multiple)
                        .searchable(
                            ctx.props
                                .get("searchable")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                        )
                });
                self.subscription = Some(cx.subscribe(&state, move |_, _, event, cx| {
                    if let ComboboxEvent::Change(values) = event {
                        change(
                            &callback,
                            id,
                            if multiple {
                                json!(values)
                            } else {
                                json!(values.first())
                            },
                        );
                    }
                    cx.notify();
                }));
                State::Combobox(state)
            });
        }
        let sync = created || self.dirty.contains("value") || self.dirty.contains("items");
        let component = match self.state.as_ref() {
            Some(State::Select(state)) => {
                state.update(cx, |state, cx| {
                    if self.dirty.contains("items") {
                        state.set_items(items, window, cx);
                    }
                    if sync {
                        if let Some(value) = ctx.props.get("value").and_then(Value::as_str) {
                            state.set_selected_value(&value.to_owned(), window, cx);
                        } else {
                            state.set_selected_index(None, window, cx);
                        }
                    }
                });
                Select::new(state)
                    .placeholder(string(&ctx, "placeholder"))
                    .search_placeholder(string(&ctx, "searchPlaceholder"))
                    .disabled(boolean(&ctx, "disabled"))
                    .cleanable(boolean(&ctx, "cleanable"))
                    .appearance(
                        ctx.props
                            .get("appearance")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    )
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            Some(State::Combobox(state)) => {
                state.update(cx, |state, cx| {
                    if self.dirty.contains("items") {
                        state.set_items(items, window, cx);
                    }
                    if sync {
                        let values = ctx.props.get("value").map_or_else(Vec::new, |v| match v {
                            Value::String(v) => vec![v.clone()],
                            Value::Array(v) => v
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect(),
                            _ => vec![],
                        });
                        state.set_selected_values(&values, window, cx);
                    }
                });
                Combobox::new(state)
                    .placeholder(string(&ctx, "placeholder"))
                    .search_placeholder(string(&ctx, "searchPlaceholder"))
                    .disabled(boolean(&ctx, "disabled"))
                    .cleanable(boolean(&ctx, "cleanable"))
                    .appearance(
                        ctx.props
                            .get("appearance")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    )
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            None => unreachable!("created choice state"),
        };
        self.dirty.clear();
        super::elements::surface(&ctx)
            .child(component)
            .into_any_element()
    }
}
