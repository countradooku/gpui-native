//! Declarative configuration of Kit's application-wide theme and locale.
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
    },
    renderer::GpuiView,
};
use gpui::{AnyElement, Context, Window, prelude::*};
use gpui_component::{SemanticThemeConfig, Theme, ThemeMode, ThemeRegistry};
use serde_json::Value;
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[(
    "kit-theme",
    &[
        "mode",
        "locale",
        "tokens",
        "fontFamily",
        "fontSize",
        "monoFontFamily",
        "monoFontSize",
        "radius",
        "shadow",
        "focusRing",
    ],
)];
pub(crate) fn register(registry: &mut CustomElementRegistry) {
    registry.register(Box::new(Factory));
}
struct Factory;
impl CustomElementFactory for Factory {
    fn element_type(&self) -> &'static str {
        "kit-theme"
    }
    fn create(&self, _: u64) -> Box<dyn CustomElement> {
        Box::new(Configuration {
            dirty: true,
            mode: None,
        })
    }
}
struct Configuration {
    mode: Option<ThemeMode>,
    dirty: bool,
}
impl CustomElement for Configuration {
    fn set_prop(&mut self, _: &str, _: Value) {
        self.dirty = true;
    }
    fn supported_props(&self) -> &'static [&'static str] {
        COMPONENTS[0].1
    }
    fn supported_events(&self) -> &'static [&'static str] {
        &[
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
        window: &mut Window,
        cx: &mut Context<GpuiView>,
    ) -> AnyElement {
        let mode = match super::elements::string(&ctx, "mode").as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => window.appearance().into(),
        };
        if self.dirty || self.mode != Some(mode) {
            self.mode = Some(mode);
            let reset = Theme {
                light_theme: ThemeRegistry::global(cx).default_light_theme().clone(),
                dark_theme: ThemeRegistry::global(cx).default_dark_theme().clone(),
                ..Theme::default()
            };
            cx.set_global(reset);
            Theme::change(mode, Some(window), cx);
            let theme = Theme::global_mut(cx);
            if let Some(tokens) = ctx
                .props
                .get("tokens")
                .and_then(|v| serde_json::from_value::<SemanticThemeConfig>(v.clone()).ok())
            {
                theme.apply_semantic_config(&tokens);
            }
            if let Some(value) = ctx.props.get("fontFamily").and_then(Value::as_str) {
                theme.font_family = value.to_owned().into();
            }
            if let Some(value) = ctx.props.get("monoFontFamily").and_then(Value::as_str) {
                theme.mono_font_family = value.to_owned().into();
            }
            theme.font_size = gpui::px(super::elements::number(
                &ctx,
                "fontSize",
                theme.font_size.into(),
            ));
            theme.mono_font_size = gpui::px(super::elements::number(
                &ctx,
                "monoFontSize",
                theme.mono_font_size.into(),
            ));
            theme.radius = gpui::px(super::elements::number(&ctx, "radius", theme.radius.into()));
            theme.shadow = ctx
                .props
                .get("shadow")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            theme.focus_ring = ctx
                .props
                .get("focusRing")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            Theme::sync_base(cx);
            gpui_component::set_locale(
                ctx.props
                    .get("locale")
                    .and_then(Value::as_str)
                    .unwrap_or("en"),
            );
            self.dirty = false;
        }
        let children = std::mem::take(&mut ctx.children);
        super::elements::surface(&ctx)
            .children(children)
            .into_any_element()
    }
}
