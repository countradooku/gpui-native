//! Cross-element text selection state.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//! Original: `crates/ui/src/markdown/selection.rs`.
//!
//! GPUI has no built-in selection for plain text. Zed's markdown selects
//! continuously because its whole document is ONE element over one text model.
//! GPUI Vue renders a TREE of text elements, so this module rebuilds that
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
//! `GpuiView` instead of a process-global. GPUI Vue is a library and a process may
//! host more than one renderer, so a global would let two windows fight over one
//! selection.

use std::ops::Range;

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
}

impl SelectionState {
    /// Begin a drag anchored at `(key, ix)`.
    pub fn begin(&mut self, key: &str, ix: usize) {
        self.anchor_key = key.to_string();
        self.anchor_ix = ix;
        self.dragging = true;
        self.active = true;
        self.spans.clear();
    }

    /// Begin with an immediate span — double or triple click inside one element.
    pub fn begin_with_span(&mut self, key: &str, text: &str, range: Range<usize>) {
        self.anchor_key = key.to_string();
        self.anchor_ix = range.start;
        self.dragging = true;
        self.active = true;
        self.spans = vec![Span {
            key: key.to_string(),
            range,
            text: text.to_string(),
        }];
    }

    /// The live drag's anchor offset, if `key` owns the drag.
    pub fn drag_anchor(&self, key: &str) -> Option<usize> {
        (self.active && self.dragging && self.anchor_key == key).then_some(self.anchor_ix)
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

    /// Clear when `key` owns a settled selection. A mouse-down landed outside
    /// the owner; the element the down landed IN claims right after. Returns
    /// true when something was cleared.
    pub fn clear_if_owner(&mut self, key: &str) -> bool {
        if self.active && self.anchor_key == key && !self.dragging {
            self.clear();
            return true;
        }
        false
    }

    pub fn clear(&mut self) {
        *self = SelectionState::default();
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
/// `(element index, byte offset)` into `elements` (document-ordered
/// `(key, text)` pairs). Handles either direction; empty slices are skipped.
pub fn resolve_spans(elements: &[(&str, &str)], a: (usize, usize), b: (usize, usize)) -> Vec<Span> {
    let (start, end) = if (a.0, a.1) <= (b.0, b.1) {
        (a, b)
    } else {
        (b, a)
    };
    let mut spans = Vec::new();
    for (ei, (key, text)) in elements.iter().enumerate().take(end.0 + 1).skip(start.0) {
        let from = if ei == start.0 { start.1 } else { 0 };
        let to = if ei == end.0 { end.1 } else { text.len() };
        let (from, to) = (clamp_boundary(text, from), clamp_boundary(text, to));
        if from < to {
            spans.push(Span {
                key: (*key).to_string(),
                range: from..to,
                text: (*text).to_string(),
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

fn join_spans(spans: &[Span]) -> String {
    spans
        .iter()
        .filter(|s| !s.range.is_empty())
        .map(|s| &s.text[s.range.clone()])
        .collect::<Vec<_>>()
        .join("\n")
}

/// Word range around `ix` for double-click selection: an alphanumeric/`_` run,
/// or the single non-space char under the cursor, or empty at spaces.
pub fn word_range(text: &str, ix: usize) -> Range<usize> {
    let ix = clamp_boundary(text, ix);
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let before = text[..ix].chars().next_back();
    let at = text[ix..].chars().next();
    if !at.is_some_and(is_word) && !before.is_some_and(is_word) {
        return match at {
            Some(c) if !c.is_whitespace() => ix..ix + c.len_utf8(),
            _ => ix..ix,
        };
    }
    let start = text[..ix]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map(|(i, _)| i)
        .unwrap_or(ix);
    let end = text[ix..]
        .char_indices()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map(|(i, c)| ix + i + c.len_utf8())
        .unwrap_or(ix);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elems<'a>() -> Vec<(&'a str, &'a str)> {
        vec![
            ("p1", "first paragraph"),
            ("p2", "second"),
            ("p3", "third one"),
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
        sel.begin("p1", 6);
        assert_eq!(sel.drag_anchor("p1"), Some(6));
        assert_eq!(sel.drag_anchor("p2"), None);
        let spans = resolve_spans(&elems(), (0, 6), (1, 6));
        assert!(sel.update_spans(spans.clone()));
        assert!(!sel.update_spans(spans));
        assert_eq!(sel.wash_range("p1"), Some(6..15));
        assert_eq!(sel.wash_range("p2"), Some(0..6));
        assert_eq!(sel.wash_range("p3"), None);
        assert_eq!(sel.end_drag("p1").as_deref(), Some("paragraph\nsecond"));
        assert_eq!(sel.selected_text().as_deref(), Some("paragraph\nsecond"));
        assert!(!sel.clear_if_owner("p2"));
        assert!(sel.clear_if_owner("p1"));
        assert_eq!(sel.selected_text(), None);
    }

    #[test]
    fn empty_click_clears_on_release() {
        let mut sel = SelectionState::default();
        sel.begin("p1", 3);
        assert_eq!(sel.end_drag("p1"), None);
        assert_eq!(sel.selected_text(), None);
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
    }

    /// A stale index past a shrunk element's text must clamp, not panic.
    #[test]
    fn resolve_spans_clamps_out_of_range_offsets() {
        let spans = resolve_spans(&[("a", "hé")], (0, 0), (0, 99));
        assert_eq!(&spans[0].text[spans[0].range.clone()], "hé");
    }
}
