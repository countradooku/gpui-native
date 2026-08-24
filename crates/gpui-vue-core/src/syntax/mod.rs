//! Tree-sitter syntax highlighting, reduced to a neutral contract.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//! Original: `crates/syntax/src/lib.rs`.
//!
//! The contract is deliberately **colour-free**: a [`HighlightSpan`] carries a
//! [`HighlightKind`], never an `Hsla`. Colour is applied later from the theme,
//! so switching appearance recolours existing spans instead of reparsing, and a
//! JS-supplied palette can override every token without touching this module.
//!
//! Ranges are byte offsets relative to one UTF-8 source **line**, which is what
//! per-line rendering needs and what makes a code block's height exact before
//! any highlighting has run.

pub mod cache;

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::Path;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Sources larger than this are rendered plain. Highlighting a megabyte of
/// minified JS blocks the frame for longer than anyone will tolerate.
pub const DEFAULT_MAX_SOURCE_BYTES: usize = 512 * 1024;
pub const DEFAULT_MAX_SPANS: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightLimits {
    pub max_source_bytes: usize,
    pub max_spans: usize,
}

impl Default for HighlightLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_spans: DEFAULT_MAX_SPANS,
        }
    }
}

/// Bundled grammars. Comet ships 28; this is the subset that covers the
/// languages a GPUI Vue app is likely to display, kept small because every grammar
/// is a C file compiled into the native binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Python,
    Go,
    Json,
    Jsonc,
    Bash,
    Toml,
    Markdown,
    Html,
    Css,
    Yaml,
    C,
}

/// A capture class. One per theme colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Comment,
    Keyword,
    String,
    StringSpecial,
    Escape,
    Number,
    Boolean,
    Type,
    TypeBuiltin,
    Constructor,
    Function,
    FunctionBuiltin,
    Macro,
    Property,
    Constant,
    Variable,
    VariableSpecial,
    Parameter,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Label,
    Embedded,
    Invalid,
}

impl HighlightKind {
    /// Stable precedence used to resolve overlapping parser captures.
    /// Without it, nested captures resolve by iteration order and a string
    /// inside a macro flickers between two colours across parses.
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Invalid => 100,
            Self::Escape => 95,
            Self::Macro => 90,
            Self::Property | Self::Attribute => 85,
            Self::FunctionBuiltin | Self::TypeBuiltin | Self::VariableSpecial => 80,
            Self::StringSpecial | Self::Constructor | Self::Parameter => 75,
            Self::Function | Self::Type | Self::Constant | Self::Tag | Self::Label => 70,
            Self::Comment | Self::Keyword | Self::String | Self::Number | Self::Boolean => 60,
            Self::Variable | Self::Operator => 50,
            Self::Punctuation | Self::Embedded => 40,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub kind: HighlightKind,
}

/// Highlight spans grouped per source line, sorted and non-overlapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedDocument {
    pub language: LanguageId,
    pub lines: Vec<Vec<HighlightSpan>>,
}

#[derive(Debug, Clone, Copy)]
pub struct HighlightRequest<'a> {
    pub source: &'a str,
    /// File path, used for extension-based detection.
    pub path: Option<&'a str>,
    /// Markdown fence tag such as `ts`, which beats the path.
    pub fence_tag: Option<&'a str>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HighlightError {
    #[error("the source language is not registered")]
    UnknownLanguage,
    #[error("highlight range {start}..{end} is invalid for a {len}-byte source")]
    InvalidRange {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("highlight range {start}..{end} is not on UTF-8 boundaries")]
    InvalidUtf8Boundary { start: usize, end: usize },
    #[error("source exceeds the configured highlighting limit")]
    SourceTooLarge,
    #[error("highlight output exceeds the configured span limit")]
    TooManySpans,
    #[error("parser failed: {0}")]
    Parser(String),
}

impl HighlightedDocument {
    /// Validate, split and normalize absolute source spans into line-relative
    /// spans. A span that crosses a newline becomes one span per line.
    pub fn from_absolute_spans(
        language: LanguageId,
        source: &str,
        spans: impl IntoIterator<Item = HighlightSpan>,
    ) -> Result<Self, HighlightError> {
        let starts = line_starts(source);
        let mut lines = vec![Vec::new(); starts.len()];
        for span in spans {
            validate_span(source, &span.range)?;
            if span.range.is_empty() {
                continue;
            }
            let first_line = starts.partition_point(|&start| start <= span.range.start) - 1;
            for (line_ix, &start) in starts.iter().enumerate().skip(first_line) {
                let raw_end = starts.get(line_ix + 1).copied().unwrap_or(source.len());
                let mut end = raw_end;
                if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\n') {
                    end -= 1;
                    if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
                        end -= 1;
                    }
                }
                let segment_start = span.range.start.max(start);
                let segment_end = span.range.end.min(end);
                if segment_start < segment_end {
                    lines[line_ix].push(HighlightSpan {
                        range: segment_start - start..segment_end - start,
                        kind: span.kind,
                    });
                }
                if raw_end >= span.range.end {
                    break;
                }
            }
        }
        for line in &mut lines {
            *line = normalize_line(std::mem::take(line));
        }
        Ok(Self { language, lines })
    }
}

fn validate_span(source: &str, range: &Range<usize>) -> Result<(), HighlightError> {
    if range.start > range.end || range.end > source.len() {
        return Err(HighlightError::InvalidRange {
            start: range.start,
            end: range.end,
            len: source.len(),
        });
    }
    if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
        return Err(HighlightError::InvalidUtf8Boundary {
            start: range.start,
            end: range.end,
        });
    }
    Ok(())
}

/// Flatten overlapping spans on one line into a sorted, non-overlapping run,
/// resolving each byte to the highest-precedence kind covering it.
fn normalize_line(spans: Vec<HighlightSpan>) -> Vec<HighlightSpan> {
    #[derive(Clone, Copy)]
    enum Edge {
        Start(usize),
        End(usize),
    }

    let mut edges = spans
        .iter()
        .enumerate()
        .flat_map(|(index, span)| {
            [
                (span.range.start, Edge::Start(index)),
                (span.range.end, Edge::End(index)),
            ]
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(offset, _)| *offset);

    // The span index is the tie-breaker, so equal-precedence overlaps let the
    // later span win — matching the parser's own nesting order.
    let mut active = BTreeSet::new();
    let mut normalized: Vec<HighlightSpan> = Vec::new();
    let mut cursor = 0;
    while cursor < edges.len() {
        let offset = edges[cursor].0;
        let group_start = cursor;
        while cursor < edges.len() && edges[cursor].0 == offset {
            if let Edge::End(index) = edges[cursor].1 {
                active.remove(&(spans[index].kind.precedence(), index));
            }
            cursor += 1;
        }
        for (_, edge) in &edges[group_start..cursor] {
            if let Edge::Start(index) = *edge {
                active.insert((spans[index].kind.precedence(), index));
            }
        }

        let Some(next_offset) = edges.get(cursor).map(|(next, _)| *next) else {
            break;
        };
        if offset == next_offset {
            continue;
        }
        if let Some((_, index)) = active.last().copied() {
            let kind = spans[index].kind;
            let merged = match normalized.last_mut() {
                Some(previous) if previous.kind == kind && previous.range.end == offset => {
                    previous.range.end = next_offset;
                    true
                }
                _ => false,
            };
            if !merged {
                normalized.push(HighlightSpan {
                    range: offset..next_offset,
                    kind,
                });
            }
        }
    }
    normalized
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .match_indices('\n')
            .map(|(index, _)| index + 1)
            .filter(|start| *start < source.len()),
    );
    starts
}

/// Highlight a document with the default limits.
pub fn highlight(request: HighlightRequest<'_>) -> Result<HighlightedDocument, HighlightError> {
    highlight_with_limits(request, HighlightLimits::default())
}

pub fn highlight_with_limits(
    request: HighlightRequest<'_>,
    limits: HighlightLimits,
) -> Result<HighlightedDocument, HighlightError> {
    if request.source.len() > limits.max_source_bytes {
        return Err(HighlightError::SourceTooLarge);
    }
    let language = detect_language(
        request.path,
        request.fence_tag,
        request.source.lines().next(),
    )
    .ok_or(HighlightError::UnknownLanguage)?;

    let mut primary = configuration(language)?;
    primary.configure(CAPTURE_NAMES);
    // Only HTML and Markdown host other languages; building 15 extra grammar
    // configurations for every Rust snippet would dwarf the parse itself.
    let injected = if matches!(language, LanguageId::Html | LanguageId::Markdown) {
        injected_languages(language)
            .into_iter()
            .filter_map(|language| {
                let mut config = configuration(language).ok()?;
                config.configure(CAPTURE_NAMES);
                Some((language, config))
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(&primary, request.source.as_bytes(), None, |name| {
            let language = language_for_alias(name)?;
            injected
                .iter()
                .find(|(candidate, _)| *candidate == language)
                .map(|(_, config)| config)
        })
        .map_err(|error| HighlightError::Parser(error.to_string()))?;

    let mut active = Vec::new();
    let mut spans = Vec::new();
    for event in events {
        match event.map_err(|error| HighlightError::Parser(error.to_string()))? {
            HighlightEvent::HighlightStart(highlight) => active.push(CAPTURE_KINDS[highlight.0]),
            HighlightEvent::HighlightEnd => {
                active.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(kind) = active.iter().copied().max_by_key(|kind| kind.precedence()) {
                    spans.push(HighlightSpan {
                        range: start..end,
                        kind,
                    });
                    if spans.len() > limits.max_spans {
                        return Err(HighlightError::TooManySpans);
                    }
                }
            }
        }
    }
    HighlightedDocument::from_absolute_spans(language, request.source, spans)
}

fn injected_languages(parent: LanguageId) -> Vec<LanguageId> {
    use LanguageId::*;
    match parent {
        Html => vec![JavaScript, Css, Json],
        Markdown => vec![
            Rust, JavaScript, Jsx, TypeScript, Tsx, Python, Go, Json, Bash, Toml, Html, Css, Yaml,
            C,
        ],
        _ => Vec::new(),
    }
}

fn make_configuration(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
    locals: &str,
) -> Result<HighlightConfiguration, HighlightError> {
    HighlightConfiguration::new(language, name, highlights, injections, locals)
        .map_err(|error| HighlightError::Parser(error.to_string()))
}

fn rust_configuration() -> Result<HighlightConfiguration, HighlightError> {
    // The upstream Rust query groups numbers and booleans under
    // `constant.builtin`. Splitting them keeps a number amber and a bool its
    // own colour, matching every other language in the palette.
    let highlights = tree_sitter_rust::HIGHLIGHTS_QUERY
        .replace(
            "(boolean_literal) @constant.builtin",
            "(boolean_literal) @boolean",
        )
        .replace(
            "(integer_literal) @constant.builtin",
            "(integer_literal) @number",
        )
        .replace(
            "(float_literal) @constant.builtin",
            "(float_literal) @number",
        );
    make_configuration(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        &highlights,
        tree_sitter_rust::INJECTIONS_QUERY,
        "",
    )
}

fn configuration(language: LanguageId) -> Result<HighlightConfiguration, HighlightError> {
    use LanguageId::*;
    match language {
        Rust => rust_configuration(),
        JavaScript => make_configuration(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        Jsx => make_configuration(
            tree_sitter_javascript::LANGUAGE.into(),
            "jsx",
            &format!(
                "{}\n{}",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::JSX_HIGHLIGHT_QUERY
            ),
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        // `tree_sitter_typescript::HIGHLIGHTS_QUERY` holds only the
        // TypeScript-specific captures — types, interfaces, enums. On its own it
        // colours a type annotation and leaves every keyword and string plain.
        // The JavaScript query has to be prepended. Comet ships the TS query
        // alone and its TypeScript blocks are correspondingly bare.
        TypeScript => make_configuration(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            &format!(
                "{}\n{}",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_typescript::HIGHLIGHTS_QUERY
            ),
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        Tsx => make_configuration(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            &format!(
                "{}\n{}\n{}",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                tree_sitter_typescript::HIGHLIGHTS_QUERY
            ),
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        Python => make_configuration(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Go => make_configuration(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Json | Jsonc => make_configuration(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Bash => make_configuration(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Toml => make_configuration(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Markdown => make_configuration(
            tree_sitter_md::LANGUAGE.into(),
            "markdown",
            tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            tree_sitter_md::INJECTION_QUERY_BLOCK,
            "",
        ),
        Html => make_configuration(
            tree_sitter_html::LANGUAGE.into(),
            "html",
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
            "",
        ),
        Css => make_configuration(
            tree_sitter_css::LANGUAGE.into(),
            "css",
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Yaml => make_configuration(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        C => make_configuration(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
        ),
    }
}

/// Ordered generic to specific. `HighlightConfiguration::configure` resolves a
/// dotted capture such as `function.method` to the best entry in this table, so
/// the order is load-bearing: `function` must precede `function.builtin`.
const CAPTURE_NAMES: &[&str] = &[
    "comment",
    "keyword",
    "string",
    "string.special",
    "string.escape",
    "number",
    "boolean",
    "type",
    "type.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "property",
    "constant",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "label",
    "embedded",
    "error",
];

const CAPTURE_KINDS: &[HighlightKind] = &[
    HighlightKind::Comment,
    HighlightKind::Keyword,
    HighlightKind::String,
    HighlightKind::StringSpecial,
    HighlightKind::Escape,
    HighlightKind::Number,
    HighlightKind::Boolean,
    HighlightKind::Type,
    HighlightKind::TypeBuiltin,
    HighlightKind::Constructor,
    HighlightKind::Function,
    HighlightKind::FunctionBuiltin,
    HighlightKind::Macro,
    HighlightKind::Property,
    HighlightKind::Constant,
    HighlightKind::Variable,
    HighlightKind::VariableSpecial,
    HighlightKind::Parameter,
    HighlightKind::Operator,
    HighlightKind::Punctuation,
    HighlightKind::Tag,
    HighlightKind::Attribute,
    HighlightKind::Label,
    HighlightKind::Embedded,
    HighlightKind::Invalid,
];

/// Fence tag beats path, path beats shebang.
pub fn detect_language(
    path: Option<&str>,
    fence_tag: Option<&str>,
    first_line: Option<&str>,
) -> Option<LanguageId> {
    fence_tag
        .and_then(language_for_alias)
        .or_else(|| path.and_then(language_for_path))
        .or_else(|| first_line.and_then(language_for_shebang))
}

pub fn language_for_alias(alias: &str) -> Option<LanguageId> {
    let alias = alias
        .trim()
        .split_ascii_whitespace()
        .next()?
        .to_ascii_lowercase();
    Some(match alias.as_str() {
        "rust" | "rs" => LanguageId::Rust,
        "javascript" | "js" | "mjs" | "cjs" => LanguageId::JavaScript,
        "jsx" => LanguageId::Jsx,
        "typescript" | "ts" | "mts" | "cts" => LanguageId::TypeScript,
        "tsx" => LanguageId::Tsx,
        "python" | "py" | "python3" => LanguageId::Python,
        "go" | "golang" => LanguageId::Go,
        "json" => LanguageId::Json,
        "jsonc" => LanguageId::Jsonc,
        "bash" | "sh" | "shell" | "zsh" | "console" => LanguageId::Bash,
        "toml" => LanguageId::Toml,
        "markdown" | "md" => LanguageId::Markdown,
        "html" | "htm" => LanguageId::Html,
        "css" => LanguageId::Css,
        "yaml" | "yml" => LanguageId::Yaml,
        "c" | "h" => LanguageId::C,
        _ => return None,
    })
}

pub fn language_for_path(path: &str) -> Option<LanguageId> {
    let path = Path::new(path);
    let name = path.file_name()?.to_str()?;
    match name.to_ascii_lowercase().as_str() {
        "cargo.lock" | "cargo.toml" | "pyproject.toml" => return Some(LanguageId::Toml),
        _ => {}
    }
    language_for_alias(path.extension()?.to_str()?)
}

fn language_for_shebang(line: &str) -> Option<LanguageId> {
    let line = line.strip_prefix("#!")?.to_ascii_lowercase();
    if line.contains("python") {
        Some(LanguageId::Python)
    } else if line.contains("node") {
        Some(LanguageId::JavaScript)
    } else if ["bash", "zsh", "/sh", " sh"]
        .iter()
        .any(|name| line.contains(name))
    {
        Some(LanguageId::Bash)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_keep_language_variants_distinct() {
        let cases = [
            ("js", LanguageId::JavaScript),
            ("jsx", LanguageId::Jsx),
            ("ts", LanguageId::TypeScript),
            ("tsx", LanguageId::Tsx),
            ("RS", LanguageId::Rust),
            ("shell", LanguageId::Bash),
        ];
        for (alias, expected) in cases {
            assert_eq!(language_for_alias(alias), Some(expected), "{alias}");
        }
        assert_eq!(language_for_alias("unknown-lang"), None);
    }

    #[test]
    fn paths_and_exact_names_resolve() {
        let cases = [
            ("src/main.rs", LanguageId::Rust),
            ("web/app.tsx", LanguageId::Tsx),
            ("Cargo.toml", LanguageId::Toml),
            ("config.jsonc", LanguageId::Jsonc),
        ];
        for (path, expected) in cases {
            assert_eq!(language_for_path(path), Some(expected), "{path}");
        }
        assert_eq!(language_for_path("README"), None);
        assert_eq!(language_for_path("image.png"), None);
    }

    #[test]
    fn fence_tag_beats_path() {
        assert_eq!(
            detect_language(Some("a.rs"), Some("python"), None),
            Some(LanguageId::Python)
        );
    }

    #[test]
    fn shebang_is_the_last_resort() {
        assert_eq!(
            detect_language(None, None, Some("#!/usr/bin/env python3")),
            Some(LanguageId::Python)
        );
        assert_eq!(detect_language(None, None, Some("let x = 1")), None);
    }

    #[test]
    fn spans_are_line_relative_sorted_and_non_overlapping() {
        let source = "let café = \"x\";\nnext";
        let document = highlight(HighlightRequest {
            source,
            path: None,
            fence_tag: Some("rust"),
        })
        .unwrap();
        assert_eq!(document.lines.len(), 2);
        for line in &document.lines {
            let mut previous_end = 0;
            for span in line {
                assert!(span.range.start >= previous_end, "spans must not overlap");
                previous_end = span.range.end;
            }
        }
        // Line 0 is 16 bytes ("café" is 5), so nothing may reach past it and
        // in particular no span may swallow the newline.
        assert!(document.lines[0].iter().all(|s| s.range.end <= 16));
    }

    #[test]
    fn rust_numbers_and_booleans_are_their_own_kinds() {
        let document = highlight(HighlightRequest {
            source: "let a = 42; let b = true;",
            path: Some("x.rs"),
            fence_tag: None,
        })
        .unwrap();
        let kinds: Vec<_> = document.lines[0].iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&HighlightKind::Number), "{kinds:?}");
        assert!(kinds.contains(&HighlightKind::Boolean), "{kinds:?}");
        assert!(kinds.contains(&HighlightKind::Keyword), "{kinds:?}");
    }

    #[test]
    fn typescript_highlights_keywords_and_strings() {
        let document = highlight(HighlightRequest {
            source: "const greeting: string = \"hi\"",
            path: None,
            fence_tag: Some("ts"),
        })
        .unwrap();
        let kinds: Vec<_> = document.lines[0].iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&HighlightKind::Keyword), "{kinds:?}");
        assert!(kinds.contains(&HighlightKind::String), "{kinds:?}");
    }

    #[test]
    fn unknown_language_is_an_error_not_a_panic() {
        let result = highlight(HighlightRequest {
            source: "hello",
            path: Some("notes.txt"),
            fence_tag: None,
        });
        assert_eq!(result, Err(HighlightError::UnknownLanguage));
    }

    #[test]
    fn oversized_sources_are_rejected() {
        let big = "a".repeat(64);
        let result = highlight_with_limits(
            HighlightRequest {
                source: &big,
                path: Some("a.rs"),
                fence_tag: None,
            },
            HighlightLimits {
                max_source_bytes: 8,
                max_spans: 10,
            },
        );
        assert_eq!(result, Err(HighlightError::SourceTooLarge));
    }

    #[test]
    fn crlf_line_endings_do_not_leak_into_spans() {
        let document = highlight(HighlightRequest {
            source: "// one\r\n// two\r\n",
            path: Some("a.rs"),
            fence_tag: None,
        })
        .unwrap();
        assert_eq!(document.lines[0][0].range, 0..6);
        assert_eq!(document.lines[1][0].range, 0..6);
    }

    #[test]
    fn empty_source_yields_one_empty_line() {
        let document = highlight(HighlightRequest {
            source: "",
            path: Some("a.rs"),
            fence_tag: None,
        })
        .unwrap();
        assert_eq!(document.lines.len(), 1);
        assert!(document.lines[0].is_empty());
    }
}
