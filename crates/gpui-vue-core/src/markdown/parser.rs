//! Block-level markdown parsing over pulldown-cmark.
//!
//! Ported from Comet (<https://github.com/zeronsh/comet>), MIT.
//! Original: `crates/ui/src/markdown/parser.rs`.
//!
//! pulldown-cmark emits a flat event stream. This turns it into a block tree
//! with pre-flattened inline runs, because the renderer needs one string plus a
//! run list per paragraph to hand to `gpui::StyledText`, not a nested AST.
//!
//! No incremental parsing here. Comet needs it because it streams LLM output
//! token by token; gpui-vue renders whatever Vue hands it, and a full reparse of
//! a document is cheap next to laying it out.

use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

/// Bound recursive AST construction and rendering for attacker-controlled
/// markdown. Content beyond this depth is preserved as plain text.
const MAX_NESTING_DEPTH: usize = 128;

/// Inline styling flags, threaded through nested emphasis and links.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "markdown inline marks are independent composable formatting flags"
)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strikethrough: bool,
    /// Destination URL when inside a link.
    pub link: Option<String>,
}

/// One run of identically-styled inline text.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineRun {
    pub text: String,
    pub style: InlineStyle,
}

/// A markdown block. Containers nest.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Block {
    Paragraph {
        runs: Vec<InlineRun>,
    },
    Heading {
        level: u8,
        runs: Vec<InlineRun>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    BlockQuote {
        children: Vec<Block>,
    },
    List {
        ordered_start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Table {
        header: Vec<Vec<InlineRun>>,
        rows: Vec<Vec<Vec<InlineRun>>>,
        /// Per-column GFM alignment. Unspecified renders as Left.
        align: Vec<TableAlign>,
    },
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// The parse result: top-level blocks in document order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlockTree {
    pub blocks: Vec<Block>,
}

impl BlockTree {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Parse a whole source into a [`BlockTree`].
pub fn parse(source: &str) -> BlockTree {
    let events: Vec<(Event, Range<usize>)> = Parser::new_ext(source, options())
        .into_offset_iter()
        .collect();
    let mut cur = Cursor {
        events: &events,
        ix: 0,
    };
    let mut blocks = Vec::new();
    while let Some((event, _)) = cur.peek() {
        match event {
            Event::Rule => {
                cur.bump();
                blocks.push(Block::Rule);
            }
            Event::Start(_) => blocks.extend(parse_started_block(&mut cur, 0)),
            // Stray inline events at the top level should not happen; skip.
            _ => cur.bump(),
        }
    }
    BlockTree { blocks }
}

struct Cursor<'a, 'e> {
    events: &'a [(Event<'e>, Range<usize>)],
    ix: usize,
}

impl<'e> Cursor<'_, 'e> {
    fn peek(&self) -> Option<&(Event<'e>, Range<usize>)> {
        self.events.get(self.ix)
    }

    fn peek_event(&self) -> Option<&Event<'e>> {
        self.peek().map(|(e, _)| e)
    }

    fn bump(&mut self) {
        self.ix += 1;
    }

    fn next_event(&mut self) -> Option<Event<'e>> {
        let event = self.events.get(self.ix).map(|(e, _)| e.clone());
        if event.is_some() {
            self.ix += 1;
        }
        event
    }
}

fn is_block_tag(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::CodeBlock(_)
            | Tag::BlockQuote(_)
            | Tag::List(_)
            | Tag::Item
            | Tag::Table(_)
            | Tag::HtmlBlock
            | Tag::FootnoteDefinition(_)
    )
}

/// Consume a `Start(tag)` and everything through its matching `End`.
/// Unknown containers are transparent: their children splice in.
fn parse_started_block(cur: &mut Cursor, depth: usize) -> Vec<Block> {
    let Some(Event::Start(tag)) = cur.next_event() else {
        return Vec::new();
    };
    if depth >= MAX_NESTING_DEPTH && is_block_tag(&tag) {
        return plain_blocks_from_container(cur);
    }
    match tag {
        Tag::Paragraph => vec![Block::Paragraph {
            runs: parse_inline_container(cur, &InlineStyle::default(), depth),
        }],
        Tag::Heading { level, .. } => vec![Block::Heading {
            level: heading_level(level),
            runs: parse_inline_container(cur, &InlineStyle::default(), depth),
        }],
        Tag::CodeBlock(kind) => {
            let language = match kind {
                CodeBlockKind::Fenced(info) => {
                    let lang = info.split_whitespace().next().unwrap_or("");
                    (!lang.is_empty()).then(|| lang.to_string())
                }
                CodeBlockKind::Indented => None,
            };
            let mut code = String::new();
            loop {
                match cur.next_event() {
                    Some(Event::Text(t)) => code.push_str(&t),
                    Some(Event::End(_)) | None => break,
                    Some(_) => {}
                }
            }
            // Fenced blocks carry a trailing newline. Keeping it would render
            // a phantom final line, and the block's height is line-exact.
            if code.ends_with('\n') {
                code.pop();
            }
            vec![Block::CodeBlock { language, code }]
        }
        Tag::BlockQuote(_) => vec![Block::BlockQuote {
            children: parse_block_sequence(cur, depth + 1),
        }],
        Tag::List(ordered_start) => {
            let mut items = Vec::new();
            loop {
                match cur.peek_event() {
                    Some(Event::Start(Tag::Item)) => {
                        cur.bump();
                        items.push(parse_block_sequence(cur, depth + 1));
                    }
                    Some(Event::End(_)) | None => {
                        cur.bump();
                        break;
                    }
                    Some(_) => cur.bump(),
                }
            }
            vec![Block::List {
                ordered_start,
                items,
            }]
        }
        Tag::Table(align) => {
            let align = align
                .iter()
                .map(|a| match a {
                    Alignment::Center => TableAlign::Center,
                    Alignment::Right => TableAlign::Right,
                    Alignment::None | Alignment::Left => TableAlign::Left,
                })
                .collect();
            vec![parse_table(cur, align)]
        }
        Tag::HtmlBlock => {
            // Raw HTML renders as plain text. Rendering it for real would mean
            // an HTML engine, and swallowing it silently loses content.
            let mut text = String::new();
            loop {
                match cur.next_event() {
                    Some(Event::Html(t)) | Some(Event::Text(t)) => text.push_str(&t),
                    Some(Event::End(_)) | None => break,
                    Some(_) => {}
                }
            }
            let text = text.trim_end_matches('\n').to_string();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![Block::Paragraph {
                    runs: vec![InlineRun {
                        text,
                        style: InlineStyle::default(),
                    }],
                }]
            }
        }
        _ => parse_block_sequence(cur, depth + 1),
    }
}

/// Iteratively consume the container whose `Start` was just read. This is the
/// overflow path for hostile nesting, so it must not call back into the parser.
fn plain_blocks_from_container(cur: &mut Cursor) -> Vec<Block> {
    let mut nesting = 1usize;
    let mut text = String::new();
    while let Some(event) = cur.next_event() {
        match event {
            Event::Start(_) => nesting += 1,
            Event::End(_) => {
                nesting -= 1;
                if nesting == 0 {
                    break;
                }
            }
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => text.push_str(&value),
            Event::SoftBreak => text.push(' '),
            Event::HardBreak | Event::Rule => text.push('\n'),
            Event::TaskListMarker(done) => text.push_str(if done { "[x] " } else { "[ ] " }),
            Event::FootnoteReference(value) => {
                text.push('[');
                text.push_str(&value);
                text.push(']');
            }
            _ => {}
        }
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        Vec::new()
    } else {
        vec![Block::Paragraph {
            runs: vec![InlineRun {
                text,
                style: InlineStyle::default(),
            }],
        }]
    }
}

/// Parse a block sequence until the container's `End`, which is consumed.
/// Bare inline events (tight list items) accumulate into an implicit paragraph.
fn parse_block_sequence(cur: &mut Cursor, depth: usize) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut inline_acc: Vec<InlineRun> = Vec::new();
    while let Some(event) = cur.peek_event() {
        match event {
            Event::End(_) => {
                cur.bump();
                break;
            }
            Event::Start(tag) if is_block_tag(tag) => {
                flush_paragraph(&mut out, &mut inline_acc);
                out.extend(parse_started_block(cur, depth));
            }
            Event::Rule => {
                flush_paragraph(&mut out, &mut inline_acc);
                cur.bump();
                out.push(Block::Rule);
            }
            _ => parse_inline_event(cur, &mut inline_acc, &InlineStyle::default(), depth),
        }
    }
    flush_paragraph(&mut out, &mut inline_acc);
    out
}

fn flush_paragraph(out: &mut Vec<Block>, acc: &mut Vec<InlineRun>) {
    if !acc.is_empty() {
        out.push(Block::Paragraph {
            runs: autolink_runs(merge_runs(std::mem::take(acc))),
        });
    }
}

fn parse_table(cur: &mut Cursor, align: Vec<TableAlign>) -> Block {
    let mut header = Vec::new();
    let mut rows = Vec::new();
    loop {
        match cur.peek_event() {
            Some(Event::Start(Tag::TableHead)) => {
                cur.bump();
                header = parse_table_cells(cur);
            }
            Some(Event::Start(Tag::TableRow)) => {
                cur.bump();
                rows.push(parse_table_cells(cur));
            }
            Some(Event::End(_)) | None => {
                cur.bump();
                break;
            }
            Some(_) => cur.bump(),
        }
    }
    Block::Table {
        header,
        rows,
        align,
    }
}

fn parse_table_cells(cur: &mut Cursor) -> Vec<Vec<InlineRun>> {
    let mut cells = Vec::new();
    loop {
        match cur.peek_event() {
            Some(Event::Start(Tag::TableCell)) => {
                cur.bump();
                cells.push(parse_inline_container(cur, &InlineStyle::default(), 0));
            }
            Some(Event::End(_)) | None => {
                cur.bump();
                break;
            }
            Some(_) => cur.bump(),
        }
    }
    cells
}

/// Parse inline events until the container's `End`, which is consumed.
fn parse_inline_container(cur: &mut Cursor, style: &InlineStyle, depth: usize) -> Vec<InlineRun> {
    let mut runs = Vec::new();
    while let Some(event) = cur.peek_event() {
        if matches!(event, Event::End(_)) {
            cur.bump();
            break;
        }
        parse_inline_event(cur, &mut runs, style, depth);
    }
    // Autolink AFTER merging: pulldown splits `Text` events at would-be
    // emphasis characters, so `…/Foo_(bar)` arrives as three events and a
    // per-event scan would truncate the URL at every underscore.
    autolink_runs(merge_runs(runs))
}

fn parse_inline_event(
    cur: &mut Cursor,
    runs: &mut Vec<InlineRun>,
    style: &InlineStyle,
    depth: usize,
) {
    let Some(event) = cur.next_event() else {
        return;
    };
    let push = |runs: &mut Vec<InlineRun>, text: String, style: InlineStyle| {
        if !text.is_empty() {
            runs.push(InlineRun { text, style });
        }
    };
    match event {
        Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => {
            push(runs, t.into_string(), style.clone());
        }
        Event::Code(t) => {
            let mut s = style.clone();
            s.code = true;
            push(runs, t.into_string(), s);
        }
        Event::SoftBreak => push(runs, " ".into(), style.clone()),
        Event::HardBreak => push(runs, "\n".into(), style.clone()),
        Event::TaskListMarker(done) => push(
            runs,
            if done { "[x] ".into() } else { "[ ] ".into() },
            style.clone(),
        ),
        Event::FootnoteReference(t) => push(runs, format!("[{t}]"), style.clone()),
        Event::Start(tag) => {
            let mut inner = style.clone();
            match tag {
                Tag::Emphasis => inner.italic = true,
                Tag::Strong => inner.bold = true,
                Tag::Strikethrough => inner.strikethrough = true,
                Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                    inner.link = Some(dest_url.into_string());
                }
                _ => {}
            }
            if depth >= MAX_NESTING_DEPTH {
                for block in plain_blocks_from_container(cur) {
                    if let Block::Paragraph { runs: plain } = block {
                        runs.extend(plain.into_iter().map(|mut run| {
                            run.style = inner.clone();
                            run
                        }));
                    }
                }
            } else {
                runs.extend(parse_inline_container(cur, &inner, depth + 1));
            }
        }
        // `End` is consumed by the container loop; anything else is ignored.
        _ => {}
    }
}

/// Promote bare `http(s)://` URLs into link runs.
///
/// This is GFM's autolink extension, which pulldown-cmark has no option for.
/// Without it a pasted PR link renders as dead text. Runs already inside a link
/// or a code span pass through untouched, and the pass is idempotent.
fn autolink_runs(runs: Vec<InlineRun>) -> Vec<InlineRun> {
    let mut out = Vec::with_capacity(runs.len());
    for run in runs {
        if run.style.link.is_some() || run.style.code {
            out.push(run);
        } else {
            push_text_autolinked(&mut out, &run.text, &run.style);
        }
    }
    out
}

fn push_text_autolinked(runs: &mut Vec<InlineRun>, text: &str, style: &InlineStyle) {
    let push = |runs: &mut Vec<InlineRun>, text: &str, style: InlineStyle| {
        if !text.is_empty() {
            runs.push(InlineRun {
                text: text.to_string(),
                style,
            });
        }
    };
    let mut rest = text;
    while let Some(at) = find_url_start(rest) {
        let from = &rest[at..];
        let scheme = if from.starts_with("https://") {
            "https://".len()
        } else {
            "http://".len()
        };
        let len = bare_url_len(from);
        if len <= scheme {
            // A scheme with nothing after it stays text, and must not be re-found.
            push(runs, &rest[..at + scheme], style.clone());
            rest = &from[scheme..];
            continue;
        }
        push(runs, &rest[..at], style.clone());
        let mut linked = style.clone();
        linked.link = Some(from[..len].to_string());
        push(runs, &from[..len], linked);
        rest = &from[len..];
    }
    push(runs, rest, style.clone());
}

/// First viable `http(s)://`, not glued to a preceding alphanumeric, per GFM's
/// boundary rule: `foohttps://x` stays text.
fn find_url_start(text: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = text[from..].find("http") {
        let at = from + rel;
        let after = &text[at..];
        let is_scheme = after.starts_with("http://") || after.starts_with("https://");
        let boundary = text[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if is_scheme && boundary {
            return Some(at);
        }
        from = at + "http".len();
    }
    None
}

/// Byte length of the bare URL at the start of `text`: run to whitespace or a
/// delimiter that never appears in pasted URLs, then trim the trailing
/// punctuation GFM excludes. A closing paren only survives when an opener
/// inside the URL balances it, so `…/Foo_(bar))` keeps one and sheds one.
fn bare_url_len(text: &str) -> usize {
    let end = text
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`'))
        .map_or(text.len(), |(i, _)| i);
    let mut url = &text[..end];
    let (open_parens, close_parens) =
        url.bytes()
            .fold((0usize, 0usize), |counts, byte| match byte {
                b'(' => (counts.0 + 1, counts.1),
                b')' => (counts.0, counts.1 + 1),
                _ => counts,
            });
    let mut unmatched_close_parens = close_parens.saturating_sub(open_parens);
    while let Some(last) = url.chars().next_back() {
        let trim = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '*' | '_' | '~' => true,
            ')' if unmatched_close_parens > 0 => {
                unmatched_close_parens -= 1;
                true
            }
            _ => false,
        };
        if !trim {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    url.len()
}

/// Merge adjacent identically-styled runs. Keeps run counts small and makes the
/// tree canonical, which is what lets equality tests be readable.
fn merge_runs(runs: Vec<InlineRun>) -> Vec<InlineRun> {
    let mut out: Vec<InlineRun> = Vec::with_capacity(runs.len());
    for run in runs {
        match out.last_mut() {
            Some(last) if last.style == run.style => last.text.push_str(&run.text),
            _ => out.push(run),
        }
    }
    out
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> InlineRun {
        InlineRun {
            text: text.into(),
            style: InlineStyle::default(),
        }
    }

    fn flat(runs: &[InlineRun]) -> String {
        runs.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn parses_headings_at_every_level() {
        let tree = parse("# One\n\n## Two\n\n###### Six");
        assert_eq!(tree.blocks.len(), 3);
        match &tree.blocks[0] {
            Block::Heading { level, runs } => {
                assert_eq!(*level, 1);
                assert_eq!(runs, &vec![plain("One")]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(tree.blocks[2], Block::Heading { level: 6, .. }));
    }

    #[test]
    fn parses_inline_emphasis_and_code() {
        let tree = parse("**bold** *em* `code` ~~gone~~");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        assert!(runs.iter().any(|r| r.style.bold && r.text == "bold"));
        assert!(runs.iter().any(|r| r.style.italic && r.text == "em"));
        assert!(runs.iter().any(|r| r.style.code && r.text == "code"));
        assert!(
            runs.iter()
                .any(|r| r.style.strikethrough && r.text == "gone")
        );
    }

    #[test]
    fn parses_links_and_keeps_their_text() {
        let tree = parse("see [docs](https://example.com/x) now");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        let link = runs.iter().find(|r| r.style.link.is_some()).unwrap();
        assert_eq!(link.text, "docs");
        assert_eq!(link.style.link.as_deref(), Some("https://example.com/x"));
        assert_eq!(flat(runs), "see docs now");
    }

    #[test]
    fn autolinks_bare_urls() {
        let tree = parse("go to https://github.com/remorses/gpuix now");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        let link = runs.iter().find(|r| r.style.link.is_some()).unwrap();
        assert_eq!(link.text, "https://github.com/remorses/gpuix");
    }

    #[test]
    fn autolink_trims_trailing_punctuation_but_balances_parens() {
        let tree = parse("see https://x.dev/a_(b), ok");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        let link = runs.iter().find(|r| r.style.link.is_some()).unwrap();
        assert_eq!(link.text, "https://x.dev/a_(b)");
    }

    #[test]
    fn autolink_trims_many_unmatched_parens_in_one_pass() {
        let url = format!("https://x.dev/{}", ")".repeat(40_000));
        assert_eq!(bare_url_len(&url), "https://x.dev/".len());
    }

    #[test]
    fn does_not_autolink_glued_schemes() {
        let tree = parse("foohttps://x.dev bar");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        assert!(runs.iter().all(|r| r.style.link.is_none()));
    }

    #[test]
    fn parses_fenced_code_with_a_language_and_no_trailing_newline() {
        let tree = parse("```ts\nconst a = 1\nconst b = 2\n```");
        match &tree.blocks[0] {
            Block::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("ts"));
                assert_eq!(code, "const a = 1\nconst b = 2");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_unordered_and_ordered_lists() {
        let tree = parse("- a\n- b\n\n3. x\n4. y");
        match &tree.blocks[0] {
            Block::List {
                ordered_start,
                items,
            } => {
                assert_eq!(*ordered_start, None);
                assert_eq!(items.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        match &tree.blocks[1] {
            Block::List { ordered_start, .. } => assert_eq!(*ordered_start, Some(3)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_nested_lists() {
        let tree = parse("- outer\n  - inner");
        let Block::List { items, .. } = &tree.blocks[0] else {
            panic!("expected a list");
        };
        assert!(
            items[0]
                .iter()
                .any(|block| matches!(block, Block::List { .. }))
        );
    }

    #[test]
    fn parses_block_quotes_with_nested_blocks() {
        let tree = parse("> quoted\n>\n> - item");
        let Block::BlockQuote { children } = &tree.blocks[0] else {
            panic!("expected a block quote");
        };
        assert!(matches!(children[0], Block::Paragraph { .. }));
        assert!(matches!(children[1], Block::List { .. }));
    }

    #[test]
    fn hostile_block_nesting_is_capped_without_losing_text() {
        let source = format!("{} x", ">".repeat(20_000));
        let tree = parse(&source);
        let mut pending: Vec<&Block> = tree.blocks.iter().collect();
        let mut blocks_seen = 0usize;
        let mut found_text = false;
        while let Some(block) = pending.pop() {
            blocks_seen += 1;
            assert!(
                blocks_seen <= MAX_NESTING_DEPTH + 2,
                "the AST exceeded its nesting cap"
            );
            match block {
                Block::BlockQuote { children } => pending.extend(children),
                Block::List { items, .. } => {
                    pending.extend(items.iter().flat_map(|item| item.iter()));
                }
                Block::Paragraph { runs } => {
                    found_text |= runs.iter().any(|run| run.text.contains('x'));
                }
                _ => {}
            }
        }
        assert!(found_text, "overflow nesting must preserve visible content");
    }

    #[test]
    fn parses_tables_with_alignment() {
        let tree = parse("| a | b |\n|:--|--:|\n| 1 | 2 |");
        match &tree.blocks[0] {
            Block::Table {
                header,
                rows,
                align,
            } => {
                assert_eq!(flat(&header[0]), "a");
                assert_eq!(flat(&rows[0][1]), "2");
                assert_eq!(align, &vec![TableAlign::Left, TableAlign::Right]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_horizontal_rules() {
        let tree = parse("a\n\n---\n\nb");
        assert!(matches!(tree.blocks[1], Block::Rule));
    }

    #[test]
    fn task_list_markers_become_literal_text() {
        let tree = parse("- [x] done\n- [ ] todo");
        let Block::List { items, .. } = &tree.blocks[0] else {
            panic!("expected a list");
        };
        let Block::Paragraph { runs } = &items[0][0] else {
            panic!("expected a paragraph");
        };
        assert_eq!(flat(runs), "[x] done");
    }

    #[test]
    fn raw_html_renders_as_text_instead_of_vanishing() {
        let tree = parse("<div>hi</div>");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        assert!(flat(runs).contains("<div>hi</div>"));
    }

    #[test]
    fn soft_breaks_become_spaces_and_hard_breaks_newlines() {
        let tree = parse("one\ntwo");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        assert_eq!(flat(runs), "one two");

        let tree = parse("one  \ntwo");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        assert_eq!(flat(runs), "one\ntwo");
    }

    #[test]
    fn adjacent_runs_of_the_same_style_merge() {
        let tree = parse("plain **a** **b**");
        let Block::Paragraph { runs } = &tree.blocks[0] else {
            panic!("expected a paragraph");
        };
        // "a" and "b" are separated by a plain space, so three runs, not five.
        assert_eq!(runs.len(), 4, "{runs:?}");
    }

    #[test]
    fn an_empty_document_has_no_blocks() {
        assert!(parse("").is_empty());
        assert!(parse("   \n\n  ").is_empty());
    }

    #[test]
    fn unclosed_fences_still_produce_a_code_block() {
        let tree = parse("```rust\nfn main() {}");
        assert!(matches!(tree.blocks[0], Block::CodeBlock { .. }));
    }
}
