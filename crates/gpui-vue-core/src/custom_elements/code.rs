//! `<code>` — a syntax-highlighted, selectable code block.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//! Original: `render_code_block` in `crates/ui/src/markdown/render.rs`.
//!
//! ```tsx
//! <code
//!   code={source}
//!   language="typescript"       // or path="src/app.ts"
//!   showLineNumbers
//!   theme={{ syntax: { keyword: '#f38ba8' } }}
//! />
//! ```
//!
//! The block renders **one div per line** at an exact `CODE_LINE_HEIGHT`, so
//! its height is `lines × lineHeight + padding + header` before any
//! highlighting has run. Highlighting is pure paint: every run on a line shares
//! the same font and differs only in colour, so a late highlight can never
//! reflow the block.

use std::sync::Arc;

use gpui::{px, Font, SharedString};

use super::{CustomElement, CustomElementFactory, CustomRenderContext};
use crate::syntax::{cache::highlight_cached, HighlightedDocument};
use crate::text::runs::runs_for_spans;
use crate::theme::{Metrics, Theme};

// ── Factory ──────────────────────────────────────────────────────────

pub struct CodeFactory;

impl CustomElementFactory for CodeFactory {
    fn element_type(&self) -> &str {
        "code"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(CodeElement::default())
    }
}

// ── Element ──────────────────────────────────────────────────────────

#[derive(Default)]
pub struct CodeElement {
    code: String,
    language: Option<String>,
    path: Option<String>,
    show_line_numbers: bool,
    show_header: bool,
    theme: Theme,
    /// Cached highlight for the current `(code, language, path)`. The syntax
    /// cache already dedupes parsing, but this avoids hashing the source on
    /// every frame too.
    highlight: Option<Arc<HighlightedDocument>>,
    highlight_key: Option<(usize, u64)>,
}

impl CodeElement {
    /// Resolve the highlight for the current props, reusing the last result
    /// when nothing changed.
    fn resolve_highlight(&mut self) -> Option<Arc<HighlightedDocument>> {
        let key = (
            self.code.len(),
            fingerprint(&self.code, &self.language, &self.path),
        );
        if self.highlight_key == Some(key) {
            return self.highlight.clone();
        }
        self.highlight_key = Some(key);
        self.highlight =
            highlight_cached(&self.code, self.path.as_deref(), self.language.as_deref());
        self.highlight.clone()
    }

    fn mono(&self) -> Font {
        gpui::font(self.theme.font_mono.clone())
    }
}

fn fingerprint(code: &str, language: &Option<String>, path: &Option<String>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    language.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish()
}

impl CustomElement for CodeElement {
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<crate::renderer::GpuiView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let theme = self.theme.clone();
        let m = &theme.metrics;
        let highlight = self.resolve_highlight();
        let mono = self.mono();
        let lines: Vec<&str> = self.code.split('\n').collect();
        let gutter_width = gutter_width(lines.len(), m);

        let mut body = gpui::div()
            .id(SharedString::from(format!(
                "__gpui_vue_code_body_{}",
                ctx.id
            )))
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .min_w_0()
            .px(px(m.code_padding_x))
            .py(px(m.code_padding_y))
            .font_family(theme.font_mono.clone())
            .text_size(px(m.code_text_size))
            .line_height(px(m.code_line_height))
            .whitespace_nowrap()
            .flex()
            .flex_col();

        for (line_ix, line) in lines.iter().enumerate() {
            let spans: Vec<(std::ops::Range<usize>, gpui::Hsla)> = highlight
                .as_ref()
                .and_then(|doc| doc.lines.get(line_ix))
                .map(|spans| {
                    spans
                        .iter()
                        .map(|span| (span.range.clone(), theme.syntax.color(span.kind)))
                        .collect()
                })
                .unwrap_or_default();
            let runs = runs_for_spans(line, &spans, &mono, theme.text);

            let mut row = gpui::div()
                .h(px(m.code_line_height))
                .flex_none()
                .flex()
                .flex_row();

            if self.show_line_numbers {
                row = row.child(
                    gpui::div()
                        .w(px(gutter_width))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .pr(px(m.code_gutter_padding_right))
                        .text_color(theme.text_faint)
                        // The gutter is chrome, not content: a drag across the
                        // block must copy code, never a column of numbers.
                        .child(ctx.chrome_text((line_ix + 1).to_string(), None)),
                );
            }

            // `sub` is the line index so each line owns a stable selection key
            // across frames. Using the element id alone would make every line
            // share one key and the wash would paint on all of them at once.
            body = body.child(row.child(ctx.text(line_ix, line.to_string(), Some(runs))));
        }

        let mut block = gpui::div()
            .rounded(px(m.code_radius))
            .bg(ink(&theme, 0.035))
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .relative();

        if self.show_header {
            if let Some(language) = self.language.clone() {
                block = block.child(
                    gpui::div()
                        .px(px(m.code_padding_x))
                        .py(px(m.code_header_padding_y))
                        .border_b_1()
                        .border_color(theme.border)
                        .bg(ink(&theme, 0.02))
                        .text_size(px(m.code_header_text_size))
                        .text_color(theme.text_muted)
                        .child(ctx.chrome_text(language, None)),
                );
            }
        }

        let mut block = block
            .id(SharedString::from(format!("__gpui_vue_code_{}", ctx.id)))
            .child(body);
        if let Some(style) = ctx.style {
            block = crate::renderer::apply_styles(block, style);
        }
        block = wire_standard_events(block, &ctx);
        block.into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        match key {
            "code" => self.code = value.as_str().unwrap_or("").to_string(),
            "language" => self.language = value.as_str().map(str::to_string),
            "path" => self.path = value.as_str().map(str::to_string),
            "showLineNumbers" => self.show_line_numbers = value.as_bool().unwrap_or(false),
            // The header defaults ON when a language is set, matching Comet's
            // markdown code blocks. Pass `showHeader={false}` for a bare block.
            "showHeader" => self.show_header = value.as_bool().unwrap_or(true),
            "theme" => self.theme = Theme::from_prop(Some(&value)),
            _ => {}
        }
    }

    fn supported_props(&self) -> &[&str] {
        &[
            "code",
            "language",
            "path",
            "showLineNumbers",
            "showHeader",
            "theme",
        ]
    }

    fn get_prop(&self, key: &str) -> Option<serde_json::Value> {
        match key {
            "code" => Some(serde_json::Value::String(self.code.clone())),
            "language" => self
                .language
                .clone()
                .map(serde_json::Value::String)
                .or(Some(serde_json::Value::Null)),
            _ => None,
        }
    }

    fn supported_events(&self) -> &[&str] {
        &["click", "mouseEnter", "mouseLeave"]
    }

    fn destroy(&mut self) {}
}

/// Attach the mouse events a custom element declares in `supported_events`.
///
/// Declaring an event and never installing a handler is worse than not
/// supporting it: the prop type-checks, the listener is registered on the JS
/// side, and nothing ever fires.
pub(crate) fn wire_standard_events(
    mut el: gpui::Stateful<gpui::Div>,
    ctx: &CustomRenderContext,
) -> gpui::Stateful<gpui::Div> {
    use gpui::prelude::*;

    let id = ctx.id;
    for event in ctx.events {
        let callback = ctx.event_callback.clone();
        match event.as_str() {
            "click" => {
                el = el.on_click(move |click, _window, _cx| {
                    crate::renderer::emit_event_full(&callback, id, "click", |p| {
                        let (x, y) = crate::renderer::point_to_xy(click.position());
                        p.x = Some(x);
                        p.y = Some(y);
                        p.click_count = Some(click.click_count() as u32);
                        p.modifiers = Some(click.modifiers().into());
                    });
                });
            }
            "mouseEnter" | "mouseLeave" => {
                // gpui reports both edges through one listener, so wire it once.
                if event == "mouseEnter" || !ctx.events.contains("mouseEnter") {
                    let enter = ctx.events.contains("mouseEnter");
                    let leave = ctx.events.contains("mouseLeave");
                    let callback = ctx.event_callback.clone();
                    el = el.on_hover(move |&hovered, _window, _cx| {
                        let kind = if hovered { "mouseEnter" } else { "mouseLeave" };
                        if (hovered && enter) || (!hovered && leave) {
                            crate::renderer::emit_event_full(&callback, id, kind, |p| {
                                p.hovered = Some(hovered);
                            });
                        }
                    });
                }
            }
            _ => {}
        }
    }
    el
}

/// Line-number gutter width, sized analytically from the digit count so the
/// code column never shifts as the block scrolls.
fn gutter_width(line_count: usize, m: &Metrics) -> f32 {
    let digits = line_count.max(1).to_string().len() as f32;
    (digits * m.code_gutter_digit_width + m.code_gutter_padding_right).max(m.code_gutter_min_width)
}

/// Translucent white on dark, translucent black on light — Comet's `ink`.
fn ink(theme: &Theme, alpha: f32) -> gpui::Hsla {
    // `bg` lightness is the appearance tell: near-black in dark, white in light.
    let lightness = if theme.bg.l < 0.5 { 1.0 } else { 0.0 };
    gpui::hsla(0.0, 0.0, lightness, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_grows_with_digit_count() {
        let m = Metrics::default();
        assert_eq!(gutter_width(9, &m), m.code_gutter_min_width);
        assert!(gutter_width(1000, &m) > gutter_width(10, &m));
    }

    #[test]
    fn gutter_follows_the_metrics_override() {
        let mut m = Metrics::default();
        m.code_gutter_min_width = 64.0;
        assert_eq!(gutter_width(9, &m), 64.0);
    }

    #[test]
    fn ink_flips_with_appearance() {
        assert_eq!(ink(&Theme::dark(), 0.1).l, 1.0);
        assert_eq!(ink(&Theme::light(), 0.1).l, 0.0);
    }

    #[test]
    fn highlight_is_reused_until_props_change() {
        let mut element = CodeElement {
            code: "let a = 1;".to_string(),
            path: Some("a.rs".to_string()),
            ..Default::default()
        };
        let first = element.resolve_highlight().unwrap();
        let second = element.resolve_highlight().unwrap();
        assert!(Arc::ptr_eq(&first, &second));

        element.code = "let bb = 2;".to_string();
        let third = element.resolve_highlight().unwrap();
        assert!(!Arc::ptr_eq(&first, &third));
    }
}
