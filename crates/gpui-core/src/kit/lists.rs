//! Kit command palettes and virtualized lists with framework-owned data.
use super::elements::{boolean, change, kit_text, size, string, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, App, Context, Entity, Subscription, Task, Window, prelude::*};
use gpui_component::{
    Disableable as _, IndexPath, Sizable as _,
    command::{Command, CommandItem, CommandState},
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashSet, rc::Rc};
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-command",
        &[
            "items",
            "query",
            "searchable",
            "filterable",
            "placeholder",
            "loading",
            "bordered",
        ],
    ),
    (
        "kit-list",
        &[
            "items",
            "query",
            "selectedIndex",
            "searchable",
            "placeholder",
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
            texts: Rc::new(RefCell::new(vec![])),
        })
    }
}
struct Data {
    query: String,
    labels: Vec<String>,
    matched: Vec<usize>,
    texts: Rc<RefCell<Vec<gpui_component::text::Text>>>,
}
impl ListDelegate for Data {
    type Item = ListItem;
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.matched.len()
    }
    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let index = *self.matched.get(index.row)?;
        Some(ListItem::new(("list-item", index)).child(self.texts.borrow().get(index)?.clone()))
    }
    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }
    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let query = query.to_lowercase();
        self.query.clone_from(&query);
        self.matched = self
            .labels
            .iter()
            .enumerate()
            .filter_map(|(index, label)| label.to_lowercase().contains(&query).then_some(index))
            .collect();
        cx.notify();
        Task::ready(())
    }
}
enum State {
    Command(Entity<CommandState>),
    List(Entity<ListState<Data>>),
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
            State::Command(state) => state.focus_handle(cx),
            State::List(state) => state.focus_handle(cx),
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
        reason = "keep related native prop and event mappings adjacent"
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
        let labels = values
            .iter()
            .map(|v| {
                v.as_str()
                    .or_else(|| v.get("label").and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        *self.texts.borrow_mut() = labels
            .iter()
            .enumerate()
            .map(|(i, label)| kit_text(&ctx, i, label.clone()))
            .collect();
        let created = self.state.is_none();
        if created {
            self.state = Some(if self.name == "kit-command" {
                State::Command(cx.new(|cx| CommandState::new(window, cx)))
            } else {
                let state = cx.new(|cx| {
                    ListState::new(
                        Data {
                            query: String::new(),
                            matched: (0..labels.len()).collect(),
                            labels: labels.clone(),
                            texts: self.texts.clone(),
                        },
                        window,
                        cx,
                    )
                });
                let callback = callback.clone();
                self.subscription=Some(cx.subscribe(&state,move |_,state,event,cx|{
                    let value=match event{ListEvent::Select(index)|ListEvent::Confirm(index)=>json!({"type":if matches!(event,ListEvent::Select(_)){"select"}else{"confirm"},"index":state.read(cx).delegate().matched.get(index.row)}),ListEvent::Cancel=>json!({"type":"cancel"})};change(&callback,id,value);cx.notify();
                }));
                State::List(state)
            });
        }
        let component =
            match self.state.as_ref() {
                Some(State::Command(state)) => {
                    state.update(cx, |state, cx| {
                        if created || self.dirty.contains("query") {
                            state.set_query(string(&ctx, "query"), window, cx);
                        }
                        if created || self.dirty.contains("loading") {
                            state.set_loading(boolean(&ctx, "loading"), window, cx);
                        }
                    });
                    let query = callback.clone();
                    let select = callback.clone();
                    let confirm = callback.clone();
                    Command::new(state)
                        .items(values.iter().zip(labels).enumerate().map(
                            |(index, (value, label))| {
                                let text = self.texts.borrow()[index].clone();
                                CommandItem::new()
                                    .label(label)
                                    .disabled(
                                        value
                                            .get("disabled")
                                            .and_then(Value::as_bool)
                                            .unwrap_or(false),
                                    )
                                    .checked(
                                        value
                                            .get("checked")
                                            .and_then(Value::as_bool)
                                            .unwrap_or(false),
                                    )
                                    .child(move |_, _| text.clone())
                            },
                        ))
                        .searchable(
                            ctx.props
                                .get("searchable")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                        )
                        .filterable(
                            ctx.props
                                .get("filterable")
                                .and_then(Value::as_bool)
                                .unwrap_or(true),
                        )
                        .placeholder(string(&ctx, "placeholder"))
                        .bordered(boolean(&ctx, "bordered"))
                        .on_query(move |value, _, _| {
                            change(&query, id, json!({"type":"query","value":value}));
                        })
                        .on_select(move |index, _, _| {
                            change(&select, id, json!({"type":"select","index":index.row}));
                        })
                        .on_confirm(move |index, _, _| {
                            change(&confirm, id, json!({"type":"confirm","index":index.row}));
                        })
                        .on_cancel(move |_, _| change(&callback, id, json!({"type":"cancel"})))
                        .into_any_element()
                }
                Some(State::List(state)) => {
                    state.update(cx, |state, cx| {
                        if self.dirty.contains("items") {
                            let data = state.delegate_mut();
                            data.labels = labels;
                            data.matched = data
                                .labels
                                .iter()
                                .enumerate()
                                .filter_map(|(i, label)| {
                                    label.to_lowercase().contains(&data.query).then_some(i)
                                })
                                .collect();
                            cx.notify();
                        }
                        if created || self.dirty.contains("searchable") {
                            state.set_searchable(boolean(&ctx, "searchable"), cx);
                        }
                        if created || self.dirty.contains("query") {
                            state.set_query(&string(&ctx, "query"), window, cx);
                        }
                        if created || self.dirty.contains("selectedIndex") {
                            state.set_selected_index(
                                ctx.props
                                    .get("selectedIndex")
                                    .and_then(Value::as_u64)
                                    .and_then(|v| usize::try_from(v).ok())
                                    .and_then(|row| {
                                        state.delegate().matched.iter().position(|i| *i == row)
                                    })
                                    .map(|row| IndexPath::default().row(row)),
                                window,
                                cx,
                            );
                        }
                    });
                    List::new(state)
                        .search_placeholder(string(&ctx, "placeholder"))
                        .with_size(size(&ctx))
                        .into_any_element()
                }
                None => unreachable!("initialized list state"),
            };
        self.dirty.clear();
        surface(&ctx).child(component).into_any_element()
    }
}
