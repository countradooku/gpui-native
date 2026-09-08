//! `<markdown>` — GitHub-flavoured markdown rendered natively and selectable.
//!
//! ```tsx
//! <markdown source={text} theme={{ accent: '#7c86ff' }} onLinkClick={(e) => {}} />
//! ```
//!
//! Every paragraph, heading, table cell and code line registers into the shared
//! selection registry in document order, so a drag can start in a heading and
//! end inside a fenced code block, and Cmd+C copies the whole span.

use std::rc::Rc;
use std::sync::Arc;

use super::{CustomElement, CustomElementFactory, CustomRenderContext};
use crate::markdown::parser::{BlockTree, parse};
use crate::markdown::render::{MdContext, PreparedCodeBlock, prepare_code_blocks, render_tree};
use crate::renderer::emit_event_full;
use crate::theme::Theme;

pub struct MarkdownFactory;

impl CustomElementFactory for MarkdownFactory {
    fn element_type(&self) -> &'static str {
        "markdown"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(MarkdownElement::default())
    }
}

#[derive(Default)]
pub struct MarkdownElement {
    source: String,
    theme: Theme,
    /// Parsed tree for the current source. `Rc` so a frame clones a pointer
    /// rather than every block, string and inline run in the document.
    tree: Option<Rc<BlockTree>>,
    code_blocks: Arc<[PreparedCodeBlock]>,
}

impl MarkdownElement {
    fn tree(&mut self) -> Rc<BlockTree> {
        if self.tree.is_none() {
            let tree = Rc::new(parse(&self.source));
            self.code_blocks = prepare_code_blocks(&tree);
            self.tree = Some(tree);
        }
        self.tree.clone().expect("just parsed")
    }
}

impl CustomElement for MarkdownElement {
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        window: &mut gpui::Window,
        _cx: &mut gpui::Context<crate::renderer::GpuiView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let theme = self.theme.clone();
        let tree = self.tree();

        // Link clicks are hit-tested per byte range inside the painted text, so
        // clicking prose emits nothing and clicking the second link emits the
        // second URL.
        let on_link: Option<crate::text::paint::LinkCallback> = if ctx.events.contains("linkClick")
        {
            let callback = ctx.event_callback.clone();
            let element_id = ctx.id;
            Some(Arc::new(move |url: &str| {
                let url = url.to_string();
                emit_event_full(&callback, element_id, "linkClick", |p| {
                    p.value = Some(url);
                });
            }))
        } else {
            None
        };

        let mut md = MdContext::new(
            ctx.id,
            ctx.selection.clone(),
            ctx.selectable,
            ctx.selection_wash,
            theme.clone(),
            on_link,
            ctx.highlight_set.clone(),
            self.code_blocks.clone(),
        );
        let body = render_tree(&tree, &mut md, window);

        let container = gpui::div().id(super::custom_element_id("__gpui_vue_markdown", ctx.id));
        let container = super::custom_surface(container, &ctx)
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .text_color(theme.text)
            .font_family(theme.font_sans.clone())
            .text_size(gpui::px(theme.metrics.md_text_size))
            .line_height(gpui::px(theme.metrics.md_line_height))
            .child(body);

        container.into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        match key {
            "source" => {
                let source = value.as_str().unwrap_or("");
                if self.source != source {
                    self.source.clear();
                    self.source.push_str(source);
                    self.tree = None;
                    self.code_blocks = Arc::default();
                }
            }
            "theme" => self.theme = Theme::from_prop(Some(&value)),
            _ => {}
        }
    }

    fn supported_props(&self) -> &'static [&'static str] {
        &["source", "theme"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &["linkClick", "click", "mouseEnter", "mouseLeave"]
    }

    fn destroy(&mut self) {
        self.tree = None;
    }
}
