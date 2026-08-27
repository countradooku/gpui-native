//! Selectable text: the shared foundation for every text GPUIX paints.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//!
//! Every text node in GPUIX goes through [`paint::selectable_text`], so a drag
//! can start in a plain `<text>` and end inside a `<code>` block. That only
//! works because all of them register into the same per-frame registry in paint
//! order. Adding a new text-painting element means calling that helper, never
//! `div().child(string)`.

pub mod paint;
pub mod runs;
pub mod search;
pub mod selection;

pub use paint::{
    chrome_text, log_painted_text, painted_highlights, painted_text, range_rects,
    record_start_region, selectable_text, selection_frame_reset, selection_key, HighlightSource,
    PaintedHighlight, SelectableText, SharedSelection,
};
pub use search::{GroupList, HighlightContext, HighlightSet};
