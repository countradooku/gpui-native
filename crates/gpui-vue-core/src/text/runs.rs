//! Building `gpui::TextRun` lists.
//!
//! The invariant that makes late-arriving syntax highlighting free: every run
//! for a line uses the **same font**, only the colour differs. Recolouring can
//! then never change layout, so a highlight result can land a frame later
//! without reflowing anything.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//! Original: `runs_for_syntax_line_with_plain` in `crates/ui/src/markdown/render.rs`.

use gpui::{Font, Hsla, TextRun};

/// A single run covering the whole string.
#[allow(dead_code)]
pub fn plain_runs(text: &str, font: &Font, color: Hsla) -> Vec<TextRun> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![TextRun {
        len: text.len(),
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }]
}

/// Build the exact-cover run list for one line from its highlight spans.
///
/// `spans` must be sorted and non-overlapping, with byte ranges relative to
/// `line`. Gaps become `plain_color`. The returned runs sum to `line.len()`.
pub fn runs_for_spans(
    line: &str,
    spans: &[(std::ops::Range<usize>, Hsla)],
    font: &Font,
    plain_color: Hsla,
) -> Vec<TextRun> {
    let plain = |len: usize, color: Hsla| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::with_capacity(spans.len() * 2 + 1);
    let mut at = 0usize;
    for (range, color) in spans {
        // Defensive: a stale highlight for a shorter line must not slice past
        // the end or emit an inverted run.
        let start = range.start.min(line.len()).max(at);
        let end = range.end.min(line.len());
        if end <= start {
            continue;
        }
        if start > at {
            runs.push(plain(start - at, plain_color));
        }
        runs.push(plain(end - start, *color));
        at = end;
    }
    if at < line.len() {
        runs.push(plain(line.len() - at, plain_color));
    }
    runs.retain(|run| run.len > 0);
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{font, hsla};

    fn mono() -> Font {
        font("Menlo")
    }

    fn red() -> Hsla {
        hsla(0.0, 1.0, 0.5, 1.0)
    }

    fn white() -> Hsla {
        hsla(0.0, 0.0, 1.0, 1.0)
    }

    #[test]
    fn runs_cover_the_line_exactly() {
        let line = "let x = 1;";
        let runs = runs_for_spans(line, &[(0..3, red()), (8..9, red())], &mono(), white());
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), line.len());
        assert!(runs.iter().all(|r| r.font == mono()));
        assert_eq!(runs[0].color, red());
        assert_eq!(runs[1].color, white());
    }

    #[test]
    fn no_spans_is_one_plain_run() {
        let runs = runs_for_spans("plain text", &[], &mono(), white());
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].len, 10);
    }

    #[test]
    fn stale_spans_past_the_end_are_clipped() {
        let line = "ab";
        let runs = runs_for_spans(line, &[(0..99, red())], &mono(), white());
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), line.len());
    }

    #[test]
    fn overlapping_spans_do_not_double_count() {
        let line = "abcdef";
        let runs = runs_for_spans(line, &[(0..4, red()), (2..6, red())], &mono(), white());
        assert_eq!(runs.iter().map(|r| r.len).sum::<usize>(), line.len());
    }

    #[test]
    fn empty_text_has_no_runs() {
        assert!(plain_runs("", &mono(), white()).is_empty());
    }
}
