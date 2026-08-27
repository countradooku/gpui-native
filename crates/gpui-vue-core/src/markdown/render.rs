//! `BlockTree` to gpui elements.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//! Original: `crates/ui/src/markdown/render.rs`.
//!
//! Numbers drive layout (font sizes, line heights, paddings are all constants
//! here); colours are paint. Code blocks render per line so their height is
//! exactly `lines × line_height`, and syntax highlighting arrives as recoloured
//! `TextRun`s on the identical font, so layout never changes.

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    div, font, px, AnyElement, BorderStyle, FontStyle, FontWeight, Hsla, SharedString, TextRun,
    UnderlineStyle, Window,
};

use super::parser::{Block, BlockTree, InlineRun, TableAlign};
use crate::syntax::cache::highlight_cached;
use crate::text::{range_rects, runs::runs_for_spans, SharedSelection};
use crate::theme::{Metrics, Theme};

// ── Metrics ──────────────────────────────────────────────────────────
//
// Everything that decides layout lives in `Theme::metrics`, so a design tweak
// is a Vue prop change and not a native rebuild. Only the hairline and the
// inline-code wash overhang stay fixed: they are paint geometry, and neither
// can move a glyph.

/// Hairline between table rows.
const TABLE_DIVIDER: f32 = 1.0;

/// Per-column geometry resolved from measured max-content widths.
pub struct TableColumns {
    /// Max-content width per column, padding included.
    pub naturals: Vec<f32>,
    /// `min(natural, TABLE_MIN_COLUMN_WIDTH)` per column.
    pub minimums: Vec<f32>,
    /// Sum of the minimums: the width below which the table stops shrinking.
    pub min_table_width: f32,
}

/// Resolve column geometry from measured content widths (padding added here).
pub fn table_columns(content_widths: &[f32], m: &Metrics) -> TableColumns {
    let naturals: Vec<f32> = content_widths
        .iter()
        .map(|w| w.max(m.md_table_min_column_content) + 2.0 * m.md_table_cell_padding)
        .collect();
    let minimums: Vec<f32> = naturals
        .iter()
        .map(|n| n.min(m.md_table_min_column_width))
        .collect();
    let min_table_width = minimums.iter().sum();
    TableColumns {
        naturals,
        minimums,
        min_table_width,
    }
}
/// Inline-code wash overhang: x extends past the glyphs, y insets from the line
/// box. Paint only, so neither can move a glyph.
const INLINE_CODE_PAD_X: f32 = 2.0;
const INLINE_CODE_INSET_Y: f32 = 2.0;

// ── Flattened inline text ────────────────────────────────────────────

/// Inline runs flattened into one string plus gpui `TextRun`s, with the byte
/// ranges of clickable links and inline-code spans.
///
/// Inline code needs its own ranges because `TextRun::background_color` can only
/// paint a square box; the rounded pill is a canvas underlay.
pub struct FlatText {
    pub text: SharedString,
    pub runs: Vec<TextRun>,
    pub links: Vec<(Range<usize>, String)>,
    pub code_ranges: Vec<Range<usize>>,
}

/// Flatten inline runs into shaped-text inputs. Pure given a theme.
pub fn flatten_runs(runs: &[InlineRun], theme: &Theme, base_weight: FontWeight) -> FlatText {
    let mut text = String::new();
    let mut out: Vec<TextRun> = Vec::with_capacity(runs.len());
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    let mut code_ranges: Vec<Range<usize>> = Vec::new();

    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        let start = text.len();
        text.push_str(&run.text);

        let mut f = if run.style.code {
            font(theme.font_mono.clone())
        } else {
            font(theme.font_sans.clone())
        };
        f.weight = if run.style.bold && base_weight.0 < FontWeight::SEMIBOLD.0 {
            FontWeight::SEMIBOLD
        } else {
            base_weight
        };
        f.style = if run.style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        };

        if run.style.code {
            // Merge adjacent code runs into one wash box, like links below.
            match code_ranges.last_mut() {
                Some(range) if range.end == start => range.end = text.len(),
                _ => code_ranges.push(start..text.len()),
            }
        }
        if let Some(url) = &run.style.link {
            match links.last_mut() {
                Some((range, last_url)) if range.end == start && last_url == url => {
                    range.end = text.len();
                }
                _ => links.push((start..text.len(), url.clone())),
            }
        }

        out.push(TextRun {
            len: run.text.len(),
            font: f,
            // Inline code reads violet; links stay the monochrome foreground
            // with an underline, because accent is reserved for actions.
            color: if run.style.code {
                theme.code_text
            } else {
                theme.text
            },
            background_color: None,
            underline: run.style.link.is_some().then_some(UnderlineStyle {
                color: Some(theme.text_muted),
                thickness: px(1.0),
                wavy: false,
            }),
            strikethrough: run.style.strikethrough.then_some(gpui::StrikethroughStyle {
                thickness: px(1.0),
                color: Some(theme.text_muted),
            }),
        });
    }

    FlatText {
        text: text.into(),
        runs: out,
        links,
        code_ranges,
    }
}

// ── Render context ───────────────────────────────────────────────────

/// Everything block rendering needs. Carries a mutable counter so each painted
/// text run gets a distinct, document-ordered selection sub-key.
pub struct MdContext {
    pub element_id: u64,
    pub selection: SharedSelection,
    pub selectable: bool,
    pub selection_wash: Hsla,
    pub theme: Theme,
    /// Inherited `highlight`, matched per painted string. See
    /// [`crate::text::search::washes_for_native_run`].
    pub highlight_set: Option<Arc<crate::text::HighlightContext>>,
    /// Monotonic sub-key counter. Must advance in document order so a drag
    /// resolves spans the same way the reader sees them.
    next_sub: usize,
    /// Called with the URL of the link under a click. Hit testing happens per
    /// byte range inside the painted text, not per block.
    pub on_link: Option<Arc<dyn Fn(&str)>>,
}

impl MdContext {
    pub fn new(
        element_id: u64,
        selection: SharedSelection,
        selectable: bool,
        selection_wash: Hsla,
        theme: Theme,
        on_link: Option<Arc<dyn Fn(&str)>>,
        highlight_set: Option<Arc<crate::text::HighlightContext>>,
    ) -> Self {
        Self {
            element_id,
            selection,
            selectable,
            selection_wash,
            theme,
            highlight_set,
            next_sub: 0,
            on_link,
        }
    }

    fn take_sub(&mut self) -> usize {
        let sub = self.next_sub;
        self.next_sub += 1;
        sub
    }
}

// ── Blocks ───────────────────────────────────────────────────────────

/// Render a whole tree stacked with the block gap.
///
/// `window` is threaded through only so tables can shape their cells to measure
/// max-content widths, the same resolution CSS performs.
pub fn render_tree(tree: &BlockTree, ctx: &mut MdContext, window: &Window) -> AnyElement {
    use gpui::prelude::*;

    let block_gap = ctx.theme.metrics.md_block_gap;
    div()
        .flex()
        .flex_col()
        .gap(px(block_gap))
        .children(
            tree.blocks
                .iter()
                .map(|block| render_block(block, ctx, window))
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

pub fn render_block(block: &Block, ctx: &mut MdContext, window: &Window) -> AnyElement {
    use gpui::prelude::*;

    let theme = ctx.theme.clone();
    let m = &theme.metrics;
    match block {
        Block::Paragraph { runs } => text_element(
            runs,
            m.md_text_size,
            m.md_line_height,
            FontWeight::NORMAL,
            ctx,
        ),
        Block::Heading { level, runs } => {
            let (size, line) = m.heading(*level);
            text_element(runs, size, line, FontWeight::SEMIBOLD, ctx)
        }
        Block::CodeBlock { language, code } => render_code_block(language.as_deref(), code, ctx),
        Block::BlockQuote { children } => div()
            // Accent-tinted quote: an indigo rail with a whisper of the same
            // hue behind it.
            .border_l_2()
            .border_color(opacity(theme.accent, 0.6))
            .bg(opacity(theme.accent, 0.05))
            .rounded_tr(px(6.0))
            .rounded_br(px(6.0))
            .pl(px(12.0))
            .pr(px(10.0))
            .py(px(6.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .text_color(theme.text_muted)
            .children(
                children
                    .iter()
                    .map(|child| render_block(child, ctx, window))
                    .collect::<Vec<_>>(),
            )
            .into_any_element(),
        Block::List {
            ordered_start,
            items,
        } => {
            let mut list = div().flex().flex_col().gap(px(4.0));
            for (item_ix, item) in items.iter().enumerate() {
                // Ordered numbers are accent-tinted text; unordered markers are
                // a real 5px disc, because the glyph "•" reads too small at 14px.
                let marker: AnyElement = match ordered_start {
                    Some(start) => div()
                        .flex_none()
                        .min_w(px(18.0))
                        .text_size(px(m.md_text_size))
                        .line_height(px(m.md_line_height))
                        .text_color(opacity(theme.accent, 0.85))
                        .child(crate::text::chrome_text(
                            SharedString::from(format!("{}.", start + item_ix as u64)),
                            None,
                        ))
                        .into_any_element(),
                    None => div()
                        .flex_none()
                        .min_w(px(18.0))
                        // Centre the disc on the first text line's cap band.
                        .h(px(m.md_line_height))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .ml(px(1.0))
                                .w(px(5.0))
                                .h(px(5.0))
                                .rounded_full()
                                .bg(opacity(theme.accent, 0.85)),
                        )
                        .into_any_element(),
                };
                let children: Vec<AnyElement> = item
                    .iter()
                    .map(|child| render_block(child, ctx, window))
                    .collect();
                list = list.child(
                    div().flex().flex_row().gap(px(8.0)).child(marker).child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .children(children),
                    ),
                );
            }
            list.into_any_element()
        }
        Block::Table {
            header,
            rows,
            align,
        } => render_table(header, rows, align, ctx, window),
        Block::Rule => div()
            .h(px(1.0))
            .w_full()
            .bg(theme.border)
            .into_any_element(),
    }
}

fn text_element(
    runs: &[InlineRun],
    size: f32,
    line_height: f32,
    weight: FontWeight,
    ctx: &mut MdContext,
) -> AnyElement {
    use gpui::prelude::*;

    let flat = flatten_runs(runs, &ctx.theme, weight);
    let inner = flat_text_element(&flat, ctx);
    div()
        .w_full()
        .min_w_0()
        .text_size(px(size))
        .line_height(px(line_height))
        .child(inner)
        .into_any_element()
}

/// Selectable text with the inline-code wash painted underneath.
fn flat_text_element(flat: &FlatText, ctx: &mut MdContext) -> AnyElement {
    let sub = ctx.take_sub();
    let code_ranges = flat.code_ranges.clone();
    let wash = ctx.theme.code_wash;
    let radius = ctx.theme.metrics.md_inline_code_radius;
    let extra: Option<Box<dyn Fn(&gpui::TextLayout, &mut gpui::Window)>> = if code_ranges.is_empty()
    {
        None
    } else {
        Some(Box::new(move |layout, window| {
            for range in &code_ranges {
                for rect in range_rects(layout, range, INLINE_CODE_PAD_X, INLINE_CODE_INSET_Y) {
                    window.paint_quad(gpui::quad(
                        rect,
                        px(radius),
                        wash,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
        }))
    };

    // Selection and link interaction are independent: a toolbar can reasonably
    // set `userSelect: "none"` and still want its links clickable, so this
    // routes through `chrome_text` for the glyphs but keeps the link listener.
    crate::text::selectable_text(crate::text::SelectableText {
        extra_wash: extra,
        links: flat.links.clone(),
        on_link: ctx.on_link.clone(),
        selectable: ctx.selectable,
        highlight: ctx
            .highlight_set
            .clone()
            .map(crate::text::HighlightSource::Native),
        ..crate::text::SelectableText::new(
            ctx.element_id,
            sub,
            flat.text.clone(),
            Some(flat.runs.clone()),
            ctx.selection.clone(),
            ctx.selection_wash,
        )
    })
}

fn render_code_block(language: Option<&str>, code: &str, ctx: &mut MdContext) -> AnyElement {
    use gpui::prelude::*;

    let theme = ctx.theme.clone();
    let m = &theme.metrics;
    let mono = font(theme.font_mono.clone());
    let highlight = highlight_cached(code, None, language);

    // overflow-x only works as a flex *row* viewport. A flex_col scroller
    // stretches each nowrap row to the card width, so the line never overflows
    // and a horizontal wheel does nothing. Same pattern as host overflowX.
    let scroll_sub = ctx.take_sub();
    let mut lines = div()
        .flex_none()
        .flex()
        .flex_col()
        .px(px(m.md_code_padding_x))
        .py(px(m.md_code_padding_y))
        .font_family(theme.font_mono.clone())
        .text_size(px(m.code_text_size))
        .line_height(px(m.code_line_height))
        .whitespace_nowrap();

    for (line_ix, line) in code.split('\n').enumerate() {
        let spans: Vec<(Range<usize>, Hsla)> = highlight
            .as_ref()
            .and_then(|doc: &Arc<_>| doc.lines.get(line_ix))
            .map(|spans| {
                spans
                    .iter()
                    .map(|span| (span.range.clone(), theme.syntax.color(span.kind)))
                    .collect()
            })
            .unwrap_or_default();
        let runs = runs_for_spans(line, &spans, &mono, theme.text);
        let sub = ctx.take_sub();
        // Content, not chrome: `userSelect: "none"` stops the drag, not the
        // find, and `chrome_text` cannot paint a highlight wash.
        let text: AnyElement = crate::text::selectable_text(crate::text::SelectableText {
            selectable: ctx.selectable,
            highlight: ctx
                .highlight_set
                .clone()
                .map(crate::text::HighlightSource::Native),
            ..crate::text::SelectableText::new(
                ctx.element_id,
                sub,
                SharedString::from(line.to_string()),
                Some(runs),
                ctx.selection.clone(),
                ctx.selection_wash,
            )
        });
        lines = lines.child(div().h(px(m.code_line_height)).flex_none().child(text));
    }

    let body = div()
        .id(SharedString::from(format!(
            "__gpui_vue_md_code_{}_{scroll_sub}",
            ctx.element_id
        )))
        .flex()
        .min_w_0()
        .overflow_x_scroll()
        .restrict_scroll_to_axis()
        .child(lines);

    let mut block = div()
        .w_full()
        .min_w_0()
        .rounded(px(m.md_code_radius))
        .bg(ink(&theme, 0.035))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden();

    if let Some(language) = language {
        block = block.child(
            div()
                .px(px(m.md_code_padding_x))
                .py(px(m.md_code_header_padding_y))
                .border_b_1()
                .border_color(theme.border)
                .bg(ink(&theme, 0.02))
                .text_size(px(m.md_code_header_text_size))
                .text_color(theme.text_muted)
                .child(crate::text::chrome_text(
                    SharedString::from(language.to_string()),
                    None,
                )),
        );
    }

    block.child(body).into_any_element()
}

/// A GFM table with the frameless "flat hairline" chrome Comet uses: 1px rules
/// under the header and between rows, no outer box, no header fill, no rounding.
///
/// Column widths resolve the way the CSS does: each cell is
/// `flex: <max-content> <max-content> 0` with `min-width: min(max-content, 96px)`,
/// so widths are content-proportional with a readable per-column floor. Naturals
/// come from shaping each cell unwrapped; gpui's line-layout cache makes repeat
/// frames cheap. When even the floors no longer fit, the rows overflow and the
/// table scrolls horizontally instead of crushing every column.
fn render_table(
    header: &[Vec<InlineRun>],
    rows: &[Vec<Vec<InlineRun>>],
    align: &[TableAlign],
    ctx: &mut MdContext,
    window: &Window,
) -> AnyElement {
    use gpui::prelude::*;

    let theme = ctx.theme.clone();
    let m = &theme.metrics;
    let all: Vec<&[Vec<InlineRun>]> = std::iter::once(header)
        .filter(|h| !h.is_empty())
        .map(|h| h as &[Vec<InlineRun>])
        .chain(rows.iter().map(|r| r.as_slice()))
        .collect();
    let cols = all.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return gpui::Empty.into_any_element();
    }
    let has_header = !header.is_empty();
    // Consume a sub-key up front: a table whose cells are all empty renders no
    // text and would otherwise share a scroll id with the next such table.
    let table_sub = ctx.take_sub();
    let hairline = hairline(&theme, 0.10);

    // Flatten every cell once and take per-column max-content widths.
    let text_system = window.text_system();
    let mut flats: Vec<Vec<Option<FlatText>>> = Vec::with_capacity(all.len());
    let mut content = vec![0.0f32; cols];
    for (row_ix, row) in all.iter().enumerate() {
        let weight = if has_header && row_ix == 0 {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        let mut out: Vec<Option<FlatText>> = Vec::with_capacity(cols);
        for (col, natural) in content.iter_mut().enumerate() {
            let Some(runs) = row.get(col) else {
                out.push(None);
                continue;
            };
            let flat = flatten_runs(runs, &theme, weight);
            if !flat.text.is_empty() {
                // Cells are single-line; guard anyway, and keep the byte count
                // identical so the runs still cover the text exactly.
                let line: SharedString = if flat.text.contains('\n') {
                    flat.text.replace('\n', " ").into()
                } else {
                    flat.text.clone()
                };
                let width = f32::from(
                    text_system
                        .shape_line(line, px(m.md_text_size), &flat.runs, None)
                        .width(),
                );
                if width > *natural {
                    *natural = width;
                }
            }
            out.push(Some(flat));
        }
        flats.push(out);
    }
    let geo = table_columns(&content, m);

    let mut inner = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w(px(geo.min_table_width));
    for (row_ix, row) in flats.iter().enumerate() {
        if row_ix > 0 {
            inner = inner.child(div().flex_none().h(px(TABLE_DIVIDER)).w_full().bg(hairline));
        }
        let mut row_el = div().flex().flex_row();
        for (col, cell_flat) in row.iter().enumerate() {
            let mut cell = div()
                .flex_grow(geo.naturals[col])
                .flex_shrink(geo.naturals[col])
                .flex_basis(px(0.0))
                .min_w(px(geo.minimums[col]))
                .p(px(m.md_table_cell_padding))
                .text_size(px(m.md_text_size))
                .line_height(px(m.md_line_height));
            cell = match align.get(col).copied().unwrap_or_default() {
                TableAlign::Left => cell,
                TableAlign::Center => cell.text_center(),
                TableAlign::Right => cell.text_right(),
            };
            if let Some(flat) = cell_flat {
                if !flat.text.is_empty() {
                    cell = cell.child(flat_text_element(flat, ctx));
                }
            }
            row_el = row_el.child(cell);
        }
        inner = inner.child(row_el);
    }

    // The horizontal scroller: when the floors exceed the viewport the inner
    // block keeps `min_table_width` and this viewport scrolls it.
    div()
        .id(SharedString::from(format!(
            "__gpui_vue_md_table_{}_{table_sub}",
            ctx.element_id
        )))
        .w_full()
        .flex()
        .min_w_0()
        .overflow_x_scroll()
        .restrict_scroll_to_axis()
        .child(inner.flex_none())
        .into_any_element()
}

// ── Colour helpers ───────────────────────────────────────────────────

fn opacity(mut color: Hsla, alpha: f32) -> Hsla {
    color.a *= alpha;
    color
}

fn ink(theme: &Theme, alpha: f32) -> Hsla {
    let lightness = if theme.bg.l < 0.5 { 1.0 } else { 0.0 };
    gpui::hsla(0.0, 0.0, lightness, alpha)
}

fn hairline(theme: &Theme, alpha: f32) -> Hsla {
    ink(theme, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parser::InlineStyle;

    fn run(text: &str, style: InlineStyle) -> InlineRun {
        InlineRun {
            text: text.into(),
            style,
        }
    }

    #[test]
    fn runs_cover_the_flattened_text_exactly() {
        let theme = Theme::dark();
        let flat = flatten_runs(
            &[
                run("go ", InlineStyle::default()),
                run(
                    "here",
                    InlineStyle {
                        link: Some("https://x.dev".into()),
                        ..Default::default()
                    },
                ),
                run(
                    " now",
                    InlineStyle {
                        bold: true,
                        ..Default::default()
                    },
                ),
            ],
            &theme,
            FontWeight::NORMAL,
        );
        assert_eq!(flat.text, "go here now");
        assert_eq!(
            flat.runs.iter().map(|r| r.len).sum::<usize>(),
            flat.text.len()
        );
        assert_eq!(flat.links, vec![(3..7, "https://x.dev".to_string())]);
        // Links stay monochrome with an underline, never accent-tinted.
        assert_eq!(flat.runs[1].color, theme.text);
        assert!(flat.runs[1].underline.is_some());
        assert_eq!(flat.runs[2].font.weight, FontWeight::SEMIBOLD);
    }

    #[test]
    fn adjacent_inline_code_merges_into_one_wash() {
        let theme = Theme::dark();
        let code = |text: &str| {
            run(
                text,
                InlineStyle {
                    code: true,
                    ..Default::default()
                },
            )
        };
        let flat = flatten_runs(
            &[
                run("use ", InlineStyle::default()),
                code("foo"),
                code("()"),
                run(" and ", InlineStyle::default()),
                code("bar"),
            ],
            &theme,
            FontWeight::NORMAL,
        );
        assert_eq!(flat.code_ranges, vec![4..9, 14..17]);
        assert_eq!(flat.runs[1].color, theme.code_text);
        // The pill is a rounded canvas quad, so the run carries no background.
        assert_eq!(flat.runs[1].background_color, None);
    }

    #[test]
    fn heading_sizes_follow_the_metrics_override() {
        let mut m = Metrics::default();
        m.md_heading_sizes = [40.0, 30.0, 20.0, 10.0];
        assert_eq!(m.heading(1).0, 40.0);
        assert_eq!(m.heading(3).0, 20.0);
        // h4 through h6 all collapse onto the last tier.
        assert_eq!(m.heading(4).0, 10.0);
        assert_eq!(m.heading(6).0, 10.0);
    }

    #[test]
    fn empty_runs_are_skipped() {
        let flat = flatten_runs(
            &[
                run("", InlineStyle::default()),
                run("x", InlineStyle::default()),
            ],
            &Theme::dark(),
            FontWeight::NORMAL,
        );
        assert_eq!(flat.runs.len(), 1);
    }

    #[test]
    fn headings_step_down_toward_body_size() {
        let m = Metrics::default();
        let (h1, _) = m.heading(1);
        let (h2, _) = m.heading(2);
        let (h6, _) = m.heading(6);
        assert!(h1 > h2 && h2 > h6);
        assert_eq!(h6, m.md_text_size);
    }
}
