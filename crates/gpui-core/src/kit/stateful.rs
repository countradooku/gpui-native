//! Entity-backed controls retain their native state until their host node is removed.

use super::elements::{boolean, change, integer, number, size, string};
use crate::custom_elements::{
    CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
};
use crate::renderer::GpuiView;
use gpui::{AnyElement, Context, Entity, Subscription, Window, prelude::*};
use gpui_base::slider::{SliderEvent, SliderState, SliderValue};
use gpui_base::{CalendarEvent, CalendarState, ColorPickerEvent, ColorPickerState, Date};
use gpui_component::date_picker::{DatePickerEvent, DatePickerState};
use gpui_component::{Disableable as _, Sizable as _};
use serde_json::{Value, json};
use std::collections::HashSet;

pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-slider",
        &["value", "min", "max", "step", "vertical", "disabled"],
    ),
    (
        "kit-calendar",
        &["value", "range", "numberOfMonths", "size"],
    ),
    (
        "kit-date-picker",
        &[
            "value",
            "range",
            "placeholder",
            "cleanable",
            "disabled",
            "numberOfMonths",
            "size",
        ],
    ),
    ("kit-color-picker", &["value", "size"]),
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
        })
    }
}
enum State {
    Slider(Entity<SliderState>),
    Calendar(Entity<CalendarState>),
    Date(Entity<DatePickerState>),
    Color(Entity<ColorPickerState>),
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    state: Option<State>,
    subscription: Option<Subscription>,
    dirty: HashSet<String>,
}
fn date(ctx: &CustomRenderContext<'_>) -> Date {
    let parse = |v: &Value| {
        v.as_str()
            .and_then(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
    };
    match ctx.props.get("value") {
        Some(Value::Array(v)) => Date::Range(v.first().and_then(parse), v.get(1).and_then(parse)),
        Some(v) => Date::Single(parse(v)),
        None if boolean(ctx, "range") => Date::Range(None, None),
        None => Date::Single(None),
    }
}
fn date_json(v: Date) -> Value {
    match v {
        Date::Single(v) => json!(v.map(|d| d.to_string())),
        Date::Range(a, b) => json!([a.map(|d| d.to_string()), b.map(|d| d.to_string())]),
    }
}
fn slider_value(ctx: &CustomRenderContext<'_>) -> SliderValue {
    if let Some(Value::Array(v)) = ctx.props.get("value") {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "GPUI slider values use f32"
        )]
        return SliderValue::Range(
            v.first().and_then(Value::as_f64).unwrap_or(0.) as f32,
            v.get(1).and_then(Value::as_f64).unwrap_or(100.) as f32,
        );
    }
    SliderValue::Single(number(ctx, "value", 0.))
}
impl CustomElement for Control {
    fn native_focus_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        use gpui::Focusable as _;
        Some(match self.state.as_ref()? {
            State::Slider(_) | State::Calendar(_) => return None,
            State::Date(state) => state.focus_handle(cx),
            State::Color(state) => state.focus_handle(cx),
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
        self.dirty.clear();
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
        if self.name == "kit-slider"
            && self
                .dirty
                .iter()
                .any(|key| matches!(key.as_str(), "min" | "max" | "step"))
        {
            self.state = None;
            self.subscription = None;
        }
        let created = self.state.is_none();
        if created {
            self.state = Some(match self.name {
                "kit-slider" => {
                    let min = number(&ctx, "min", 0.);
                    let max = number(&ctx, "max", 100.).max(min);
                    let state = cx.new(|_| {
                        SliderState::new()
                            .min(min)
                            .max(max)
                            .step(number(&ctx, "step", 1.))
                            .default_value(slider_value(&ctx))
                    });
                    self.subscription = Some(cx.subscribe(&state, move |_, _, event, cx| {
                        if let SliderEvent::Change(v) = event {
                            change(
                                &callback,
                                id,
                                match v {
                                    SliderValue::Single(v) => json!(v),
                                    SliderValue::Range(a, b) => json!([a, b]),
                                },
                            );
                        }
                        cx.notify();
                    }));
                    State::Slider(state)
                }
                "kit-calendar" => {
                    let state = cx.new(|cx| CalendarState::new(window, cx));
                    self.subscription = Some(cx.subscribe(&state, move |_, _, event, cx| {
                        let CalendarEvent::Selected(v) = event;
                        change(&callback, id, date_json(*v));
                        cx.notify();
                    }));
                    State::Calendar(state)
                }
                "kit-date-picker" => {
                    let state = cx.new(|cx| DatePickerState::new(window, cx));
                    self.subscription = Some(cx.subscribe(&state, move |_, _, event, cx| {
                        let DatePickerEvent::Change(v) = event;
                        change(&callback, id, date_json(*v));
                        cx.notify();
                    }));
                    State::Date(state)
                }
                "kit-color-picker" => {
                    let state = cx.new(|cx| ColorPickerState::new(window, cx));
                    self.subscription = Some(cx.subscribe(&state, move |_, _, event, cx| {
                        let ColorPickerEvent::Change(v) = event;
                        change(
                            &callback,
                            id,
                            json!(v.map(|v| format!("#{:08x}", u32::from(gpui::Rgba::from(v))))),
                        );
                        cx.notify();
                    }));
                    State::Color(state)
                }
                _ => unreachable!("registered stateful Kit control"),
            });
        }
        let sync = created || self.dirty.contains("value") || self.dirty.contains("range");
        let component = match self.state.as_ref() {
            Some(State::Slider(state)) => {
                if sync {
                    state.update(cx, |s, cx| s.set_value(slider_value(&ctx), window, cx));
                }
                let mut slider =
                    gpui_component::slider::Slider::new(state).disabled(boolean(&ctx, "disabled"));
                if boolean(&ctx, "vertical") {
                    slider = slider.vertical();
                }
                slider.into_any_element()
            }
            Some(State::Calendar(state)) => {
                if sync {
                    state.update(cx, |s, cx| s.set_date(date(&ctx), window, cx));
                }
                gpui_component::calendar::Calendar::new(state)
                    .number_of_months(integer(&ctx, "numberOfMonths", 1))
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            Some(State::Date(state)) => {
                if sync {
                    state.update(cx, |s, cx| s.set_date(date(&ctx), window, cx));
                }
                gpui_component::date_picker::DatePicker::new(state)
                    .placeholder(string(&ctx, "placeholder"))
                    .cleanable(boolean(&ctx, "cleanable"))
                    .disabled(boolean(&ctx, "disabled"))
                    .number_of_months(integer(&ctx, "numberOfMonths", 1))
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            Some(State::Color(state)) => {
                if sync {
                    let color = crate::color::parse_color_rgba(&string(&ctx, "value"));
                    state.update(cx, |s, cx| {
                        if let Some(color) = color {
                            s.set_value(color, window, cx);
                        } else {
                            s.clear_value(window, cx);
                        }
                    });
                }
                gpui_component::color_picker::ColorPicker::new(state)
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            None => unreachable!("state was initialized above"),
        };
        self.dirty.clear();
        super::elements::surface(&ctx)
            .child(component)
            .into_any_element()
    }
}
