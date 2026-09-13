//! Native dock layouts retain panel entities while framework children reconcile.
use super::{
    content::ChildRenderer,
    elements::{boolean, change, kit_text, surface},
};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, Render, Subscription,
    Window, prelude::*,
};
use gpui_component::{
    ResizablePanelGroup, ResizableState,
    dock::{
        DockArea, DockAreaState, DockEvent, DockLayout, DockPlacement, DockSkin, PanelEvent,
        PanelInfo, PanelState, panel_handle,
    },
    resizable_panel,
    text::Text,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    ("kit-dock", &["panels", "layout", "locked"]),
    (
        "kit-resizable",
        &["sizes", "minSize", "maxSize", "vertical"],
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
            area: None,
            panels: HashMap::new(),
            resize: None,
            subscription: None,
            dirty: HashSet::new(),
            revision: None,
            last_layout: Rc::new(RefCell::new(Value::Null)),
        })
    }
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    area: Option<Entity<DockArea>>,
    panels: HashMap<String, Entity<Panel>>,
    resize: Option<Entity<ResizableState>>,
    subscription: Option<Subscription>,
    dirty: HashSet<String>,
    revision: Option<u64>,
    last_layout: Rc<RefCell<Value>>,
}
struct Panel {
    key: String,
    title: Text,
    closable: bool,
    index: usize,
    render: ChildRenderer,
    focus: FocusHandle,
}
impl EventEmitter<PanelEvent> for Panel {}
impl Focusable for Panel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::div()
            .size_full()
            .track_focus(&self.focus)
            .child((self.render)(Some(self.index), window, cx))
    }
}
impl gpui_component::dock::BasePanel for Panel {
    fn panel_name(&self) -> &'static str {
        "gpui-native-kit"
    }
    fn closable(&self, _: &App) -> bool {
        self.closable
    }
    fn dump(&self, _: &App) -> PanelState {
        PanelState {
            panel_name: self.panel_name().into(),
            children: vec![],
            info: PanelInfo::panel(json!({"id":self.key})),
        }
    }
}
impl gpui_component::dock::Panel for Panel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
    }
}
fn placement(value: Option<&Value>) -> DockPlacement {
    match value.and_then(Value::as_str) {
        Some("left") => DockPlacement::Left,
        Some("right") => DockPlacement::Right,
        Some("bottom") => DockPlacement::Bottom,
        _ => DockPlacement::Center,
    }
}
fn layout(state: &PanelState, panels: &HashMap<String, Entity<Panel>>, cx: &App) -> DockLayout {
    match &state.info {
        PanelInfo::Stack { sizes, axis } => {
            let mut result = if *axis == 1 {
                DockLayout::v_split()
            } else {
                DockLayout::h_split()
            };
            for (index, child) in state.children.iter().enumerate() {
                result = result.child(layout(child, panels, cx), sizes.get(index).copied());
            }
            result
        }
        PanelInfo::Tabs { active_index } => {
            let mut result = DockLayout::tabs().active_index(*active_index);
            for child in &state.children {
                if let PanelInfo::Panel(value) = &child.info
                    && let Some(panel) = value
                        .get("id")
                        .and_then(Value::as_str)
                        .and_then(|id| panels.get(id))
                {
                    result = result.panel_view(panel_handle(panel.clone()), cx);
                }
            }
            result
        }
        PanelInfo::Panel(value) => value
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| panels.get(id))
            .map_or_else(DockLayout::tabs, |panel| {
                DockLayout::tabs().panel_view(panel_handle(panel.clone()), cx)
            }),
    }
}
impl CustomElement for Control {
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
        self.area = None;
        self.panels.clear();
        self.resize = None;
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
        if self.name == "kit-resizable" {
            if self.dirty.contains("sizes")
                && ctx.props.get("sizes") != Some(&*self.last_layout.borrow())
            {
                self.resize = None;
            }
            let state = self
                .resize
                .get_or_insert_with(|| cx.new(|_| ResizableState::default()))
                .clone();
            let mut group = ResizablePanelGroup::new(custom_element_id("gpui-kit", id))
                .with_state(&state)
                .axis(if boolean(&ctx, "vertical") {
                    gpui::Axis::Vertical
                } else {
                    gpui::Axis::Horizontal
                });
            let children = std::mem::take(&mut ctx.children);
            for (index, child) in children.into_iter().enumerate() {
                let mut panel = resizable_panel().child(child).size_range(
                    gpui::px(super::elements::number(&ctx, "minSize", 0.))
                        ..gpui::px(super::elements::number(&ctx, "maxSize", 10000.)),
                );
                #[allow(clippy::cast_possible_truncation, reason = "validated pixel size")]
                if let Some(size) = ctx
                    .props
                    .get("sizes")
                    .and_then(Value::as_array)
                    .and_then(|v| v.get(index))
                    .and_then(Value::as_f64)
                {
                    panel = panel.size(gpui::px(size as f32));
                }
                group = group.child(panel);
            }
            let last = self.last_layout.clone();
            self.dirty.clear();
            return surface(&ctx)
                .child(group.on_resize(move |state, _, cx| {
                    let value = json!(
                        state
                            .read(cx)
                            .sizes()
                            .iter()
                            .map(|v| f32::from(*v))
                            .collect::<Vec<_>>()
                    );
                    last.borrow_mut().clone_from(&value);
                    change(&callback, id, value);
                }))
                .into_any_element();
        }
        let Some(render) = ctx.render_children.clone() else {
            return gpui::Empty.into_any_element();
        };
        let created = self.area.is_none();
        if created {
            let (area, _skin) = DockSkin::dock_area(format!("kit-dock-{id}"), Some(1), window, cx);
            let last = self.last_layout.clone();
            self.subscription = Some(cx.subscribe(&area, move |_, area, event, cx| {
                if let DockEvent::LayoutChanged = event
                    && let Ok(value) = serde_json::to_value(area.read(cx).dump(cx))
                {
                    last.borrow_mut().clone_from(&value);
                    change(&callback, id, value);
                }
                cx.notify();
            }));
            self.area = Some(area);
        }
        let area = self.area.as_ref().expect("initialized dock").clone();
        let values = ctx
            .props
            .get("panels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let live = values
            .iter()
            .filter_map(|v| v.get("id").and_then(Value::as_str))
            .collect::<HashSet<_>>();
        let stale = self
            .panels
            .keys()
            .filter(|key| !live.contains(key.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(panel) = self.panels.remove(&key) {
                area.update(cx, |area, cx| area.remove_panel(panel, window, cx));
            }
        }
        for (index, value) in values.iter().enumerate() {
            let key = value.get("id").and_then(Value::as_str).unwrap_or_default();
            let title = kit_text(
                &ctx,
                index,
                value
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(key)
                    .to_owned(),
            );
            let closable = value
                .get("closable")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            if let Some(panel) = self.panels.get(key) {
                panel.update(cx, |panel, cx| {
                    panel.index = index;
                    panel.render = render.clone();
                    panel.title = title;
                    panel.closable = closable;
                    if self.revision != Some(ctx.revision) {
                        cx.notify();
                    }
                });
            } else {
                let panel = cx.new(|cx| Panel {
                    key: key.into(),
                    title,
                    closable,
                    index,
                    render: render.clone(),
                    focus: cx.focus_handle(),
                });
                let view = panel_handle(panel.clone());
                area.update(cx, |area, cx| {
                    area.add_panel_view(view, placement(value.get("placement")), None, window, cx);
                });
                self.panels.insert(key.into(), panel);
            }
        }
        if self.dirty.contains("layout") && !ctx.props.contains_key("layout") {
            area.update(cx, |area, cx| {
                area.set_center(DockLayout::tabs(), window, cx);
                for place in [
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ] {
                    area.remove_dock(place, window, cx);
                }
                for value in &values {
                    if let Some(panel) = value
                        .get("id")
                        .and_then(Value::as_str)
                        .and_then(|id| self.panels.get(id))
                    {
                        area.add_panel_view(
                            panel_handle(panel.clone()),
                            placement(value.get("placement")),
                            None,
                            window,
                            cx,
                        );
                    }
                }
            });
        }
        if (created || self.dirty.contains("layout"))
            && ctx.props.get("layout") != Some(&*self.last_layout.borrow())
            && let Some(saved) = ctx
                .props
                .get("layout")
                .and_then(|value| serde_json::from_value::<DockAreaState>(value.clone()).ok())
        {
            area.update(cx, |area, cx| {
                area.set_center(layout(&saved.center, &self.panels, cx), window, cx);
                for (placement, state) in [
                    (DockPlacement::Left, saved.left_dock),
                    (DockPlacement::Right, saved.right_dock),
                    (DockPlacement::Bottom, saved.bottom_dock),
                ] {
                    if let Some(state) = state {
                        area.set_dock(
                            placement,
                            layout(state.panel(), &self.panels, cx),
                            window,
                            cx,
                        );
                        area.set_dock_size(placement, state.size(), window, cx);
                        if !state.open() {
                            area.toggle_dock(placement, window, cx);
                        }
                    } else {
                        area.remove_dock(placement, window, cx);
                    }
                }
            });
        }
        area.update(cx, |area, cx| {
            area.set_locked(boolean(&ctx, "locked"), window, cx);
        });
        self.dirty.clear();
        self.revision = Some(ctx.revision);
        surface(&ctx).child(area).into_any_element()
    }
}
