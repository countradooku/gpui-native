//! The Kit editor uses the same bounded, platform-independent syntax engine as code blocks.
use crate::syntax::{self, HighlightKind, HighlightRequest};
use gpui::{Context, HighlightStyle, SharedString, Window};
use gpui_base::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
    InputHighlighterFactory, Rope,
};
use std::{ops::Range, rc::Rc};

pub(super) fn factory() -> InputHighlighterFactory {
    Rc::new(|language| {
        syntax::language_for_alias(language).map(|_| {
            Box::new(Highlighter {
                language: language.to_owned().into(),
                spans: Vec::new(),
            }) as Box<dyn InputHighlighter>
        })
    })
}
struct Highlighter {
    language: SharedString,
    spans: Vec<(Range<usize>, HighlightKind)>,
}
impl Highlighter {
    fn parse(&mut self, source: &str) {
        self.spans.clear();
        let Ok(document) = syntax::highlight(HighlightRequest {
            source,
            path: None,
            fence_tag: Some(&self.language),
        }) else {
            return;
        };
        let mut offset = 0;
        for (line, spans) in source.split_inclusive('\n').zip(document.lines) {
            self.spans.extend(spans.into_iter().map(|span| {
                (
                    span.range.start + offset..span.range.end + offset,
                    span.kind,
                )
            }));
            offset += line.len();
        }
    }
}
impl InputHighlighter for Highlighter {
    fn language(&self) -> SharedString {
        self.language.clone()
    }
    fn update(
        &mut self,
        _: Option<InputEdit>,
        text: &Rope,
        _: bool,
        _: &mut Window,
        _: &mut Context<EditorState>,
    ) {
        if text.len() > syntax::DEFAULT_MAX_SOURCE_BYTES {
            self.spans.clear();
            return;
        }
        self.parse(&text.to_string());
    }
    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut result = Vec::new();
        let mut cursor = range.start;
        for (span, kind) in &self.spans {
            if span.end <= cursor {
                continue;
            }
            if span.start >= range.end {
                break;
            }
            let start = span.start.max(cursor);
            let end = span.end.min(range.end);
            if start > cursor {
                result.push((cursor..start, HighlightStyle::default()));
            }
            result.push((
                start..end,
                resolver.style(semantic_name(*kind)).unwrap_or_default(),
            ));
            cursor = end;
        }
        if cursor < range.end {
            result.push((cursor..range.end, HighlightStyle::default()));
        }
        result
    }
    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}
fn semantic_name(kind: HighlightKind) -> &'static str {
    match kind {
        HighlightKind::Comment => "comment",
        HighlightKind::Keyword => "keyword",
        HighlightKind::String | HighlightKind::StringSpecial => "string",
        HighlightKind::Escape => "string.escape",
        HighlightKind::Number => "number",
        HighlightKind::Boolean => "boolean",
        HighlightKind::Type | HighlightKind::TypeBuiltin | HighlightKind::Constructor => "type",
        HighlightKind::Function | HighlightKind::FunctionBuiltin | HighlightKind::Macro => {
            "function"
        }
        HighlightKind::Property | HighlightKind::Attribute => "property",
        HighlightKind::Constant => "constant",
        HighlightKind::Variable | HighlightKind::VariableSpecial | HighlightKind::Parameter => {
            "variable"
        }
        HighlightKind::Operator => "operator",
        HighlightKind::Punctuation => "punctuation",
        HighlightKind::Tag => "tag",
        HighlightKind::Label => "label",
        HighlightKind::Embedded => "embedded",
        HighlightKind::Invalid => "error",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Resolver;
    impl HighlightStyleResolver for Resolver {
        fn style(&self, _: &str) -> Option<HighlightStyle> {
            Some(HighlightStyle {
                font_weight: Some(gpui::FontWeight::BOLD),
                ..Default::default()
            })
        }
    }
    #[test]
    fn unicode_multiline_ranges_cover_the_requested_slice() {
        let source = "let café = \"你好\";\n// second line\nlet n = 3;";
        let mut highlighter = Highlighter {
            language: "rust".into(),
            spans: vec![],
        };
        highlighter.parse(source);
        let start = source.find("你好").unwrap();
        let styles = highlighter.styles(&(start..source.len()), &Resolver);
        let mut cursor = start;
        for (range, _) in styles {
            assert_eq!(range.start, cursor);
            assert!(source.is_char_boundary(range.start));
            assert!(source.is_char_boundary(range.end));
            cursor = range.end;
        }
        assert_eq!(cursor, source.len());
        assert!(!highlighter.spans.is_empty());
        highlighter.parse("");
        assert!(highlighter.spans.is_empty());
    }
}
