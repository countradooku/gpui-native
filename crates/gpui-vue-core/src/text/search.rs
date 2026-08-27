//! Highlight ranges painted behind text: search matches and explicit ranges.
//!
//! There is deliberately no joined "document" for the `query` case. Vue makes
//! a separate host node for every interpolated string, so `<text>Hello {name}!`
//! is three painted runs of ONE logical line. Those runs are merged into a
//! [`Group`]; a match never crosses a group. Chrome's find behaves the same way
//! across a paragraph boundary, and it means a 5k-row list is 5k small strings
//! instead of one megabyte string that must be rebuilt on every keystroke.
//!
//! Grouping is structural (same parent host element, adjacent children). It never
//! reads `display`, which only knows `flex` and `grid` here anyway.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use gpui::Hsla;

use crate::retained_tree::RetainedTree;

// ── Spec ─────────────────────────────────────────────────────────────

/// One `highlight` entry as it arrives from JS, with colours already resolved
/// against the theme.
#[derive(Clone, Debug, PartialEq)]
pub struct HighlightSpec {
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// `[start, end)` in UTF-16 code units, indexing the declaring subtree's
    /// joined text. Empty unless the caller supplied explicit ranges.
    pub ranges: Vec<(usize, usize)>,
    pub color: Hsla,
    pub active_color: Hsla,
    pub active_index: Option<usize>,
    /// How many MATCHES come before this subtree in the caller's document, so
    /// the nth match here is numbered `match_index_offset + n`.
    ///
    /// It is a match count, not a row index. Rows hold different numbers of
    /// matches, so a row index cannot stand in for it.
    ///
    /// A `<virtual-list>` mounts a window of its rows, so nothing native can
    /// see the matches above it. Without this, `activeIndex` would silently
    /// mean "the nth match in the window" and a find cursor would land on the
    /// wrong row. `<virtual-list>` already takes `windowStart` and `itemCount`
    /// from the app for the same reason.
    pub match_index_offset: usize,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HighlightSet {
    pub specs: Vec<HighlightSpec>,
}

impl HighlightSet {
    /// Parse the raw custom prop. `null`, a bad shape, or an all-empty set
    /// yields `None`, so nothing resolves and nothing paints.
    pub fn parse(value: &serde_json::Value, theme: &crate::theme::Theme) -> Option<Self> {
        let items = match value {
            serde_json::Value::Array(items) => items.as_slice(),
            serde_json::Value::Object(_) => std::slice::from_ref(value),
            _ => return None,
        };
        let specs: Vec<HighlightSpec> = items
            .iter()
            .filter_map(|item| HighlightSpec::parse(item, theme))
            .collect();
        (!specs.is_empty()).then_some(Self { specs })
    }

    /// Key for the match cache. Deliberately excludes `active_index`,
    /// `match_index_offset`, `color` and `active_color`: moving a find-bar
    /// cursor or scrolling a virtual list must not rescan any text.
    pub fn matcher_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for spec in &self.specs {
            spec.query.hash(&mut hasher);
            spec.case_sensitive.hash(&mut hasher);
            spec.whole_word.hash(&mut hasher);
            spec.ranges.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// True when any spec supplies explicit ranges. Only then is the joined
    /// document built, because only then does anyone index into it.
    pub fn needs_document(&self) -> bool {
        self.specs.iter().any(|spec| !spec.ranges.is_empty())
    }
}

impl HighlightSpec {
    fn parse(value: &serde_json::Value, theme: &crate::theme::Theme) -> Option<Self> {
        let object = value.as_object()?;
        let string = |key: &str| object.get(key).and_then(serde_json::Value::as_str);
        let flag = |key: &str| {
            object
                .get(key)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        let color = |key: &str| string(key).and_then(crate::color::parse_color_rgba);

        // A bad offset silently renumbers every match, which shows up as a find
        // cursor on the wrong row and nothing else. Refuse the spec instead of
        // treating a malformed value as absent.
        let match_index_offset = match object.get("matchIndexOffset") {
            None | Some(serde_json::Value::Null) => 0,
            Some(value) => match value.as_u64() {
                Some(offset) => offset as usize,
                None => {
                    log::warn!(
                        "highlight matchIndexOffset must be a non-negative integer, got {value}"
                    );
                    return None;
                }
            },
        };

        let query = string("query").unwrap_or_default().to_string();
        let ranges = object
            .get("ranges")
            .and_then(serde_json::Value::as_array)
            .map(|pairs| {
                pairs
                    .iter()
                    .filter_map(|pair| {
                        let pair = pair.as_array()?;
                        let start = pair.first()?.as_u64()? as usize;
                        let end = pair.get(1)?.as_u64()? as usize;
                        (start < end).then_some((start, end))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if query.is_empty() && ranges.is_empty() {
            return None;
        }

        let mut base = theme.accent;
        base.a = 0.30;
        let mut active = theme.accent;
        active.a = 0.65;
        Some(Self {
            query,
            case_sensitive: flag("caseSensitive"),
            whole_word: flag("wholeWord"),
            ranges,
            color: color("color").map(Into::into).unwrap_or(base),
            active_color: color("activeColor").map(Into::into).unwrap_or(active),
            active_index: object
                .get("activeIndex")
                .and_then(serde_json::Value::as_u64)
                .map(|n| n as usize),
            match_index_offset,
            radius: object
                .get("radius")
                .and_then(serde_json::Value::as_f64)
                .map(|n| n as f32)
                .unwrap_or(2.0),
        })
    }
}

// ── Lowercasing ──────────────────────────────────────────────────────
//
// This is Unicode default lowercasing (`str::to_lowercase`), NOT full case
// folding. `ﬀ` does not match `ff`, and `İ` lowercases to `i` plus a combining
// dot. `findRanges` in `packages/react/src/hooks/use-text-search.ts` uses
// JS `toLowerCase`, which follows the same Unicode rules.

/// Lowercase `text` and record, for every folded byte, the byte offset of the
/// original character that produced it.
///
/// Lowercasing alone is not enough: Unicode case conversion changes byte length
/// (`İ` is 2 bytes and folds to 3), so a match offset in folded space does not
/// index the original. The map converts it back exactly.
fn fold(text: &str) -> (String, Vec<u32>) {
    let mut folded = String::with_capacity(text.len());
    let mut map: Vec<u32> = Vec::with_capacity(text.len() + 1);
    for (ix, ch) in text.char_indices() {
        for lower in ch.to_lowercase() {
            let len = lower.len_utf8();
            folded.push(lower);
            for _ in 0..len {
                map.push(ix as u32);
            }
        }
    }
    map.push(text.len() as u32);
    (folded, map)
}

// ── Offsets ──────────────────────────────────────────────────────────

/// UTF-16 code-unit range to a UTF-8 byte range.
///
/// JS gives UTF-16 indices, which is what `indexOf` and `RegExp.exec` return.
/// A boundary that falls inside a surrogate pair has no character boundary here,
/// so it is rejected rather than snapped: silently moving a caller's range is
/// worse than telling them it was wrong.
fn utf16_range_to_bytes(text: &str, start: usize, end: usize) -> Option<Range<usize>> {
    if start >= end {
        return None;
    }
    let (mut byte_start, mut byte_end) = (None, None);
    let mut units = 0usize;
    for (ix, ch) in text.char_indices() {
        if units == start {
            byte_start = Some(ix);
        }
        if units == end {
            byte_end = Some(ix);
        }
        units += ch.len_utf16();
    }
    if units == start {
        byte_start = Some(text.len());
    }
    if units == end {
        byte_end = Some(text.len());
    }
    match (byte_start, byte_end) {
        (Some(s), Some(e)) if s < e => Some(s..e),
        _ => None,
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn is_whole_word(text: &str, range: &Range<usize>) -> bool {
    let before = text[..range.start].chars().next_back();
    let after = text[range.end..].chars().next();
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

/// A resolved declaration: the spec, plus the retained matches located once for
/// the whole subtree.
///
/// One value serves both kinds of run. A retained run looks its matches up by
/// key; a native run matches the string it is about to paint. Both take their
/// ordinal from the same per-frame sequence, which is what makes `activeIndex`
/// mean "the nth match in the document".
#[derive(Debug)]
pub struct HighlightContext {
    /// Element that declared the `highlight`. It is the per-frame ordinal
    /// sequence's identity, and it is what a virtual-list row re-resolves
    /// against after the root render returned.
    pub declaration: u64,
    pub set: HighlightSet,
    /// Shared, so a colour or `activeIndex` change reuses the located matches.
    pub matches: Arc<MatchSet>,
}

/// Identity of one match within a declaration, stable across the runs that
/// paint it and across a repaint of the same run.
#[derive(Clone, PartialEq, Eq, Hash)]
enum MatchId {
    /// Located at build time; the value is its document-order index per spec.
    /// A match split over several interpolated runs shares one id, so it takes
    /// exactly one ordinal.
    Retained(usize),
    /// Generated during `render()`; identified by the run and the position
    /// inside it, because nothing located it earlier.
    Native(Arc<str>, usize),
}

/// Per-frame match numbering, shared by retained and native runs.
#[derive(Default)]
struct Ordinals {
    /// Next ordinal per `(declaration, spec)`.
    cursor: HashMap<(u64, usize), usize>,
    /// Ordinals already handed out, so a match painted by two runs, or a row
    /// gpui paints twice, keeps the number it got the first time.
    assigned: HashMap<(u64, usize, MatchId), usize>,
}

thread_local! {
    static ORDINALS: RefCell<Ordinals> = RefCell::new(Ordinals::default());
}

/// Clear the per-frame match numbering. Called by the frame reset, which paints
/// before any text, including `gpui::list()` rows and deferred overlays.
pub fn ordinal_frame_reset() {
    ORDINALS.with(|state| *state.borrow_mut() = Ordinals::default());
}

/// The wash for one match, numbered in PAINT order.
///
/// Numbering at build time cannot work: retained matches are located before the
/// frame, native text only exists during it, and a subtree can interleave them
/// (`<code>` then `<text>`). Numbering each kind separately made `activeIndex`
/// point at the wrong match whenever a native element came first.
fn wash(
    ctx: &HighlightContext,
    spec_index: usize,
    spec: &HighlightSpec,
    range: Range<usize>,
    id: MatchId,
) -> Wash {
    let slot = (ctx.declaration, spec_index, id);
    let ordinal = ORDINALS.with(|state| {
        let state = &mut *state.borrow_mut();
        if let Some(assigned) = state.assigned.get(&slot) {
            return *assigned;
        }
        let next = state
            .cursor
            .entry((slot.0, slot.1))
            .or_insert(spec.match_index_offset);
        let ordinal = *next;
        *next += 1;
        state.assigned.insert(slot, ordinal);
        ordinal
    });
    let active = spec.active_index == Some(ordinal);
    Wash {
        range,
        color: if active {
            spec.active_color
        } else {
            spec.color
        },
        radius: spec.radius,
        active,
    }
}

/// Washes for a retained `<text>` run, looked up by selection key.
pub fn washes_for_retained_run(ctx: &HighlightContext, key: &Arc<str>) -> Vec<Wash> {
    let Some(entries) = ctx.matches.by_key.get(key) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let spec = ctx.set.specs.get(entry.spec)?;
            let id = MatchId::Retained(entry.index);
            Some(wash(ctx, entry.spec, spec, entry.range.clone(), id))
        })
        .collect()
}

/// Washes for a string a native element is about to paint.
///
/// `<code>`, `<markdown>` and `<diff>` build their text inside `render()`, so it
/// never reaches the retained tree and [`GroupList::collect`] cannot see it.
/// They match the exact string they are painting instead, which makes drift
/// between the search pass and the paint pass impossible.
///
/// Explicit `ranges` are skipped here: they index the retained subtree's joined
/// document, which a natively generated string is not part of.
pub fn washes_for_native_run(ctx: &HighlightContext, key: &Arc<str>, text: &str) -> Vec<Wash> {
    let mut out = Vec::new();
    // Folding allocates, and a case-sensitive spec never reads it.
    let folded = ctx
        .set
        .specs
        .iter()
        .any(|spec| !spec.case_sensitive && !spec.query.is_empty())
        .then(|| fold(text));
    let (folded, fold_map) = match &folded {
        Some((folded, map)) => (folded.as_str(), map.as_slice()),
        None => ("", [].as_slice()),
    };

    for (spec_index, spec) in ctx.set.specs.iter().enumerate() {
        for (position, range) in matches_in(text, folded, fold_map, spec)
            .into_iter()
            .enumerate()
        {
            let id = MatchId::Native(key.clone(), position);
            out.push(wash(ctx, spec_index, spec, range, id));
        }
    }
    out
}

/// Non-overlapping byte ranges of `spec.query` in `text`, leftmost first.
///
/// `folded` and `fold_map` come from [`fold`] and are cached with the group, so
/// a keystroke never re-folds text that did not change.
fn matches_in(
    text: &str,
    folded: &str,
    fold_map: &[u32],
    spec: &HighlightSpec,
) -> Vec<Range<usize>> {
    if spec.query.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if spec.case_sensitive {
        for (ix, hit) in text.match_indices(spec.query.as_str()) {
            let range = ix..ix + hit.len();
            if !spec.whole_word || is_whole_word(text, &range) {
                out.push(range);
            }
        }
        return out;
    }
    let needle = spec.query.to_lowercase();
    for (ix, hit) in folded.match_indices(needle.as_str()) {
        let (Some(&start), Some(&end)) = (fold_map.get(ix), fold_map.get(ix + hit.len())) else {
            continue;
        };
        let range = start as usize..end as usize;
        if range.start >= range.end {
            continue;
        }
        if !spec.whole_word || is_whole_word(text, &range) {
            out.push(range);
        }
    }
    out
}

// ── Groups ───────────────────────────────────────────────────────────

/// Consecutive primitive text children of one host element, merged into the
/// single logical line the author wrote.
#[derive(Debug)]
pub struct Group {
    /// Selection key of each painted run, with its byte range inside `text`.
    pub parts: Vec<(Arc<str>, Range<usize>)>,
    pub text: String,
    folded: String,
    fold_map: Vec<u32>,
}

impl Group {
    fn new(parts: Vec<(Arc<str>, Range<usize>)>, text: String) -> Self {
        let (folded, fold_map) = fold(&text);
        Self {
            parts,
            text,
            folded,
            fold_map,
        }
    }
}

#[derive(Debug, Default)]
pub struct GroupList {
    groups: Vec<Group>,
}

impl GroupList {
    /// Collect the groups of `id`'s subtree, skipping any descendant that
    /// declares its own `highlight`: the nearest declaration wins, so an
    /// ancestor must not resolve or count matches that will never paint.
    pub fn collect(tree: &RetainedTree, id: u64) -> Self {
        let mut groups = Vec::new();
        collect_into(tree, id, true, &mut groups);
        Self { groups }
    }

    /// The joined text, groups separated by a newline, plus each group's start
    /// offset. Built only when a spec supplies explicit `ranges`.
    fn document(&self) -> (String, Vec<usize>) {
        let mut text = String::new();
        let mut starts = Vec::with_capacity(self.groups.len());
        for group in &self.groups {
            if !text.is_empty() {
                text.push('\n');
            }
            starts.push(text.len());
            text.push_str(&group.text);
        }
        (text, starts)
    }
}

/// True for a primitive text node, the kind Vue makes for a raw string.
///
/// Shape only: an empty one is transparent rather than a run boundary, so
/// `{'a'}{''}{'b'}` stays one line. Copy uses the same predicate through
/// [`group_id`], which is the only way the two can agree.
pub fn is_text_leaf(element: &crate::retained_tree::RetainedElement) -> bool {
    element.element_type == "text"
        && element.children.is_empty()
        && !element.custom_props.contains_key("highlight")
}

/// Id of the first run of the adjacent primitive-text run `id` belongs to, or
/// `None` for a run that never merges with a neighbour.
///
/// This is the group identity the selection registry stores. It must stay in
/// step with [`collect_into`]: `element.parent` alone is not enough, because a
/// non-text sibling between two text leaves ends the run for search but would
/// not end it for copy.
pub fn group_id(tree: &RetainedTree, id: u64) -> Option<u64> {
    let element = tree.elements.get(&id)?;
    if !is_text_leaf(element) {
        return None;
    }
    let parent = tree.elements.get(&element.parent?)?;
    let position = parent.children.iter().position(|child| *child == id)?;
    let mut first = id;
    for &sibling in parent.children[..position].iter().rev() {
        match tree.elements.get(&sibling) {
            Some(sibling_element) if is_text_leaf(sibling_element) => first = sibling,
            _ => break,
        }
    }
    Some(first)
}

fn collect_into(tree: &RetainedTree, id: u64, is_root: bool, out: &mut Vec<Group>) {
    let Some(element) = tree.elements.get(&id) else {
        return;
    };
    if !is_root && element.custom_props.contains_key("highlight") {
        return;
    }
    // Own content is a line of its own. Only for built-ins: `build_custom`
    // renders from props and ignores `content`, so collecting it there would
    // invent matches that never paint.
    let paints_own_content = element.element_type == "text" || element.element_type == "div";
    if let Some(content) = element
        .content
        .as_ref()
        .filter(|text| !text.is_empty() && paints_own_content)
    {
        out.push(Group::new(
            vec![(crate::text::selection_key(id, 0), 0..content.len())],
            content.clone(),
        ));
    }

    let mut pending: Vec<(Arc<str>, Range<usize>)> = Vec::new();
    let mut pending_text = String::new();
    for &child_id in &element.children {
        let Some(child) = tree.elements.get(&child_id) else {
            continue;
        };
        if !is_text_leaf(child) {
            flush(&mut pending, &mut pending_text, out);
            collect_into(tree, child_id, false, out);
            continue;
        }
        let Some(content) = child.content.as_ref().filter(|text| !text.is_empty()) else {
            continue;
        };
        let start = pending_text.len();
        pending_text.push_str(content);
        pending.push((
            crate::text::selection_key(child_id, 0),
            start..pending_text.len(),
        ));
    }
    flush(&mut pending, &mut pending_text, out);
}

fn flush(parts: &mut Vec<(Arc<str>, Range<usize>)>, text: &mut String, out: &mut Vec<Group>) {
    if parts.is_empty() {
        return;
    }
    out.push(Group::new(std::mem::take(parts), std::mem::take(text)));
}

// ── Resolution ───────────────────────────────────────────────────────

/// Where one match landed inside one painted run.
///
/// Colour-free on purpose: this is what the matcher-hash cache stores, so a
/// colour or `activeIndex` change never scans any text again.
#[derive(Clone, Debug)]
struct MatchRef {
    range: Range<usize>,
    spec: usize,
    /// Ordinal of the match within its spec, in document order. Stable across
    /// runs, so a match split over several runs is still one match.
    index: usize,
}

/// Every match of one subtree, keyed by the run that must paint it.
#[derive(Debug, Default)]
pub struct MatchSet {
    by_key: HashMap<Arc<str>, Vec<MatchRef>>,
    /// Matches found, counted once even when split across runs. Reported to JS.
    pub total: usize,
}

impl MatchSet {
    /// Identity for the `onHighlight` guard. The count alone is not enough:
    /// swapping one query for another with the same count is a new result.
    /// Colours and `activeIndex` are excluded, so a find-cursor move is not.
    pub fn identity(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut keys: Vec<&Arc<str>> = self.by_key.keys().collect();
        keys.sort();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.total.hash(&mut hasher);
        for key in keys {
            key.hash(&mut hasher);
            for entry in &self.by_key[key] {
                entry.range.hash(&mut hasher);
                entry.spec.hash(&mut hasher);
                entry.index.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// One painted wash: a byte range of one run, with its colour.
#[derive(Clone, Debug)]
pub struct Wash {
    pub range: Range<usize>,
    pub color: Hsla,
    pub radius: f32,
    pub active: bool,
}

/// Locate every match of a subtree's groups. Colour-free and ordinal-free: both
/// are decided at paint, where document order is known.
pub fn resolve(groups: &GroupList, set: &HighlightSet) -> MatchSet {
    let mut matches = MatchSet::default();
    let document = set.needs_document().then(|| groups.document());

    for (spec_index, spec) in set.specs.iter().enumerate() {
        let mut index = 0usize;
        if !spec.query.is_empty() {
            for group in &groups.groups {
                for range in matches_in(&group.text, &group.folded, &group.fold_map, spec) {
                    push_match(&mut matches, group, &range, spec_index, index);
                    index += 1;
                }
            }
        }
        if let Some((text, starts)) = document.as_ref() {
            for &(start, end) in &spec.ranges {
                let Some(doc_range) = utf16_range_to_bytes(text, start, end) else {
                    log::warn!("highlight range [{start}, {end}) is not a valid UTF-16 range");
                    continue;
                };
                let mut painted = false;
                for (group, &group_start) in groups.groups.iter().zip(starts) {
                    // Groups sit at `start..start + len` with a separating
                    // newline that belongs to no group.
                    let group_end = group_start + group.text.len();
                    if doc_range.end <= group_start || doc_range.start >= group_end {
                        continue;
                    }
                    let local = (doc_range.start.max(group_start) - group_start)
                        ..(doc_range.end.min(group_end) - group_start);
                    if local.start < local.end {
                        push_match(&mut matches, group, &local, spec_index, index);
                        painted = true;
                    }
                }
                // A range that covers only a separator paints nothing, so it is
                // not a match. Counting it would shift every later ordinal.
                if painted {
                    index += 1;
                }
            }
        }
        matches.total += index;
    }
    matches
}

/// Split one group-level range across the runs that actually painted it.
fn push_match(
    matches: &mut MatchSet,
    group: &Group,
    range: &Range<usize>,
    spec: usize,
    index: usize,
) {
    for (key, part) in &group.parts {
        let lo = range.start.max(part.start);
        let hi = range.end.min(part.end);
        if lo >= hi {
            continue;
        }
        matches
            .by_key
            .entry(key.clone())
            .or_default()
            .push(MatchRef {
                range: (lo - part.start)..(hi - part.start),
                spec,
                index,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(query: &str) -> HighlightSpec {
        HighlightSpec {
            query: query.to_string(),
            case_sensitive: false,
            whole_word: false,
            ranges: Vec::new(),
            color: gpui::rgba(0xff000080).into(),
            active_color: gpui::rgba(0x00ff0080).into(),
            active_index: None,
            match_index_offset: 0,
            radius: 2.0,
        }
    }

    fn find(text: &str, spec: &HighlightSpec) -> Vec<Range<usize>> {
        let (folded, map) = fold(text);
        matches_in(text, &folded, &map, spec)
    }

    fn context(groups: &GroupList, set: HighlightSet) -> HighlightContext {
        HighlightContext {
            declaration: 1,
            matches: Arc::new(resolve(groups, &set)),
            set,
        }
    }

    /// Paint every group's runs in document order, the way a frame would, and
    /// collect the washes per selection key.
    fn paint(ctx: &HighlightContext, groups: &GroupList) -> HashMap<Arc<str>, Vec<Wash>> {
        ordinal_frame_reset();
        let mut out = HashMap::new();
        for group in &groups.groups {
            for (key, _) in &group.parts {
                let washes = washes_for_retained_run(ctx, key);
                if !washes.is_empty() {
                    out.insert(key.clone(), washes);
                }
            }
        }
        out
    }

    #[test]
    fn case_insensitive_by_default() {
        assert_eq!(find("Foo foo FOO", &spec("foo")), vec![0..3, 4..7, 8..11]);
    }

    #[test]
    fn case_sensitive_when_asked() {
        let mut s = spec("foo");
        s.case_sensitive = true;
        assert_eq!(find("Foo foo FOO", &s), vec![4..7]);
    }

    #[test]
    fn whole_word_rejects_substrings() {
        let mut s = spec("foo");
        s.whole_word = true;
        assert_eq!(find("foo food _foo foo!", &s), vec![0..3, 14..17]);
    }

    #[test]
    fn empty_query_finds_nothing() {
        assert_eq!(find("anything", &spec("")), Vec::<Range<usize>>::new());
    }

    #[test]
    fn matches_are_not_overlapping() {
        assert_eq!(find("aaaa", &spec("aa")), vec![0..2, 2..4]);
    }

    /// Folding changes byte length, so the offset map is the only thing that
    /// keeps a case-insensitive hit indexing the original string.
    ///
    /// `ẞ` is 3 bytes and folds to `ß`, which is 2. A naive lowercase-both
    /// approach reports a range that is short by one byte per occurrence.
    #[test]
    fn folding_keeps_offsets_in_the_original() {
        let text = "auf der STRAẞE hier";
        let hits = find(text, &spec("straße"));
        assert_eq!(hits.len(), 1);
        assert_eq!(&text[hits[0].clone()], "STRAẞE");
    }

    /// Documented Unicode edge: `İ` lowercases to `i` plus a combining dot, so
    /// a plain `istanbul` query does not match `İstanbul`. Matching the dotted
    /// form works. This is `str::to_lowercase` behaviour, not a bug here, and
    /// the offset map still keeps the reported range on a character boundary.
    #[test]
    fn dotted_capital_i_folds_to_two_characters() {
        let text = "İstanbul";
        assert!(find(text, &spec("istanbul")).is_empty());
        let hits = find(text, &spec("i\u{307}stanbul"));
        assert_eq!(hits.len(), 1);
        assert_eq!(&text[hits[0].clone()], "İstanbul");
    }

    #[test]
    fn matches_containing_an_emoji() {
        let text = "hi 👋 there";
        let hits = find(text, &spec("👋 there"));
        assert_eq!(hits.len(), 1);
        assert_eq!(&text[hits[0].clone()], "👋 there");
    }

    #[test]
    fn utf16_offsets_map_to_bytes() {
        assert_eq!(utf16_range_to_bytes("hello", 1, 3), Some(1..3));
        // "é" is 1 UTF-16 unit and 2 UTF-8 bytes.
        assert_eq!(utf16_range_to_bytes("héllo", 1, 3), Some(1..4));
        // "👋" is 2 UTF-16 units and 4 UTF-8 bytes.
        assert_eq!(utf16_range_to_bytes("a👋b", 1, 3), Some(1..5));
    }

    #[test]
    fn utf16_rejects_a_split_surrogate_pair() {
        assert_eq!(utf16_range_to_bytes("a👋b", 1, 2), None);
        assert_eq!(utf16_range_to_bytes("a👋b", 2, 4), None);
    }

    #[test]
    fn utf16_rejects_reversed_and_out_of_range() {
        assert_eq!(utf16_range_to_bytes("hello", 3, 1), None);
        assert_eq!(utf16_range_to_bytes("hello", 0, 0), None);
        assert_eq!(utf16_range_to_bytes("hello", 2, 99), None);
    }

    // ── Grouping ─────────────────────────────────────────────────────

    /// `<div><text>Hello {{ name }}!</text></div>` — Vue splits one line into
    /// three host nodes that must search as one string.
    fn interpolated_tree() -> RetainedTree {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        tree.create_element(2, "text".to_string());
        tree.append_child(1, 2);
        for (id, text) in [(3, "Hello "), (4, "Tommy"), (5, "!")] {
            tree.create_element(id, "text".to_string());
            tree.append_child(2, id);
            tree.set_text(id, text.to_string());
        }
        tree
    }

    #[test]
    fn interpolated_children_form_one_group() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        assert_eq!(groups.groups.len(), 1);
        assert_eq!(groups.groups[0].text, "Hello Tommy!");
        assert_eq!(groups.groups[0].parts.len(), 3);
    }

    #[test]
    fn a_match_across_runs_splits_into_per_run_washes() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let set = HighlightSet {
            specs: vec![spec("Hello Tommy")],
        };
        let ctx = context(&groups, set);
        assert_eq!(ctx.matches.total, 1);
        let painted = paint(&ctx, &groups);
        assert_eq!(painted[&Arc::from("3:0")][0].range, 0..6);
        assert_eq!(painted[&Arc::from("4:0")][0].range, 0..5);
        assert!(!painted.contains_key(&Arc::<str>::from("5:0")));
    }

    #[test]
    fn separate_parents_are_separate_groups() {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        for (wrapper, leaf, text) in [(2, 3, "quick "), (4, 5, "brown")] {
            tree.create_element(wrapper, "text".to_string());
            tree.append_child(1, wrapper);
            tree.create_element(leaf, "text".to_string());
            tree.append_child(wrapper, leaf);
            tree.set_text(leaf, text.to_string());
        }
        let groups = GroupList::collect(&tree, 1);
        assert_eq!(groups.groups.len(), 2);
        // A match must not cross the line boundary, exactly like browser find.
        let set = HighlightSet {
            specs: vec![spec("quick brown")],
        };
        assert_eq!(resolve(&groups, &set).total, 0);
    }

    #[test]
    fn a_nested_declaration_is_skipped_by_the_ancestor() {
        let mut tree = interpolated_tree();
        tree.set_custom_prop(
            2,
            "highlight".to_string(),
            serde_json::json!({"query": "x"}),
        );
        let groups = GroupList::collect(&tree, 1);
        assert!(groups.groups.is_empty(), "nearest declaration must win");
        // The nested element still resolves its own subtree.
        assert_eq!(GroupList::collect(&tree, 2).groups.len(), 1);
    }

    #[test]
    fn explicit_ranges_index_the_joined_document() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let mut s = spec("");
        s.query = String::new();
        s.ranges = vec![(6, 11)];
        let ctx = context(&groups, HighlightSet { specs: vec![s] });
        assert_eq!(ctx.matches.total, 1);
        assert_eq!(paint(&ctx, &groups)[&Arc::from("4:0")][0].range, 0..5);
    }

    #[test]
    fn active_index_recolours_exactly_one_match() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let mut s = spec("l");
        s.active_index = Some(1);
        let ctx = context(&groups, HighlightSet { specs: vec![s] });
        assert_eq!(ctx.matches.total, 2);
        let painted = paint(&ctx, &groups);
        let washes = &painted[&Arc::<str>::from("3:0")];
        assert_eq!(washes.len(), 2);
        assert!(!washes[0].active);
        assert!(washes[1].active);
    }

    /// A non-text sibling ends the run for search, so it must end it for copy
    /// too. `group_id` is the shared answer.
    #[test]
    fn a_non_text_sibling_ends_the_group_for_both() {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "text".to_string());
        for (id, kind) in [(2, "text"), (3, "div"), (4, "text")] {
            tree.create_element(id, kind.to_string());
            tree.append_child(1, id);
        }
        tree.set_text(2, "A".to_string());
        tree.set_text(4, "C".to_string());

        let groups = GroupList::collect(&tree, 1);
        assert_eq!(groups.groups.len(), 2);
        assert_eq!(group_id(&tree, 2), Some(2));
        assert_eq!(
            group_id(&tree, 4),
            Some(4),
            "the run restarts after the div"
        );
    }

    #[test]
    fn adjacent_leaves_share_a_group_id() {
        let tree = interpolated_tree();
        assert_eq!(group_id(&tree, 3), Some(3));
        assert_eq!(group_id(&tree, 4), Some(3));
        assert_eq!(group_id(&tree, 5), Some(3));
        // The wrapper is not a primitive leaf, so it never merges.
        assert_eq!(group_id(&tree, 2), None);
    }

    /// An empty interpolation must not split a line.
    #[test]
    fn an_empty_leaf_is_transparent() {
        let mut tree = interpolated_tree();
        tree.set_text(4, String::new());
        let groups = GroupList::collect(&tree, 1);
        assert_eq!(groups.groups.len(), 1);
        assert_eq!(groups.groups[0].text, "Hello !");
        assert_eq!(group_id(&tree, 5), Some(3));
    }

    /// The mutation API allows a declaration directly on a text leaf, where
    /// there is no wrapper to collect the content.
    #[test]
    fn a_declaring_leaf_collects_its_own_content() {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "text".to_string());
        tree.set_text(1, "a fox here".to_string());
        let groups = GroupList::collect(&tree, 1);
        assert_eq!(groups.groups.len(), 1);
        assert_eq!(
            resolve(
                &groups,
                &HighlightSet {
                    specs: vec![spec("fox")]
                }
            )
            .total,
            1
        );
    }

    fn native_context(mut s: HighlightSpec, active: usize) -> HighlightContext {
        s.active_index = Some(active);
        HighlightContext {
            declaration: 1,
            set: HighlightSet { specs: vec![s] },
            matches: Arc::new(MatchSet::default()),
        }
    }

    /// Native runs share one sequence, so `activeIndex` marks ONE match even
    /// when the element paints many strings.
    #[test]
    fn native_runs_share_one_active_sequence() {
        ordinal_frame_reset();
        let ctx = native_context(spec("x"), 2);
        let first: Arc<str> = "7:0".into();
        let second: Arc<str> = "7:1".into();
        let line_one = washes_for_native_run(&ctx, &first, "x x");
        let line_two = washes_for_native_run(&ctx, &second, "x x");
        let actives: Vec<bool> = line_one
            .iter()
            .chain(line_two.iter())
            .map(|wash| wash.active)
            .collect();
        assert_eq!(actives, vec![false, false, true, false]);
    }

    /// gpui can paint the same row twice in one frame. The second paint must
    /// reuse the ordinals rather than advance the cursor again.
    #[test]
    fn a_repainted_native_run_keeps_its_ordinals() {
        ordinal_frame_reset();
        let ctx = native_context(spec("x"), 0);
        let key: Arc<str> = "7:0".into();
        let first = washes_for_native_run(&ctx, &key, "x x");
        let again = washes_for_native_run(&ctx, &key, "x x");
        assert_eq!(first[0].active, again[0].active);
        assert!(again[0].active);
        assert!(!again[1].active);
    }

    /// The blocker a review caught: ordinals follow PAINT order, so a native
    /// element painted BEFORE retained text takes the lower numbers. Numbering
    /// retained matches first made `activeIndex: 0` mark the `<text>` match.
    #[test]
    fn ordinals_follow_paint_order_across_both_kinds() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let mut s = spec("l");
        s.active_index = Some(0);
        let ctx = context(&groups, HighlightSet { specs: vec![s] });

        ordinal_frame_reset();
        let native: Arc<str> = "9:0".into();
        // The native element paints first, so it must own ordinal 0.
        let painted_native = washes_for_native_run(&ctx, &native, "l");
        let painted_text = washes_for_retained_run(&ctx, &"3:0".into());
        assert!(
            painted_native[0].active,
            "the first painted match is active"
        );
        assert!(painted_text.iter().all(|wash| !wash.active));

        // Reverse the paint order and the ownership moves with it.
        ordinal_frame_reset();
        let painted_text = washes_for_retained_run(&ctx, &"3:0".into());
        let painted_native = washes_for_native_run(&ctx, &native, "l");
        assert!(painted_text[0].active);
        assert!(painted_native.iter().all(|wash| !wash.active));
    }

    /// A match split across interpolated runs is ONE match, so it takes one
    /// ordinal and the next match gets the following number.
    #[test]
    fn a_split_match_takes_one_ordinal() {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "text".to_string());
        for (id, text) in [(2, "ab"), (3, "ab")] {
            tree.create_element(id, "text".to_string());
            tree.append_child(1, id);
            tree.set_text(id, text.to_string());
        }
        // "abab": matches at 0..2 and 2..4, the first split across both runs.
        let groups = GroupList::collect(&tree, 1);
        let mut s = spec("ba");
        s.active_index = Some(0);
        let ctx = context(&groups, HighlightSet { specs: vec![s] });

        ordinal_frame_reset();
        let first = washes_for_retained_run(&ctx, &"2:0".into());
        let second = washes_for_retained_run(&ctx, &"3:0".into());
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert!(
            first[0].active && second[0].active,
            "one match, one ordinal"
        );
    }

    /// A virtualized subtree holds a window of the document, so the app says
    /// how many matches came before it and `activeIndex` keeps meaning "the
    /// nth match in the document".
    #[test]
    fn match_index_offset_shifts_the_whole_sequence() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let mut s = spec("l");
        s.match_index_offset = 50;
        s.active_index = Some(51);
        let ctx = context(&groups, HighlightSet { specs: vec![s] });

        let painted = paint(&ctx, &groups);
        let washes = &painted[&Arc::<str>::from("3:0")];
        assert_eq!(washes.len(), 2);
        assert!(!washes[0].active, "ordinal 50");
        assert!(washes[1].active, "ordinal 51");
    }

    /// A cursor outside the mounted window paints no active wash at all,
    /// rather than marking an arbitrary visible match.
    #[test]
    fn a_cursor_above_the_window_marks_nothing() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let mut s = spec("l");
        s.match_index_offset = 50;
        s.active_index = Some(9);
        let ctx = context(&groups, HighlightSet { specs: vec![s] });
        assert!(paint(&ctx, &groups)
            .values()
            .flatten()
            .all(|wash| !wash.active));
    }

    /// One declaration is one element, not one allocation.
    ///
    /// `build_virtual_child` re-resolves the declaration after the root render
    /// returned, and gets a NEW `HighlightContext` whenever the tree moved in
    /// between. Ordinals used to key on the `Arc` address of the spec, so that
    /// row restarted the sequence at 0 and the active wash jumped to it. They
    /// key on the declaring element id now, so the sequence continues.
    #[test]
    fn two_contexts_for_one_declaration_share_the_sequence() {
        let resolved = |active: usize| {
            let mut s = spec("x");
            s.active_index = Some(active);
            HighlightContext {
                declaration: 7,
                set: HighlightSet { specs: vec![s] },
                matches: Arc::new(MatchSet::default()),
            }
        };
        // Two separate allocations, same declaring element.
        let root = resolved(1);
        let row = resolved(1);

        ordinal_frame_reset();
        let first = washes_for_native_run(&root, &"7:0".into(), "x");
        let second = washes_for_native_run(&row, &"7:1".into(), "x");
        assert!(!first[0].active, "ordinal 0");
        assert!(second[0].active, "ordinal 1, not a second ordinal 0");
    }

    /// A malformed offset renumbers every match, and the only symptom is a find
    /// cursor on the wrong row. Refuse the spec rather than read it as absent.
    #[test]
    fn a_malformed_match_index_offset_rejects_the_spec() {
        let theme = crate::theme::Theme::dark();
        let parse = |value: serde_json::Value| HighlightSet::parse(&value, &theme);

        assert!(parse(serde_json::json!({"query": "a"})).is_some());
        assert!(parse(serde_json::json!({"query": "a", "matchIndexOffset": 4})).is_some());
        assert!(parse(serde_json::json!({"query": "a", "matchIndexOffset": null})).is_some());
        assert!(parse(serde_json::json!({"query": "a", "matchIndexOffset": -1})).is_none());
        assert!(parse(serde_json::json!({"query": "a", "matchIndexOffset": 1.5})).is_none());
        assert!(parse(serde_json::json!({"query": "a", "matchIndexOffset": "4"})).is_none());
    }

    #[test]
    fn matcher_hash_ignores_the_match_index_offset() {
        let a = spec("foo");
        let mut b = spec("foo");
        b.match_index_offset = 400;
        assert_eq!(
            HighlightSet { specs: vec![a] }.matcher_hash(),
            HighlightSet { specs: vec![b] }.matcher_hash()
        );
    }

    #[test]
    fn identity_ignores_the_find_cursor_but_not_the_query() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let plain = resolve(
            &groups,
            &HighlightSet {
                specs: vec![spec("l")],
            },
        );
        let mut moved = spec("l");
        moved.active_index = Some(1);
        moved.color = gpui::rgba(0x00ff00ff).into();
        let moved = resolve(&groups, &HighlightSet { specs: vec![moved] });
        assert_eq!(plain.identity(), moved.identity());
    }

    #[test]
    fn identity_changes_when_a_query_swaps_at_the_same_count() {
        let groups = GroupList::collect(&interpolated_tree(), 1);
        let a = resolve(
            &groups,
            &HighlightSet {
                specs: vec![spec("Hello")],
            },
        );
        let b = resolve(
            &groups,
            &HighlightSet {
                specs: vec![spec("Tommy")],
            },
        );
        assert_eq!(a.total, b.total);
        assert_ne!(a.identity(), b.identity());
    }

    #[test]
    fn matcher_hash_ignores_paint_only_fields() {
        let mut a = spec("foo");
        let mut b = spec("foo");
        b.active_index = Some(3);
        b.color = gpui::rgba(0x0000ffff).into();
        b.radius = 9.0;
        let set_a = HighlightSet {
            specs: vec![a.clone()],
        };
        let set_b = HighlightSet { specs: vec![b] };
        assert_eq!(set_a.matcher_hash(), set_b.matcher_hash());
        a.whole_word = true;
        let set_c = HighlightSet { specs: vec![a] };
        assert_ne!(set_a.matcher_hash(), set_c.matcher_hash());
    }
}
