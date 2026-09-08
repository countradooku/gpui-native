/// Retained element tree — the Rust-side source of truth for the UI.
///
/// Vue's custom renderer sends mutations (create, append, remove, etc.) via napi.
/// This tree stores those mutations. `GpuiView` builds ephemeral GPUI elements
/// from it, while virtual lists defer offscreen subtrees until layout requests them.
///
/// All IDs are u64 — JS generates them with an incrementing counter,
/// passes them as numbers across napi (no string allocation).
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use gpui::SharedString;

use crate::style::StyleDesc;

const KNOWN_EVENTS: [&str; 21] = [
    "toggleFile",
    "showMore",
    "lineClick",
    "linkClick",
    "change",
    "submit",
    "click",
    "auxClick",
    "mouseDown",
    "mouseUp",
    "mouseEnter",
    "mouseLeave",
    "mouseMove",
    "mouseDownOutside",
    "keyDown",
    "keyUp",
    "focus",
    "blur",
    "scroll",
    "visibleRange",
    "highlight",
];

/// Allocation-free lookup for the renderer's event vocabulary, with a small
/// forward-compatible side set for future/custom event names.
#[derive(Clone, Default)]
pub struct EventSet {
    known: u32,
    unknown: HashSet<String>,
}

impl EventSet {
    fn known_bit(event: &str) -> Option<u32> {
        KNOWN_EVENTS
            .iter()
            .position(|candidate| *candidate == event)
            .map(|index| 1u32 << index)
    }

    pub fn contains(&self, event: &str) -> bool {
        Self::known_bit(event).is_some_and(|bit| self.known & bit != 0)
            || self.unknown.contains(event)
    }

    fn insert(&mut self, event: String) -> bool {
        if let Some(bit) = Self::known_bit(&event) {
            let changed = self.known & bit == 0;
            self.known |= bit;
            changed
        } else {
            self.unknown.insert(event)
        }
    }

    fn remove(&mut self, event: &str) -> bool {
        if let Some(bit) = Self::known_bit(event) {
            let changed = self.known & bit != 0;
            self.known &= !bit;
            changed
        } else {
            self.unknown.remove(event)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.known == 0 && self.unknown.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        KNOWN_EVENTS
            .iter()
            .enumerate()
            .filter(|(index, _)| self.known & (1u32 << index) != 0)
            .map(|(_, event)| *event)
            .chain(self.unknown.iter().map(String::as_str))
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.iter().map(str::to_owned).collect();
        names.sort_unstable();
        names
    }
}

#[derive(Clone)]
pub struct RetainedElement {
    pub id: u64,
    pub element_type: String,
    /// Shared, not owned. `StyleDesc` is ~1.4 KB, and holding it inline made an
    /// unstyled element pay for a style it does not have: `RetainedElement` was
    /// 1624 bytes, and the `HashMap` stores values inline, so the whole table
    /// was that wide. Behind an `Arc` the record is 248 bytes.
    ///
    /// The pointer is also shared between every element that declared the same
    /// style. A 10k-turn chat has 59 320 `setStyle` ops and 90 distinct styles.
    ///
    /// Read it with `.as_deref()` to get `Option<&StyleDesc>`. Never mutate
    /// through it; `Arc` has no `DerefMut`, so the compiler enforces that.
    pub style: Option<Arc<StyleDesc>>,
    pub content: Option<SharedString>,
    pub events: EventSet,
    pub children: Vec<u64>,
    pub parent: Option<u64>,
    /// Props for custom elements (input, editor, diff, etc.).
    /// Keyed by prop name, values are JSON. Ignored for "div" and "text".
    pub custom_props: HashMap<String, serde_json::Value>,
    /// Take keyboard focus the first time this element gets a focus handle.
    /// Without it an `<input>` is dead until the user clicks it.
    pub auto_focus: bool,
    /// Last mutation applied to this element or one of its descendants.
    pub subtree_revision: u64,
    /// Last change to the searchable TEXT of this subtree: content, structure,
    /// or the set of nested `highlight` declarations.
    ///
    /// Separate from `subtree_revision` because `highlight` is itself a custom
    /// prop: keying the group cache on the general revision means every find-bar
    /// keystroke re-walks and re-folds the whole subtree, which is the one case
    /// the cache exists for. Style, `activeIndex`, colours and a native
    /// element's own props never move this.
    pub search_revision: u64,
    /// Stable locator id from the Vue `testId` prop.
    pub test_id: Option<String>,
}

impl RetainedElement {
    pub fn new(id: u64, element_type: String, revision: u64) -> Self {
        Self {
            id,
            element_type,
            style: None,
            content: None,
            events: EventSet::default(),
            children: Vec::new(),
            parent: None,
            auto_focus: false,
            subtree_revision: revision,
            search_revision: revision,
            test_id: None,
            custom_props: HashMap::new(),
        }
    }
}

/// The element map, keyed by the u64 counter JS allocates.
///
/// Deliberately not the std hasher. `mark_changed` probes this map once per
/// ancestor hop on every append, insert, style and text mutation, and
/// `build_virtual_list` probes it twice per child on every frame. The keys come
/// from our own counter, never from user input, so `SipHash` only costs time.
pub type ElementMap = rustc_hash::FxHashMap<u64, Arc<RetainedElement>>;

/// One hash-consed style: the raw JSON that produced it, and the shared value.
///
/// The raw bytes are kept so a hash hit can be confirmed by comparing content.
/// A 64-bit hash collides eventually, and a collision here would paint one
/// element with another element's style, which is a bug nobody would find.
struct InternedStyle {
    raw: Box<[u8]>,
    style: Arc<StyleDesc>,
}

/// Below this many entries a sweep is not worth thinking about, so the table
/// never sweeps on a small app just because it grew from one style to two.
pub(crate) const STYLE_SWEEP_FLOOR: usize = 64;

/// Styles shared by content. Keyed by a hash of the raw payload, with a bucket
/// per key so collisions are resolved by comparing bytes.
///
/// Interning happens here, on arrival, rather than in the protocol. JS cannot
/// do it safely: `commitUpdate` resends the full style on every commit, and a
/// dragged element produces a distinct style every frame, so a JS-owned table
/// would grow without bound and the update path would send a definition plus a
/// reference where it sends one op today.
///
/// Separate from the element tree so `resolve_styles` can borrow it alone. That
/// is what makes a batch atomic: interning is the only fallible step, and the
/// borrow checker proves it cannot have touched an element.
#[derive(Default)]
pub struct StyleTable {
    entries: rustc_hash::FxHashMap<u64, Vec<InternedStyle>>,
    /// Entries currently held. Tracked rather than summed, so `maybe_sweep` is
    /// O(1) on the commits that do not sweep.
    count: usize,
    /// `count` right after the last sweep.
    swept_at: usize,
}

impl StyleTable {
    /// Parse a style payload, reusing the shared value when the same bytes
    /// arrived before. Hashing ~110 bytes is far cheaper than building the
    /// ~80 `Option` fields of a `StyleDesc`.
    pub fn intern(&mut self, raw: &[u8]) -> Result<Arc<StyleDesc>, String> {
        let mut hasher = rustc_hash::FxHasher::default();
        raw.hash(&mut hasher);
        let key = hasher.finish();
        if let Some(bucket) = self.entries.get(&key)
            && let Some(hit) = bucket.iter().find(|entry| &*entry.raw == raw)
        {
            return Ok(hit.style.clone());
        }
        let mut style: StyleDesc =
            serde_json::from_slice(raw).map_err(|error| error.to_string())?;
        style.resolve_cached_values();
        let shared = Arc::new(style);
        self.entries.entry(key).or_default().push(InternedStyle {
            raw: raw.into(),
            style: shared.clone(),
        });
        self.count += 1;
        Ok(shared)
    }

    /// Drop interned styles no element references any more.
    ///
    /// This is what makes the table safe on an interactive app. A drag creates
    /// one style per frame; each is released once the element stops referencing
    /// it. `strong_count == 1` means the table holds the last reference, so
    /// nothing can be reading it.
    pub fn sweep(&mut self) {
        let mut live = 0;
        self.entries.retain(|_, bucket| {
            bucket.retain(|entry| Arc::strong_count(&entry.style) > 1);
            live += bucket.len();
            !bucket.is_empty()
        });
        self.count = live;
        self.swept_at = live;
    }

    /// Sweep when the table has grown, or when the tree has shrunk under it.
    ///
    /// Growth alone is not enough. After a large mount `swept_at` is large, so
    /// destroying the root or remounting a smaller app leaves `count` below the
    /// next threshold forever, and the whole high-water table stays resident.
    /// Re-interning an existing style does not raise `count`, so nothing would
    /// ever reclaim it.
    ///
    /// `live_elements` closes that. A live style needs at least one element
    /// holding it, so the live style count can never exceed the element count;
    /// a table far larger than the tree is therefore mostly dead. After a sweep
    /// `count <= live_elements`, so this cannot thrash.
    ///
    /// Both arms are amortized: each scan is O(entries) and follows at least as
    /// many interns or as large a collapse.
    pub fn maybe_sweep(&mut self, live_elements: usize) {
        let grew = self.count >= self.swept_at.saturating_mul(2).max(STYLE_SWEEP_FLOOR);
        let tree_shrank = self.count > live_elements.saturating_mul(2).max(STYLE_SWEEP_FLOOR);
        if grew || tree_shrank {
            self.sweep();
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.count
    }
}

pub struct RetainedTree {
    pub elements: ElementMap,
    pub styles: StyleTable,
    /// The root element ID set by appendChildToContainer.
    pub root_id: Option<u64>,
    next_revision: u64,
}

impl Default for RetainedTree {
    fn default() -> Self {
        Self::new()
    }
}

impl RetainedTree {
    pub fn new() -> Self {
        Self {
            elements: ElementMap::default(),
            styles: StyleTable::default(),
            root_id: None,
            next_revision: 1,
        }
    }

    /// Intern a raw style payload and assign it, then keep the table bounded.
    ///
    /// The single-op entry points go through here so none of them can forget
    /// the sweep. `apply_batch_to_tree` resolves its styles up front instead,
    /// because it must do so before it mutates anything.
    pub fn set_style_json(&mut self, id: u64, raw: &[u8]) -> Result<(), String> {
        let style = self.styles.intern(raw)?;
        self.set_style(id, style);
        let live_elements = self.elements.len();
        self.styles.maybe_sweep(live_elements);
        Ok(())
    }

    pub fn create_element(&mut self, id: u64, element_type: String) {
        let revision = self.take_revision();
        self.elements.insert(
            id,
            Arc::new(RetainedElement::new(id, element_type, revision)),
        );
    }

    fn take_revision(&mut self) -> u64 {
        let revision = self.next_revision;
        self.next_revision = self.next_revision.wrapping_add(1).max(1);
        revision
    }

    /// Monotonic whole-tree revision used to skip frame maintenance that only
    /// depends on retained mutations.
    pub fn revision(&self) -> u64 {
        self.next_revision
    }

    /// Immutable view used by GPUI after releasing the mutation mutex.
    ///
    /// Cloning the map only increments one `Arc` per element. Styles, custom
    /// payloads, text, and child vectors remain shared; later mutations use
    /// `Arc::make_mut` on only the records they touch.
    pub(crate) fn render_snapshot(&self) -> Self {
        Self {
            elements: self.elements.clone(),
            // The renderer reads styles through each element's Arc. The
            // interning table is mutation-only and must not be duplicated.
            styles: StyleTable::default(),
            root_id: self.root_id,
            next_revision: self.next_revision,
        }
    }

    pub fn set_root(&mut self, id: u64) {
        if self.root_id != Some(id) {
            self.root_id = Some(id);
            self.take_revision();
        }
    }

    /// Invalidate `id` and its ancestors, including their searchable text.
    fn mark_changed(&mut self, id: u64) {
        self.mark_changed_detail(id, true);
    }

    /// Invalidate for rendering only. Use for changes that cannot move a glyph
    /// into or out of the searchable text: style, and a native element's own
    /// props, whose text is matched at paint and never enters a `GroupList`.
    fn mark_render_changed(&mut self, id: u64) {
        self.mark_changed_detail(id, false);
    }

    fn mark_changed_detail(&mut self, id: u64, search: bool) {
        let revision = self.take_revision();
        let mut current = Some(id);
        while let Some(current_id) = current {
            let Some(element) = self.elements.get_mut(&current_id) else {
                break;
            };
            let element = Arc::make_mut(element);
            element.subtree_revision = revision;
            if search {
                element.search_revision = revision;
            }
            current = element.parent;
        }
    }

    /// Recursively destroy an element and all its children.
    /// Returns all destroyed IDs so the caller can clean up JS-side state.
    ///
    /// Unlinks from the parent BEFORE removing, then marks the parent chain
    /// changed. Vue normally sends `removeChild` first, so this used to look
    /// harmless, but the batch and napi APIs allow a direct destroy: without the
    /// unlink the parent keeps a dangling child id, and without `mark_changed`
    /// any cache keyed on `subtree_revision` keeps serving text that is no
    /// longer in the tree.
    pub fn destroy_element(&mut self, id: u64) -> Vec<u64> {
        let parent_id = self.elements.get(&id).and_then(|element| element.parent);
        if let Some(parent_id) = parent_id
            && let Some(parent) = self.elements.get_mut(&parent_id)
        {
            let parent = Arc::make_mut(parent);
            parent.children.retain(|child| *child != id);
        }
        let mut destroyed = Vec::new();
        self.destroy_element_recursive(id, &mut destroyed);
        if self.root_id == Some(id) {
            self.root_id = None;
        }
        if let Some(parent_id) = parent_id {
            self.mark_changed(parent_id);
        } else if !destroyed.is_empty() {
            self.take_revision();
        }
        destroyed
    }

    fn destroy_element_recursive(&mut self, id: u64, destroyed: &mut Vec<u64>) {
        if let Some(element) = self.elements.remove(&id) {
            destroyed.push(id);
            for &child_id in &element.children {
                self.destroy_element_recursive(child_id, destroyed);
            }
        }
    }

    fn validate_reparent(&self, parent_id: u64, child_id: u64) -> Result<(), String> {
        if parent_id == child_id {
            return Err(format!("element {child_id} cannot be its own parent"));
        }
        if !self.elements.contains_key(&parent_id) {
            return Err(format!("parent element {parent_id} does not exist"));
        }
        if !self.elements.contains_key(&child_id) {
            return Err(format!("child element {child_id} does not exist"));
        }

        // Follow the prospective parent's chain. The step bound also makes
        // this safe if an older/corrupt tree already contains a cycle.
        let mut current = Some(parent_id);
        for _ in 0..=self.elements.len() {
            let Some(id) = current else {
                return Ok(());
            };
            if id == child_id {
                return Err(format!(
                    "reparenting element {child_id} under {parent_id} would create a cycle"
                ));
            }
            current = self.elements.get(&id).and_then(|element| element.parent);
        }
        Err("retained tree already contains a parent cycle".to_string())
    }

    pub fn append_child(&mut self, parent_id: u64, child_id: u64) -> Result<(), String> {
        self.validate_reparent(parent_id, child_id)?;
        self.append_child_unchecked(parent_id, child_id);
        Ok(())
    }

    /// Apply a relationship already checked by the batch's structural shadow.
    pub(crate) fn append_child_unchecked(&mut self, parent_id: u64, child_id: u64) {
        // Remove from old parent if any
        let old_parent_id = self.elements.get(&child_id).and_then(|e| e.parent);
        if let Some(old_parent_id) = old_parent_id
            && let Some(old_parent) = self.elements.get_mut(&old_parent_id)
        {
            let old_parent = Arc::make_mut(old_parent);
            old_parent.children.retain(|c| *c != child_id);
        }
        // Set new parent
        if let Some(child) = self.elements.get_mut(&child_id) {
            let child = Arc::make_mut(child);
            child.parent = Some(parent_id);
        }
        // Add to new parent's children
        if let Some(parent) = self.elements.get_mut(&parent_id) {
            let parent = Arc::make_mut(parent);
            parent.children.push(child_id);
        }
        if let Some(old_parent_id) = old_parent_id {
            self.mark_changed(old_parent_id);
        }
        self.mark_changed(parent_id);
    }

    pub fn remove_child(&mut self, parent_id: u64, child_id: u64) {
        if let Some(parent) = self.elements.get_mut(&parent_id) {
            let parent = Arc::make_mut(parent);
            parent.children.retain(|c| *c != child_id);
        }
        if let Some(child) = self.elements.get_mut(&child_id) {
            let child = Arc::make_mut(child);
            child.parent = None;
        }
        self.mark_changed(parent_id);
    }

    pub fn insert_before(
        &mut self,
        parent_id: u64,
        child_id: u64,
        before_id: u64,
    ) -> Result<(), String> {
        self.validate_reparent(parent_id, child_id)?;
        if child_id == before_id {
            return Ok(());
        }
        let before_parent = self
            .elements
            .get(&before_id)
            .and_then(|element| element.parent);
        if before_parent != Some(parent_id) {
            return Err(format!(
                "before element {before_id} is not a child of parent {parent_id}"
            ));
        }
        self.insert_before_unchecked(parent_id, child_id, before_id);
        Ok(())
    }

    /// Apply a relationship already checked by the batch's structural shadow.
    pub(crate) fn insert_before_unchecked(
        &mut self,
        parent_id: u64,
        child_id: u64,
        before_id: u64,
    ) {
        if child_id == before_id {
            return;
        }
        // Remove from old parent if any
        let old_parent_id = self.elements.get(&child_id).and_then(|e| e.parent);
        if let Some(old_parent_id) = old_parent_id
            && let Some(old_parent) = self.elements.get_mut(&old_parent_id)
        {
            let old_parent = Arc::make_mut(old_parent);
            old_parent.children.retain(|c| *c != child_id);
        }
        // Set new parent
        if let Some(child) = self.elements.get_mut(&child_id) {
            let child = Arc::make_mut(child);
            child.parent = Some(parent_id);
        }
        // Insert before the target
        if let Some(parent) = self.elements.get_mut(&parent_id) {
            let parent = Arc::make_mut(parent);
            let pos = parent
                .children
                .iter()
                .position(|c| *c == before_id)
                .unwrap_or(parent.children.len());
            parent.children.insert(pos, child_id);
        }
        if let Some(old_parent_id) = old_parent_id {
            self.mark_changed(old_parent_id);
        }
        self.mark_changed(parent_id);
    }

    /// Takes an interned style. Pointer identity is the fast path and covers
    /// every resend, because `StyleTable::intern` returns the same `Arc` for
    /// the same bytes. It is not sufficient on its own: interning keys on raw
    /// bytes, so `{"color":"red","width":10}` and `{"width":10,"color":"red"}`
    /// are two `Arc`s holding the same style. Only a pointer miss pays for the
    /// ~80-field compare, which is what decides whether this repaints.
    pub fn set_style(&mut self, id: u64, style: Arc<StyleDesc>) {
        let mut changed = false;
        if let Some(element) = self.elements.get_mut(&id) {
            let element = Arc::make_mut(element);
            let same = element
                .style
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &style) || **current == *style);
            if !same {
                element.style = Some(style);
                changed = true;
            }
        }
        if changed {
            self.mark_render_changed(id);
        }
    }

    pub fn set_text(&mut self, id: u64, content: String) {
        let mut changed = false;
        if let Some(element) = self.elements.get_mut(&id)
            && element
                .content
                .as_ref()
                .is_none_or(|current| current.as_ref() != content)
        {
            let element = Arc::make_mut(element);
            element.content = Some(content.into());
            changed = true;
        }
        if changed {
            self.mark_changed(id);
        }
    }

    pub fn set_event_listener(&mut self, id: u64, event_type: String, has_handler: bool) {
        let mut changed = false;
        if let Some(element) = self.elements.get_mut(&id) {
            let element = Arc::make_mut(element);
            changed = if has_handler {
                element.events.insert(event_type)
            } else {
                element.events.remove(&event_type)
            };
        }
        if changed {
            self.mark_render_changed(id);
        }
    }

    /// Set a custom prop on an element (for non-div/text elements).
    ///
    /// Custom props never change the searchable text of a subtree: a native
    /// element's strings are matched at paint, not collected into a group. The
    /// one exception is `highlight` itself appearing or disappearing, which
    /// changes which subtrees an ancestor skips.
    pub fn set_custom_prop(&mut self, id: u64, key: String, value: serde_json::Value) {
        let mut changed = false;
        let is_highlight = key == "highlight";
        let was_declaration = self
            .elements
            .get(&id)
            .is_some_and(|element| element.custom_props.contains_key("highlight"));
        if let Some(element) = self.elements.get_mut(&id) {
            let element = Arc::make_mut(element);
            // `autoFocus` applies to every element type, so it is lifted out of
            // the custom-prop map that only custom elements read.
            if key == "autoFocus" {
                let auto_focus = value.as_bool().unwrap_or(false);
                changed = element.auto_focus != auto_focus;
                element.auto_focus = auto_focus;
            } else if key == "testId" {
                element.test_id = value.as_str().map(str::to_string);
                return;
            } else if value.is_null() {
                changed = element.custom_props.remove(&key).is_some();
            } else if element.custom_props.get(&key) != Some(&value) {
                element.custom_props.insert(key, value);
                changed = true;
            }
        }
        if !changed {
            return;
        }
        self.mark_render_changed(id);
        let is_declaration = self
            .elements
            .get(&id)
            .is_some_and(|element| element.custom_props.contains_key("highlight"));
        if is_highlight && was_declaration != is_declaration {
            // Only the ancestors: `GroupList::collect` skips a nested
            // declaration's subtree, so which subtrees exist changed for them.
            // This element's own groups are unaffected by its own query.
            if let Some(parent) = self.elements.get(&id).and_then(|element| element.parent) {
                self.mark_changed(parent);
            }
        }
    }

    /// Read a custom prop value from an element.
    pub fn get_custom_prop(&self, id: u64, key: &str) -> Option<&serde_json::Value> {
        self.elements.get(&id)?.custom_props.get(key)
    }

    #[cfg(all(
        feature = "test-support",
        any(target_os = "macos", target_os = "windows")
    ))]
    pub fn to_json(
        &self,
        bounds: &std::collections::HashMap<u64, crate::automation::ElementBounds>,
    ) -> serde_json::Value {
        self.to_json_detail(bounds, true)
    }

    /// Locator tree. Skip style maps so a 5k-row list is not 100ms of JSON.
    pub fn to_automation_json(
        &self,
        bounds: &std::collections::HashMap<u64, crate::automation::ElementBounds>,
    ) -> serde_json::Value {
        self.to_json_detail(bounds, false)
    }

    fn to_json_detail(
        &self,
        bounds: &std::collections::HashMap<u64, crate::automation::ElementBounds>,
        include_details: bool,
    ) -> serde_json::Value {
        match self.root_id {
            Some(root_id) => element_to_json(root_id, self, bounds, include_details),
            None => serde_json::Value::Null,
        }
    }
}

fn element_to_json(
    id: u64,
    tree: &RetainedTree,
    bounds: &std::collections::HashMap<u64, crate::automation::ElementBounds>,
    include_details: bool,
) -> serde_json::Value {
    let Some(element) = tree.elements.get(&id) else {
        return serde_json::Value::Null;
    };

    let mut obj = serde_json::Map::new();
    obj.insert(
        "type".to_string(),
        serde_json::Value::String(element.element_type.clone()),
    );
    obj.insert("id".to_string(), serde_json::json!(element.id));

    if let Some(ref test_id) = element.test_id {
        obj.insert(
            "testId".to_string(),
            serde_json::Value::String(test_id.clone()),
        );
    }

    if let Some(ref content) = element.content {
        obj.insert(
            "text".to_string(),
            serde_json::Value::String(content.to_string()),
        );
    }

    if let Some(rect) = bounds.get(&id) {
        obj.insert(
            "bounds".to_string(),
            serde_json::json!({
                "x": rect.x,
                "y": rect.y,
                "width": rect.width,
                "height": rect.height,
            }),
        );
    }

    if include_details {
        if let Some(ref style) = element.style {
            // `as_ref`, not the `Arc`. Serializing the pointer needs serde's
            // `rc` feature, which we never asked for; it only compiles because
            // gpui happens to enable it, and would break when gpui stops.
            if let Ok(style_json) = serde_json::to_value(style.as_ref())
                && let serde_json::Value::Object(ref map) = style_json
            {
                let filtered: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !filtered.is_empty() {
                    obj.insert("style".to_string(), serde_json::Value::Object(filtered));
                }
            }
        }

        if !element.events.is_empty() {
            let events = element.events.names();
            obj.insert("events".to_string(), serde_json::json!(events));
        }

        if !element.custom_props.is_empty() {
            let custom: serde_json::Map<String, serde_json::Value> = element
                .custom_props
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            obj.insert("customProps".to_string(), serde_json::Value::Object(custom));
        }
    }

    if !element.children.is_empty() {
        let children: Vec<serde_json::Value> = element
            .children
            .iter()
            .map(|&cid| element_to_json(cid, tree, bounds, include_details))
            .filter(|v| !v.is_null())
            .collect();
        if !children.is_empty() {
            obj.insert("children".to_string(), serde_json::Value::Array(children));
        }
    }

    serde_json::Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree_with_child() -> RetainedTree {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        tree.create_element(2, "text".to_string());
        tree.create_element(3, "text".to_string());
        tree.append_child(1, 2).unwrap();
        tree.append_child(2, 3).unwrap();
        tree.set_text(3, "hello".to_string());
        tree
    }

    #[test]
    fn destroy_unlinks_from_parent() {
        let mut tree = tree_with_child();
        let destroyed = tree.destroy_element(2);
        assert_eq!(destroyed, vec![2, 3]);
        assert_eq!(tree.elements[&1].children, Vec::<u64>::new());
        assert!(!tree.elements.contains_key(&3));
    }

    #[test]
    fn destroy_bumps_the_parent_chain_revision() {
        let mut tree = tree_with_child();
        let before = tree.elements[&1].subtree_revision;
        tree.destroy_element(2);
        assert!(
            tree.elements[&1].subtree_revision > before,
            "destroying a child must invalidate a subtree_revision cache on the parent"
        );
    }

    #[test]
    fn destroying_the_root_clears_it() {
        let mut tree = tree_with_child();
        tree.root_id = Some(1);
        tree.destroy_element(1);
        assert_eq!(tree.root_id, None);
        assert!(tree.elements.is_empty());
    }

    #[test]
    fn reparenting_rejects_self_and_ancestor_cycles_without_mutation() {
        let mut tree = tree_with_child();
        let before = (
            tree.elements[&1].children.clone(),
            tree.elements[&2].parent,
            tree.elements[&3].children.clone(),
        );

        assert!(tree.append_child(1, 1).unwrap_err().contains("own parent"));
        assert!(
            tree.append_child(3, 1)
                .unwrap_err()
                .contains("would create a cycle")
        );
        assert_eq!(
            (
                tree.elements[&1].children.clone(),
                tree.elements[&2].parent,
                tree.elements[&3].children.clone(),
            ),
            before
        );
    }

    /// The whole point of `search_revision`: `highlight` is a custom prop, so
    /// keying the group cache on `subtree_revision` means every find-bar
    /// keystroke re-walks and re-folds the subtree it exists to avoid.
    #[test]
    fn a_query_change_does_not_move_search_revision() {
        let mut tree = tree_with_child();
        tree.set_custom_prop(
            1,
            "highlight".to_string(),
            serde_json::json!({"query": "a"}),
        );
        let search = tree.elements[&1].search_revision;
        let subtree = tree.elements[&1].subtree_revision;

        tree.set_custom_prop(
            1,
            "highlight".to_string(),
            serde_json::json!({"query": "ab"}),
        );
        assert_eq!(tree.elements[&1].search_revision, search, "same text");
        assert!(
            tree.elements[&1].subtree_revision > subtree,
            "still repaints"
        );
    }

    #[test]
    fn style_does_not_move_search_revision() {
        let mut tree = tree_with_child();
        let search = tree.elements[&1].search_revision;
        let style = tree.styles.intern(br##"{"color":"#fff"}"##).unwrap();
        tree.set_style(3, style);
        assert_eq!(tree.elements[&1].search_revision, search);
        assert!(tree.elements[&1].subtree_revision > 0);
    }

    // ── Style interning ──────────────────────────────────────────────

    #[test]
    fn equal_style_bytes_share_one_allocation() {
        let mut tree = tree_with_child();
        let first = tree.styles.intern(br#"{"color":"red"}"#).unwrap();
        let second = tree.styles.intern(br#"{"color":"red"}"#).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(tree.styles.len(), 1);

        let other = tree.styles.intern(br#"{"color":"blue"}"#).unwrap();
        assert!(!Arc::ptr_eq(&first, &other));
        assert_eq!(tree.styles.len(), 2);
    }

    /// Re-sending the same style must not repaint. `commitUpdate` resends the
    /// full style on every commit, so without this every commit would dirty
    /// every element it touched.
    #[test]
    fn resending_the_same_style_is_not_a_change() {
        let mut tree = tree_with_child();
        let style = tree.styles.intern(br#"{"color":"red"}"#).unwrap();
        tree.set_style(3, style);
        let revision = tree.elements[&3].subtree_revision;

        let same = tree.styles.intern(br#"{"color":"red"}"#).unwrap();
        tree.set_style(3, same);
        assert_eq!(tree.elements[&3].subtree_revision, revision);

        let different = tree.styles.intern(br#"{"color":"blue"}"#).unwrap();
        tree.set_style(3, different);
        assert!(tree.elements[&3].subtree_revision > revision);
    }

    /// The table must not grow without bound. A dragged element produces a
    /// distinct style every frame, so the sweep is what keeps this safe.
    #[test]
    fn sweep_releases_styles_no_element_references() {
        let mut tree = tree_with_child();
        for frame in 0..50 {
            let payload = format!(r#"{{"left":{frame}}}"#);
            let style = tree.styles.intern(payload.as_bytes()).unwrap();
            tree.set_style(3, style);
            tree.styles.sweep();
            assert_eq!(tree.styles.len(), 1, "frame {frame} leaked a style");
        }
    }

    /// A drag through the real entry point, which is the one an app hits.
    /// Calling `sweep()` by hand would pass even with `maybe_sweep` broken.
    #[test]
    fn a_drag_through_set_style_json_stays_bounded() {
        let mut tree = tree_with_child();
        for frame in 0..1_000 {
            let payload = format!(r#"{{"left":{frame}}}"#);
            tree.set_style_json(3, payload.as_bytes()).unwrap();
            assert!(
                tree.styles.len() <= STYLE_SWEEP_FLOOR * 2,
                "frame {frame} grew the table to {}",
                tree.styles.len(),
            );
        }
    }

    /// Growth alone never fires again once the tree collapses: `swept_at` stays
    /// at the high-water mark and `count` cannot climb past it, because
    /// re-interning an existing style does not add an entry. Destroying the
    /// tree must reclaim the table anyway.
    #[test]
    fn destroying_the_tree_releases_its_styles() {
        let mut tree = RetainedTree::new();
        tree.create_element(1, "div".to_string());
        tree.root_id = Some(1);

        let wide = STYLE_SWEEP_FLOOR * 4;
        for index in 0..wide {
            let child = 100 + index as u64;
            tree.create_element(child, "div".to_string());
            tree.append_child(1, child).unwrap();
            tree.set_style_json(child, format!(r#"{{"left":{index}}}"#).as_bytes())
                .unwrap();
        }
        assert_eq!(tree.styles.len(), wide, "every style is still live");

        tree.destroy_element(1);
        assert!(tree.elements.is_empty(), "the tree is gone");

        // Exactly what `apply_batch_to_tree` does after a batch that destroyed
        // the root. Nothing new is interned, so only the element count can
        // trigger this.
        let live_elements = tree.elements.len();
        tree.styles.maybe_sweep(live_elements);
        assert_eq!(tree.styles.len(), 0, "the destroyed tree kept its styles");
    }

    /// A style table larger than the tree must not re-sweep on every commit.
    #[test]
    fn the_shrink_sweep_does_not_thrash() {
        let mut tree = RetainedTree::new();
        for index in 0..(STYLE_SWEEP_FLOOR * 4) {
            let id = index as u64 + 1;
            tree.create_element(id, "div".to_string());
            tree.set_style_json(id, format!(r#"{{"left":{index}}}"#).as_bytes())
                .unwrap();
        }
        let live = tree.styles.len();
        let live_elements = tree.elements.len();
        tree.styles.maybe_sweep(live_elements);
        assert_eq!(tree.styles.len(), live, "a full table must survive a sweep");
    }

    #[test]
    fn sweep_keeps_a_style_two_elements_still_use() {
        let mut tree = tree_with_child();
        let style = tree.styles.intern(br#"{"color":"red"}"#).unwrap();
        tree.set_style(2, style.clone());
        tree.set_style(3, style);
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 1);

        let replacement = tree.styles.intern(br#"{"color":"blue"}"#).unwrap();
        tree.set_style(3, replacement);
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 2, "element 2 still holds red");

        let replacement = tree.styles.intern(br#"{"color":"blue"}"#).unwrap();
        tree.set_style(2, replacement);
        tree.styles.sweep();
        assert_eq!(tree.styles.len(), 1, "red is now unreferenced");
    }

    /// Sharing must never let one element's animation restyle another.
    #[test]
    fn a_shared_style_is_never_mutated_through() {
        let mut tree = tree_with_child();
        let style = tree.styles.intern(br#"{"color":"red"}"#).unwrap();
        tree.set_style(2, style.clone());
        tree.set_style(3, style);

        // This is what the motion path does: copy out, then mutate the copy.
        let mut animated = tree.elements[&3].style.as_deref().cloned().unwrap();
        animated.color = Some("green".to_string());

        assert_eq!(
            tree.elements[&2].style.as_ref().unwrap().color.as_deref(),
            Some("red")
        );
        assert_eq!(
            tree.elements[&3].style.as_ref().unwrap().color.as_deref(),
            Some("red")
        );
    }

    /// A nested declaration appearing changes which subtrees an ancestor skips,
    /// so the ancestor's collected groups really did change.
    #[test]
    fn a_nested_declaration_appearing_moves_the_ancestor() {
        let mut tree = tree_with_child();
        let search = tree.elements[&1].search_revision;
        tree.set_custom_prop(
            2,
            "highlight".to_string(),
            serde_json::json!({"query": "a"}),
        );
        assert!(tree.elements[&1].search_revision > search);

        // Changing that nested query does not.
        let after = tree.elements[&1].search_revision;
        tree.set_custom_prop(
            2,
            "highlight".to_string(),
            serde_json::json!({"query": "b"}),
        );
        assert_eq!(tree.elements[&1].search_revision, after);
    }

    #[test]
    fn text_and_structure_move_search_revision() {
        let mut tree = tree_with_child();
        let search = tree.elements[&1].search_revision;
        tree.set_text(3, "changed".to_string());
        assert!(tree.elements[&1].search_revision > search);

        let after = tree.elements[&1].search_revision;
        tree.destroy_element(3);
        assert!(tree.elements[&1].search_revision > after);
    }

    #[test]
    fn set_text_bumps_every_ancestor() {
        let mut tree = tree_with_child();
        let before = tree.elements[&1].subtree_revision;
        tree.set_text(3, "changed".to_string());
        assert!(tree.elements[&1].subtree_revision > before);
        // An unchanged value must not invalidate anything.
        let after = tree.elements[&1].subtree_revision;
        tree.set_text(3, "changed".to_string());
        assert_eq!(tree.elements[&1].subtree_revision, after);
    }
}
