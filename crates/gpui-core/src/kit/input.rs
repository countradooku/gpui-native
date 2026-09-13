//! Kit editing controls keep IME, undo history and caret state in native entities.
use super::elements::{boolean, change, integer, size, string, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    },
    renderer::{GpuiView, emit_event_full},
};
use gpui::{AnyElement, Context, Entity, Subscription, Window, prelude::*};
use gpui_base::input::{InputBaseState, InputModeKind};
use gpui_component::{
    Disableable as _, Sizable as _,
    input::{
        Editor, EditorState, Input, InputEvent, InputState, NumberInput, NumberStep, OtpEvent,
        OtpInput, OtpState, Textarea, TextareaState,
    },
};
use serde_json::Value;
use std::collections::HashSet;
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-input",
        &[
            "value",
            "placeholder",
            "disabled",
            "readonly",
            "masked",
            "cleanable",
            "appearance",
            "bordered",
            "size",
        ],
    ),
    (
        "kit-textarea",
        &[
            "value",
            "placeholder",
            "disabled",
            "readonly",
            "rows",
            "softWrap",
            "appearance",
            "bordered",
        ],
    ),
    (
        "kit-editor",
        &[
            "value",
            "placeholder",
            "disabled",
            "readonly",
            "language",
            "lineNumbers",
            "softWrap",
            "searchable",
            "appearance",
            "bordered",
        ],
    ),
    (
        "kit-number-input",
        &[
            "value",
            "placeholder",
            "disabled",
            "min",
            "max",
            "step",
            "appearance",
            "size",
        ],
    ),
    (
        "kit-otp-input",
        &["value", "length", "groups", "masked", "disabled", "size"],
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
        })
    }
}
enum State {
    Input(Entity<InputState>),
    Textarea(Entity<TextareaState>),
    Editor(Entity<EditorState>),
    Otp(Entity<OtpState>),
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
    state: Option<State>,
    subscription: Option<Subscription>,
    dirty: HashSet<String>,
}
fn subscribe<M: InputModeKind>(
    state: &Entity<InputBaseState<M>>,
    ctx: &CustomRenderContext<'_>,
    cx: &mut Context<GpuiView>,
) -> Subscription {
    let callback = ctx.event_callback.clone();
    let id = ctx.id;
    cx.subscribe(state, move |_, state, event, cx| {
        match event {
            InputEvent::Change => change(&callback, id, state.read(cx).value().to_string()),
            InputEvent::PressEnter { .. } => emit_event_full(&callback, id, "submit", |event| {
                event.value = Some(state.read(cx).value().to_string());
            }),
            InputEvent::Focus | InputEvent::Blur => {}
        }
        cx.notify();
    })
}
fn sync<M: InputModeKind>(
    state: &mut InputBaseState<M>,
    ctx: &CustomRenderContext<'_>,
    dirty: &HashSet<String>,
    created: bool,
    window: &mut Window,
    cx: &mut Context<InputBaseState<M>>,
) {
    if created || dirty.contains("value") {
        let value = string(ctx, "value");
        if state.value().as_ref() != value {
            state.set_value(value, window, cx);
        }
    }
    if created || dirty.contains("placeholder") {
        state.set_placeholder(string(ctx, "placeholder"), window, cx);
    }
}
fn flag(ctx: &CustomRenderContext<'_>, key: &str) -> bool {
    ctx.props.get(key).and_then(Value::as_bool).unwrap_or(true)
}
impl CustomElement for Control {
    fn native_focus_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        use gpui::Focusable as _;
        Some(match self.state.as_ref()? {
            State::Input(state) => state.focus_handle(cx),
            State::Textarea(state) => state.focus_handle(cx),
            State::Editor(state) => state.focus_handle(cx),
            State::Otp(state) => state.focus_handle(cx),
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
            "submit",
            "focus",
            "blur",
            "mouseEnter",
            "mouseLeave",
            "keyDown",
            "keyUp",
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
        if self.dirty.contains("length") {
            self.subscription = None;
            self.state = None;
        }
        let created = self.state.is_none();
        if created {
            self.state = Some(match self.name {
                "kit-textarea" => {
                    let state = cx.new(|cx| TextareaState::new(window, cx));
                    self.subscription = Some(subscribe(&state, &ctx, cx));
                    State::Textarea(state)
                }
                "kit-editor" => {
                    let state = cx.new(|cx| EditorState::new(window, cx));
                    self.subscription = Some(subscribe(&state, &ctx, cx));
                    State::Editor(state)
                }
                "kit-otp-input" => {
                    let state = cx.new(|cx| OtpState::new(integer(&ctx, "length", 6), window, cx));
                    let callback = ctx.event_callback.clone();
                    let id = ctx.id;
                    self.subscription = Some(cx.subscribe(&state, move |_, state, event, cx| {
                        match event {
                            OtpEvent::Change => {
                                change(&callback, id, state.read(cx).value().to_string());
                            }
                            OtpEvent::Complete => {
                                emit_event_full(&callback, id, "submit", |event| {
                                    event.value = Some(state.read(cx).value().to_string());
                                });
                            }
                            OtpEvent::Focus | OtpEvent::Blur => {}
                        }
                        cx.notify();
                    }));
                    State::Otp(state)
                }
                _ => {
                    let state = cx.new(|cx| InputState::new(window, cx));
                    self.subscription = Some(subscribe(&state, &ctx, cx));
                    State::Input(state)
                }
            });
        }
        let component = match self.state.as_ref() {
            Some(State::Input(state)) => {
                state.update(cx, |state, cx| {
                    sync(state, &ctx, &self.dirty, created, window, cx);
                    if created || self.dirty.contains("masked") {
                        state.set_masked(boolean(&ctx, "masked"), window, cx);
                    }
                    if self.name == "kit-number-input" {
                        if created || self.dirty.contains("min") {
                            state.set_min(ctx.props.get("min").and_then(Value::as_f64), window, cx);
                        }
                        if created || self.dirty.contains("max") {
                            state.set_max(ctx.props.get("max").and_then(Value::as_f64), window, cx);
                        }
                        if created || self.dirty.contains("step") {
                            state.set_step(
                                NumberStep::from(
                                    ctx.props.get("step").and_then(Value::as_f64).unwrap_or(1.),
                                ),
                                window,
                                cx,
                            );
                        }
                    }
                });
                if self.name == "kit-number-input" {
                    NumberInput::new(state)
                        .placeholder(string(&ctx, "placeholder"))
                        .disabled(boolean(&ctx, "disabled"))
                        .appearance(flag(&ctx, "appearance"))
                        .with_size(size(&ctx))
                        .into_any_element()
                } else {
                    Input::new(state)
                        .disabled(boolean(&ctx, "disabled"))
                        .readonly(boolean(&ctx, "readonly"))
                        .cleanable(boolean(&ctx, "cleanable"))
                        .appearance(flag(&ctx, "appearance"))
                        .bordered(flag(&ctx, "bordered"))
                        .with_size(size(&ctx))
                        .into_any_element()
                }
            }
            Some(State::Textarea(state)) => {
                state.update(cx, |state, cx| {
                    sync(state, &ctx, &self.dirty, created, window, cx);
                    if created || self.dirty.contains("rows") {
                        state.set_rows(integer(&ctx, "rows", 3), cx);
                    }
                    if created || self.dirty.contains("softWrap") {
                        state.set_soft_wrap(flag(&ctx, "softWrap"), window, cx);
                    }
                });
                Textarea::new(state)
                    .disabled(boolean(&ctx, "disabled"))
                    .readonly(boolean(&ctx, "readonly"))
                    .appearance(flag(&ctx, "appearance"))
                    .bordered(flag(&ctx, "bordered"))
                    .into_any_element()
            }
            Some(State::Editor(state)) => {
                state.update(cx, |state, cx| {
                    if created {
                        state.set_highlighter_factory(super::highlighter::factory(), cx);
                    }
                    sync(state, &ctx, &self.dirty, created, window, cx);
                    if created || self.dirty.contains("language") {
                        state.set_highlighter(string(&ctx, "language"), cx);
                    }
                    if created || self.dirty.contains("lineNumbers") {
                        state.set_line_number(flag(&ctx, "lineNumbers"), window, cx);
                    }
                    if created || self.dirty.contains("softWrap") {
                        state.set_soft_wrap(flag(&ctx, "softWrap"), window, cx);
                    }
                    if created || self.dirty.contains("searchable") {
                        state.set_searchable(flag(&ctx, "searchable"), cx);
                    }
                });
                Editor::new(state)
                    .disabled(boolean(&ctx, "disabled"))
                    .readonly(boolean(&ctx, "readonly"))
                    .appearance(flag(&ctx, "appearance"))
                    .bordered(flag(&ctx, "bordered"))
                    .into_any_element()
            }
            Some(State::Otp(state)) => {
                state.update(cx, |state, cx| {
                    if created || self.dirty.contains("value") {
                        let value = string(&ctx, "value");
                        if state.value().as_ref() != value {
                            state.set_value(value, window, cx);
                        }
                    }
                    if created || self.dirty.contains("masked") {
                        state.set_masked(boolean(&ctx, "masked"), window, cx);
                    }
                });
                OtpInput::new(state)
                    .groups(integer(&ctx, "groups", 2))
                    .disabled(boolean(&ctx, "disabled"))
                    .with_size(size(&ctx))
                    .into_any_element()
            }
            None => unreachable!("initialized input state"),
        };
        self.dirty.clear();
        surface(&ctx)
            .child(crate::automation::bounds_tracker(ctx.id, Some(false)))
            .child(component)
            .into_any_element()
    }
}
