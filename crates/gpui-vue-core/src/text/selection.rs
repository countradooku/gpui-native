//! Cross-element text selection state.
//!
//! Ported from Comet (<https://github.com/zeronsh/comet>), MIT.
//! Original: `crates/ui/src/markdown/selection.rs`.
//!
//! GPUI has no built-in selection for plain text. Zed's markdown selects
//! continuously because its whole document is ONE element over one text model.
//! GPUIX renders a TREE of text elements, so this module rebuilds that
//! continuity: every frame the renderer registers each painted text element in
//! paint order (which IS document order), and a drag anchored in one element
//! resolves against that registry into per-element SPANS — partial in the anchor
//! and head elements, whole for every element between. The wash paints per
//! element from its span; copy joins the spans in order.
//!
//! This is the pure state half. It has no gpui dependency so it can be unit
//! tested without a window. The registry, geometry and mouse listeners live in
//! [`super::paint`].
//!
//! Difference from Comet: the state lives in a `SelectionState` value owned by
//! `GpuiView` instead of a process-global. GPUIX is a library and a process may
//! host more than one renderer, so a global would let two windows fight over one
//! selection.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

/// One element's slice of the selection, in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// Element key — the numeric element id rendered as a string, plus a
    /// sub-index for elements that paint more than one text run.
    pub key: String,
    /// Selected byte range of the element's flat text.
    pub range: Range<usize>,
    /// The element's full flat text. Snapshotted at drag time so copy still
    /// works after the element scrolls out of the registry.
    pub text: String,
    /// See [`RegisteredText::group`].
    pub group: Option<u64>,
}

/// One painted text element as the frame registry sees it.
#[derive(Clone, Copy, Debug)]
pub struct RegisteredText<'a> {
    pub key: &'a str,
    pub text: &'a str,
    /// Id of the host element whose primitive text children this run belongs
    /// to, or `None` for a run that never merges with a neighbour.
    ///
    /// Vue makes a separate host node for every interpolated string
    /// (`shouldSetTextContent` is false), so `<text>Hello {name}!</text>` is
    /// three painted runs of one logical line. They share a group, and copy
    /// must join them with nothing. Anything else is a separate line and joins
    /// with a newline.
    pub group: Option<u64>,
}

/// Live selection for one renderer.
#[derive(Clone, Debug, Default)]
pub struct SelectionState {
    /// Element that owns the drag (where the mouse went down).
    anchor_key: String,
    /// Byte offset of the anchor within its element.
    anchor_ix: usize,
    dragging: bool,
    /// Resolved spans in document order. Empty while a click has not moved.
    spans: Vec<Span>,
    active: bool,
    /// Mouse-down hit that has not become a drag yet. A tap must not select
    /// or blur; iOS treats that tap as scroll or focus. Promoted on the first
    /// dragging move, or replaced by `begin_with_span` for double-click.
    pending: bool,
}

impl SelectionState {
    /// Remember a press without selecting. The next dragging move promotes it.
    pub fn arm(&mut self, key: &str, ix: usize) {
        self.anchor_key = key.to_string();
        self.anchor_ix = ix;
        self.dragging = false;
        self.active = false;
        self.pending = true;
        self.spans.clear();
    }

    pub fn promote_pending(&mut self) -> bool {
        if !self.pending {
            return false;
        }
        self.pending = false;
        self.dragging = true;
        self.active = true;
        true
    }

    pub fn active_drag(&self) -> Option<(String, usize)> {
        (self.active && self.dragging).then(|| (self.anchor_key.clone(), self.anchor_ix))
    }

    pub fn cancel_pending(&mut self) {
        if self.pending {
            *self = SelectionState::default();
        }
    }

    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// Begin with an immediate span — double or triple click inside one element.
    pub fn begin_with_span(&mut self, key: &str, text: &str, range: Range<usize>) {
        self.anchor_key = key.to_string();
        self.anchor_ix = range.start;
        self.dragging = true;
        self.active = true;
        self.pending = false;
        self.spans = vec![Span {
            key: key.to_string(),
            range,
            text: text.to_string(),
            group: None,
        }];
    }

    /// Replace the resolved spans. Returns true when they changed.
    pub fn update_spans(&mut self, spans: Vec<Span>) -> bool {
        if !self.active || self.spans == spans {
            return false;
        }
        self.spans = spans;
        true
    }

    /// End the drag for `key`'s claim. Returns the joined text when non-empty.
    pub fn end_drag(&mut self, key: &str) -> Option<String> {
        if !self.active || self.anchor_key != key || !self.dragging {
            return None;
        }
        self.dragging = false;
        if self.spans.iter().all(|s| s.range.is_empty()) {
            self.clear();
            return None;
        }
        Some(join_spans(&self.spans))
    }

    pub fn end_active_drag(&mut self) -> Option<String> {
        let key = self.anchor_key.clone();
        self.end_drag(&key)
    }

    pub fn clear(&mut self) {
        *self = SelectionState::default();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The wash range for `key` this frame. `None` means nothing to paint.
    pub fn wash_range(&self, key: &str) -> Option<Range<usize>> {
        if !self.active {
            return None;
        }
        self.spans
            .iter()
            .find(|s| s.key == key && !s.range.is_empty())
            .map(|s| s.range.clone())
    }

    /// The full selected text, spans joined in document order.
    pub fn selected_text(&self) -> Option<String> {
        if !self.active || self.spans.iter().all(|s| s.range.is_empty()) {
            return None;
        }
        Some(join_spans(&self.spans))
    }
}

/// Resolve the spans for a selection between `a` and `b`, each an
/// `(element index, byte offset)` into `elements` (document-ordered painted
/// runs). Handles either direction; empty slices are skipped.
pub fn resolve_spans(
    elements: &[RegisteredText],
    a: (usize, usize),
    b: (usize, usize),
) -> Vec<Span> {
    let (start, end) = if (a.0, a.1) <= (b.0, b.1) {
        (a, b)
    } else {
        (b, a)
    };
    let mut spans = Vec::new();
    for (ei, entry) in elements.iter().enumerate().take(end.0 + 1).skip(start.0) {
        let text = entry.text;
        let from = if ei == start.0 { start.1 } else { 0 };
        let to = if ei == end.0 { end.1 } else { text.len() };
        let (from, to) = (clamp_boundary(text, from), clamp_boundary(text, to));
        if from < to {
            spans.push(Span {
                key: entry.key.to_string(),
                range: from..to,
                text: text.to_string(),
                group: entry.group,
            });
        }
    }
    spans
}

/// Clamp a byte offset into `text` and snap it down to a char boundary.
/// Mouse-derived indices are already on boundaries; this is defensive so a
/// stale index from a previous frame's text can never panic on slicing.
fn clamp_boundary(text: &str, mut ix: usize) -> usize {
    ix = ix.min(text.len());
    while ix > 0 && !text.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

/// Join spans in document order, with a newline between logical lines and
/// nothing between runs of the same group.
///
/// Without the group check, copying `<text>Hello {name}!</text>` yields
/// `"Hello\nTommy\n!"`, because Vue split one line into three host nodes.
fn join_spans(spans: &[Span]) -> String {
    let mut out = String::new();
    let mut previous: Option<Option<u64>> = None;
    for span in spans.iter().filter(|s| !s.range.is_empty()) {
        if let Some(previous) = previous {
            let same_group = span.group.is_some() && span.group == previous;
            if !same_group {
                out.push('\n');
            }
        }
        out.push_str(&span.text[span.range.clone()]);
        previous = Some(span.group);
    }
    out
}

/// Word range around `ix` for double-click selection: an alphanumeric/`_` run,
/// or the single non-space char under the cursor, or empty at spaces.
pub fn word_range(text: &str, ix: usize) -> Range<usize> {
    let ix = clamp_boundary(text, ix);
    for (start, segment) in text.split_word_bound_indices() {
        let end = start + segment.len();
        if start <= ix
            && ix <= end
            && segment
                .chars()
                .any(|character| character.is_alphanumeric() || character == '_')
        {
            return start..end;
        }
    }
    text[ix..]
        .grapheme_indices(true)
        .next()
        .filter(|(_, grapheme)| !grapheme.chars().all(char::is_whitespace))
        .map(|(_, grapheme)| ix..ix + grapheme.len())
        .unwrap_or(ix..ix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg<'a>(key: &'a str, text: &'a str, group: Option<u64>) -> RegisteredText<'a> {
        RegisteredText { key, text, group }
    }

    fn elems<'a>() -> Vec<RegisteredText<'a>> {
        vec![
            reg("p1", "first paragraph", None),
            reg("p2", "second", None),
            reg("p3", "third one", None),
        ]
    }

    /// `<text>Hello {name}!</text>`: one line, three host nodes, one group.
    fn interpolated<'a>() -> Vec<RegisteredText<'a>> {
        vec![
            reg("2:0", "Hello ", Some(1)),
            reg("3:0", "Tommy", Some(1)),
            reg("4:0", "!", Some(1)),
        ]
    }

    #[test]
    fn spans_within_one_element() {
        let spans = resolve_spans(&elems(), (0, 6), (0, 15));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].key, "p1");
        assert_eq!(&spans[0].text[spans[0].range.clone()], "paragraph");
        assert_eq!(resolve_spans(&elems(), (0, 15), (0, 6)), spans);
    }

    #[test]
    fn spans_across_elements_cover_middles_whole() {
        let spans = resolve_spans(&elems(), (0, 6), (2, 5));
        assert_eq!(spans.len(), 3);
        assert_eq!(&spans[0].text[spans[0].range.clone()], "paragraph");
        assert_eq!(&spans[1].text[spans[1].range.clone()], "second");
        assert_eq!(&spans[2].text[spans[2].range.clone()], "third");
        assert_eq!(resolve_spans(&elems(), (2, 5), (0, 6)), spans);
    }

    #[test]
    fn drag_lifecycle_and_copy_joins() {
        let mut sel = SelectionState::default();
        sel.arm("p1", 6);
        assert!(sel.promote_pending());
        assert_eq!(sel.active_drag(), Some(("p1".to_string(), 6)));
        let spans = resolve_spans(&elems(), (0, 6), (1, 6));
        assert!(sel.update_spans(spans.clone()));
        assert!(!sel.update_spans(spans));
        assert_eq!(sel.wash_range("p1"), Some(6..15));
        assert_eq!(sel.wash_range("p2"), Some(0..6));
        assert_eq!(sel.wash_range("p3"), None);
        assert_eq!(sel.end_drag("p1").as_deref(), Some("paragraph\nsecond"));
        assert_eq!(sel.selected_text().as_deref(), Some("paragraph\nsecond"));
        sel.clear();
        assert_eq!(sel.selected_text(), None);
    }

    #[test]
    fn empty_click_clears_on_release() {
        let mut sel = SelectionState::default();
        sel.arm("p1", 3);
        assert!(sel.promote_pending());
        assert_eq!(sel.end_drag("p1"), None);
        assert_eq!(sel.selected_text(), None);
    }

    #[test]
    fn tap_does_not_select_until_drag() {
        let mut sel = SelectionState::default();
        sel.arm("p1", 3);
        assert!(sel.is_pending());
        assert!(!sel.is_active());
        assert_eq!(sel.active_drag(), None);
        sel.cancel_pending();
        assert!(!sel.is_pending());
        assert_eq!(sel.selected_text(), None);
    }

    #[test]
    fn pending_press_promotes_only_once() {
        let mut sel = SelectionState::default();
        sel.arm("p1", 6);
        assert!(sel.promote_pending());
        assert!(!sel.promote_pending());
        assert_eq!(sel.active_drag(), Some(("p1".to_string(), 6)));
        let spans = resolve_spans(&elems(), (0, 6), (0, 15));
        assert!(sel.update_spans(spans));
        assert_eq!(sel.end_drag("p1").as_deref(), Some("paragraph"));
    }

    #[test]
    fn double_click_span() {
        let mut sel = SelectionState::default();
        sel.begin_with_span("p1", "hello world", 6..11);
        assert_eq!(sel.wash_range("p1"), Some(6..11));
        assert_eq!(sel.end_drag("p1").as_deref(), Some("world"));
    }

    #[test]
    fn word_ranges() {
        let t = "let foo_bar = 12;";
        assert_eq!(word_range(t, 5), 4..11);
        assert_eq!(word_range(t, 4), 4..11);
        assert_eq!(word_range(t, 11), 4..11);
        assert_eq!(word_range(t, 15), 14..16);
        assert_eq!(&t[word_range(t, 12)], "=");
        assert_eq!(word_range(t, 3), 0..3);
        let u = "héllo wörld";
        assert_eq!(&u[word_range(u, 2)], "héllo");
        let combining = "cafe\u{301} noir";
        assert_eq!(&combining[word_range(combining, 2)], "cafe\u{301}");
    }

    /// A stale index past a shrunk element's text must clamp, not panic.
    #[test]
    fn resolve_spans_clamps_out_of_range_offsets() {
        let spans = resolve_spans(&[reg("a", "hé", None)], (0, 0), (0, 99));
        assert_eq!(&spans[0].text[spans[0].range.clone()], "hé");
    }

    /// Vue splits one line into three host nodes. Copy must not insert
    /// newlines between them.
    #[test]
    fn copy_joins_one_group_without_newlines() {
        let mut sel = SelectionState::default();
        sel.arm("2:0", 0);
        assert!(sel.promote_pending());
        let spans = resolve_spans(&interpolated(), (0, 0), (2, 1));
        assert!(sel.update_spans(spans));
        assert_eq!(sel.selected_text().as_deref(), Some("Hello Tommy!"));
    }

    #[test]
    fn copy_separates_groups_with_newlines() {
        let elements = vec![
            reg("2:0", "Hello ", Some(1)),
            reg("3:0", "Tommy", Some(1)),
            reg("5:0", "second line", Some(4)),
        ];
        let mut sel = SelectionState::default();
        sel.arm("2:0", 0);
        assert!(sel.promote_pending());
        assert!(sel.update_spans(resolve_spans(&elements, (0, 0), (2, 11))));
        assert_eq!(
            sel.selected_text().as_deref(),
            Some("Hello Tommy\nsecond line")
        );
    }

    /// `<code>` and `<diff>` register their runs without a group, so every
    /// line stays a line even though one element painted them all.
    #[test]
    fn ungrouped_runs_always_separate() {
        let elements = vec![
            reg("7:0", "let a = 1;", None),
            reg("7:1", "let b = 2;", None),
        ];
        let mut sel = SelectionState::default();
        sel.arm("7:0", 0);
        assert!(sel.promote_pending());
        assert!(sel.update_spans(resolve_spans(&elements, (0, 0), (1, 10))));
        assert_eq!(
            sel.selected_text().as_deref(),
            Some("let a = 1;\nlet b = 2;")
        );
    }
}
