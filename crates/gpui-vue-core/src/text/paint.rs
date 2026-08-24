//! The gpui half of text selection: the per-frame registry, the wash geometry,
//! and the window-level mouse and key listeners.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//! Original: the selection sections of `crates/ui/src/markdown/render.rs`.
//!
//! Why the registry is rebuilt during **paint** rather than during build:
//! paint order is the only place where document order is guaranteed, because a
//! `list()` or `uniform_list()` decides at paint time which rows exist. Comet
//! learned this the hard way; do not move registration into `build_element`.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use parking_lot::Mutex;

use gpui::{
    canvas, div, point, prelude::*, px, quad, size, BorderStyle, Bounds, Hsla, SharedString,
    StyledText, TextLayout, TextRun, Window,
};

use super::selection::{self, SelectionState};

/// Shared selection state. `GpuiView` and `GpuiRenderer` both hold clones, and
/// so does every paint closure.
///
/// `Arc<Mutex<..>>` rather than `Rc<RefCell<..>>`: napi requires `GpuiRenderer`
/// to be `Send`, and the renderer needs a handle so `getSelectedText()` works
/// without an App context. All real access is single-threaded, so the mutex is
/// always uncontended.
pub type SharedSelection = Arc<Mutex<SelectionState>>;

/// One painted text element, registered per frame in document order.
struct RegEntry {
    key: Arc<str>,
    text: SharedString,
    layout: TextLayout,
}

thread_local! {
    static REGISTRY: RefCell<Vec<RegEntry>> = const { RefCell::new(Vec::new()) };
    /// Every string painted this frame, selectable or not, in paint order.
    ///
    /// Native elements draw their text inside gpui, so it never appears in the
    /// retained tree and `getAllText()` cannot see it. Without this log the only
    /// way to assert what `<code>` or `<diff>` rendered is a screenshot, which
    /// tells you something changed but never what.
    static PAINTED: RefCell<Vec<SharedString>> = const { RefCell::new(Vec::new()) };
}

/// A zero-size canvas that clears the per-frame registries and installs the
/// frame's copy listener. Paint it FIRST in the root, before any text, so each
/// frame holds exactly that frame's visible text elements in paint order.
pub fn selection_frame_reset(selection: SharedSelection) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            REGISTRY.with(|r| r.borrow_mut().clear());
            PAINTED.with(|p| p.borrow_mut().clear());
            register_copy_listener(window, &selection);
        },
    )
    .absolute()
    .w(px(0.0))
    .h(px(0.0))
}

/// Every string painted in the last frame, in paint order. Test-facing.
pub fn painted_text() -> Vec<String> {
    PAINTED.with(|p| p.borrow().iter().map(|s| s.to_string()).collect())
}

/// Record text painted by a custom element that owns its text layout.
pub fn log_painted_text(text: SharedString) {
    PAINTED.with(|painted| painted.borrow_mut().push(text));
}

/// Text that is deliberately NOT selectable: line-number gutters, language
/// tags, diff file headers. It still lands in the paint log so tests can assert
/// on it, but a drag across the block never copies it.
pub fn chrome_text(text: SharedString, runs: Option<Vec<TextRun>>) -> gpui::AnyElement {
    let styled = match runs {
        Some(runs) => StyledText::new(text.clone()).with_runs(runs),
        None => StyledText::new(text.clone()),
    };
    let log = canvas(
        |_, _, _| (),
        move |_, _, _, _| PAINTED.with(|p| p.borrow_mut().push(text.clone())),
    )
    .absolute()
    .w(px(0.0))
    .h(px(0.0));
    div().relative().child(log).child(styled).into_any_element()
}

/// Selection key for an element. `sub` distinguishes multiple text runs painted
/// by one element, such as the lines of a code block.
pub fn selection_key(element_id: u64, sub: usize) -> Arc<str> {
    format!("{element_id}:{sub}").into()
}

/// Inputs for [`selectable_text`].
pub struct SelectableText {
    pub text: SharedString,
    /// `None` is the important case for plain `<text>` nodes: gpui then derives
    /// one run from `window.text_style()`, so colour, weight and family keep
    /// inheriting from ancestor `style` props. Pass `Some(..)` only when the
    /// element owns its own colours, as `<code>` and `<diff>` do.
    pub runs: Option<Vec<TextRun>>,
    pub key: Arc<str>,
    pub selection: SharedSelection,
    pub wash_color: Hsla,
    /// Paints additional quads under the glyphs before the selection wash:
    /// inline-code pills, word-diff highlights. Receives the laid-out text so
    /// it can turn byte ranges into rects with [`range_rects`].
    pub extra_wash: Option<Box<dyn Fn(&TextLayout, &mut Window)>>,
    /// Clickable byte ranges and their payloads, typically link URLs.
    pub links: Vec<(Range<usize>, String)>,
    /// Called with the payload of the range under a click.
    pub on_link: Option<Arc<dyn Fn(&str)>>,
    /// False under `userSelect: "none"`: the text is still painted, logged and
    /// clickable, but it does not join the selection registry.
    pub selectable: bool,
}

impl SelectableText {
    pub fn new(
        text: SharedString,
        runs: Option<Vec<TextRun>>,
        key: Arc<str>,
        selection: SharedSelection,
        wash_color: Hsla,
    ) -> Self {
        Self {
            text,
            runs,
            key,
            selection,
            wash_color,
            extra_wash: None,
            links: Vec::new(),
            on_link: None,
            selectable: true,
        }
    }
}

/// A selectable text element: `StyledText` with a canvas underlay that paints
/// the selection wash, registers into the frame registry, and installs the
/// mouse listeners.
pub fn selectable_text(opts: SelectableText) -> gpui::AnyElement {
    let SelectableText {
        text,
        runs,
        key,
        selection,
        wash_color,
        extra_wash,
        links,
        on_link,
        selectable,
    } = opts;

    let styled = match runs {
        Some(runs) => StyledText::new(text.clone()).with_runs(runs),
        None => StyledText::new(text.clone()),
    };
    let layout = styled.layout().clone();

    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            if let Some(paint) = &extra_wash {
                paint(&layout, window);
            }
            if let Some(range) = selectable
                .then(|| selection.lock().wash_range(&key))
                .flatten()
            {
                for rect in range_rects(&layout, &range, 0.0, 0.0) {
                    window.paint_quad(quad(
                        rect,
                        px(0.0),
                        wash_color,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
            if selectable {
                REGISTRY.with(|r| {
                    r.borrow_mut().push(RegEntry {
                        key: key.clone(),
                        text: text.clone(),
                        layout: layout.clone(),
                    })
                });
                register_listeners(window, &key, &text, &layout, &selection);
            }
            PAINTED.with(|p| p.borrow_mut().push(text.clone()));
            if let Some(on_link) = &on_link {
                register_link_listener(window, &layout, &links, on_link, &selection);
            }
        },
    )
    .absolute()
    .size_full();

    div()
        .relative()
        .child(underlay)
        .child(styled)
        .into_any_element()
}

/// Fire `on_link` for the range under a click.
///
/// Registered on mouse UP and skipped when a selection exists, so a drag that
/// happens to end on a link selects text instead of navigating. gpui's
/// `InteractiveText` does per-range hit testing too, but it owns the
/// `StyledText` and would displace the selection underlay.
fn register_link_listener(
    window: &mut Window,
    layout: &TextLayout,
    links: &[(Range<usize>, String)],
    on_link: &Arc<dyn Fn(&str)>,
    selection: &SharedSelection,
) {
    use gpui::{DispatchPhase, MouseButton, MouseUpEvent};

    if links.is_empty() {
        return;
    }
    let (layout, links, on_link, selection) = (
        layout.clone(),
        links.to_vec(),
        on_link.clone(),
        selection.clone(),
    );
    window.on_mouse_event(move |e: &MouseUpEvent, phase, _window, _cx| {
        if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
            return;
        }
        if !layout.bounds().contains(&e.position) {
            return;
        }
        // A drag that ends on a link is a selection, not a navigation. gpui
        // dispatches bubble listeners in reverse registration order, so the
        // spans from the drag's mouse-moves are already resolved here.
        if selection.lock().selected_text().is_some() {
            return;
        }
        // `index_for_position` returns Err with the nearest index when the
        // point is past the end of a line. Only an exact hit counts, otherwise
        // clicking the empty space after a paragraph would open its last link.
        let Ok(ix) = layout.index_for_position(e.position) else {
            return;
        };
        if let Some((_, payload)) = links.iter().find(|(range, _)| range.contains(&ix)) {
            on_link(payload);
        }
    });
}

/// `(element index, byte offset)` for a window position.
///
/// Prefers the element whose full bounds contain the point, taking the LAST
/// such element in paint order so an overlay wins over what it covers. Only
/// when the point is outside every text does it fall back to the nearest by
/// vertical distance, which is what makes a drag into the gutter or past the
/// end of a document clamp sensibly.
///
/// Comet compares Y only, because its transcript is a single column where two
/// texts never share a vertical band. GPUI Vue lays out arbitrary Vue trees: a
/// Y-only match picks the leftmost text in a flex row no matter where the
/// pointer actually is.
fn registry_point(position: gpui::Point<gpui::Pixels>) -> Option<(usize, usize)> {
    REGISTRY.with(|r| {
        let reg = r.borrow();
        let mut contained: Option<usize> = None;
        let mut nearest: Option<(usize, (f32, f32))> = None;

        for (ei, entry) in reg.iter().enumerate() {
            let b = entry.layout.bounds();
            if b.contains(&position) {
                contained = Some(ei);
                continue;
            }
            let dy = if position.y < b.top() {
                f32::from(b.top() - position.y)
            } else if position.y > b.bottom() {
                f32::from(position.y - b.bottom())
            } else {
                0.0
            };
            // Within a shared band, break the tie on horizontal distance so a
            // drag in the right-hand column does not snap to the left one.
            // Compared lexicographically: vertical distance dominates outright,
            // because a weighted sum lets a huge dx beat a 1px dy.
            let dx = if position.x < b.left() {
                f32::from(b.left() - position.x)
            } else if position.x > b.right() {
                f32::from(position.x - b.right())
            } else {
                0.0
            };
            let distance = (dy, dx);
            if nearest.is_none_or(|(_, best): (usize, (f32, f32))| {
                (distance.0, distance.1) < (best.0, best.1)
            }) {
                nearest = Some((ei, distance));
            }
        }

        let ei = contained.or(nearest.map(|(ei, _)| ei))?;
        let ix = match reg[ei].layout.index_for_position(position) {
            Ok(ix) | Err(ix) => ix,
        };
        Some((ei, ix))
    })
}

/// Resolve anchor + head into document-ordered spans over the frame's registry.
/// True when the selection changed.
fn resolve_drag(
    selection: &SharedSelection,
    anchor_key: &str,
    anchor_ix: usize,
    head: (usize, usize),
) -> bool {
    REGISTRY.with(|r| {
        let reg = r.borrow();
        let Some(anchor_ei) = reg.iter().position(|e| e.key.as_ref() == anchor_key) else {
            // Anchor scrolled out of this frame — keep the spans we have.
            return false;
        };
        let elements: Vec<(&str, &str)> = reg
            .iter()
            .map(|e| (e.key.as_ref(), e.text.as_ref()))
            .collect();
        let spans = selection::resolve_spans(&elements, (anchor_ei, anchor_ix), head);
        selection.lock().update_spans(spans)
    })
}

/// Register this frame's window-level listeners for one text element.
///
/// Window-level, not element-level, so a drag keeps tracking after the mouse
/// leaves the element's bounds. Frame-scoped, so paint re-registers every frame.
fn register_listeners(
    window: &mut Window,
    key: &Arc<str>,
    text: &SharedString,
    layout: &TextLayout,
    selection: &SharedSelection,
) {
    use gpui::{DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};

    {
        let (key, text, layout, selection) =
            (key.clone(), text.clone(), layout.clone(), selection.clone());
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                return;
            }
            if layout.bounds().contains(&e.position) {
                let ix = match layout.index_for_position(e.position) {
                    Ok(ix) | Err(ix) => ix,
                };
                let mut sel = selection.lock();
                match e.click_count {
                    2 => {
                        let range = selection::word_range(&text, ix);
                        sel.begin_with_span(&key, &text, range);
                    }
                    n if n >= 3 => sel.begin_with_span(&key, &text, 0..text.len()),
                    _ => sel.begin(&key, ix),
                }
                drop(sel);
                window.refresh();
            } else if selection.lock().clear_if_owner(&key) {
                window.refresh();
            }
        });
    }
    {
        let (key, selection) = (key.clone(), selection.clone());
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
            if phase != DispatchPhase::Bubble || !e.dragging() {
                return;
            }
            // Only the anchor element's listener drives the drag.
            let Some(anchor_ix) = selection.lock().drag_anchor(&key) else {
                return;
            };
            let Some(head) = registry_point(e.position) else {
                return;
            };
            if resolve_drag(&selection, &key, anchor_ix, head) {
                window.refresh();
            }
        });
    }
    {
        let (key, selection) = (key.clone(), selection.clone());
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, _cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            selection.lock().end_drag(&key);
        });
    }
}

/// Register the frame's single Cmd+C / Ctrl+C listener.
///
/// GPUI Vue has no keymap or action system, so this reads the raw keystroke.
/// It lives on the frame reset rather than on each text element: registering it
/// per element made one Cmd+C write the clipboard once per visible text node.
fn register_copy_listener(window: &mut Window, selection: &SharedSelection) {
    use gpui::{ClipboardItem, DispatchPhase, KeyDownEvent};

    let selection = selection.clone();
    window.on_key_event(move |e: &KeyDownEvent, phase, _window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        let m = &e.keystroke.modifiers;
        if e.keystroke.key != "c" || !(m.platform || m.control) {
            return;
        }
        // Read out of the lock before touching platform code: the clipboard
        // backend is out of our control and must never run under our mutex.
        let text = selection.lock().selected_text();
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    });
}

/// The wash boxes for one byte range: one box per visual line the range covers,
/// since soft wraps split it, in window coordinates from the laid-out text's own
/// geometry.
///
/// `pad_x` overhangs the box horizontally (inline code); `inset_y` shrinks it
/// vertically. Both are 0 for a selection wash, which wants full-line-height
/// boxes that tile seamlessly across wrapped rows.
pub fn range_rects(
    layout: &TextLayout,
    range: &Range<usize>,
    pad_x: f32,
    inset_y: f32,
) -> Vec<Bounds<gpui::Pixels>> {
    let mut rects = Vec::new();
    let line_height = layout.line_height();
    let mut cur = range.start;
    // Walk the range one visual row at a time: binary search for the furthest
    // index that still sits on the current row.
    let mut guard = 0;
    while cur < range.end && guard < 256 {
        guard += 1;
        let Some(p1) = layout.position_for_index(cur) else {
            break;
        };
        // `seg_end` closes the wash on this row; `next` is the first index on the
        // following row. They differ because a row-end index's position still
        // reports the earlier row, and we need strict progress.
        let (seg_end, next) = match layout.position_for_index(range.end) {
            Some(pe) if pe.y == p1.y => (range.end, range.end),
            _ => {
                let (mut lo, mut hi) = (cur, range.end);
                while hi - lo > 1 {
                    let mid = lo + (hi - lo) / 2;
                    match layout.position_for_index(mid) {
                        Some(pm) if pm.y == p1.y => lo = mid,
                        _ => hi = mid,
                    }
                }
                (lo, hi)
            }
        };
        if let Some(p2) = layout.position_for_index(seg_end) {
            if p2.x > p1.x {
                rects.push(Bounds::new(
                    point(p1.x - px(pad_x), p1.y + px(inset_y)),
                    size(
                        p2.x - p1.x + px(2.0 * pad_x),
                        line_height - px(2.0 * inset_y),
                    ),
                ));
            }
        }
        if next <= cur {
            break;
        }
        cur = next;
    }
    rects
}
