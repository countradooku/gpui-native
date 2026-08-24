//! Markdown: pulldown-cmark parsing into a block tree, then gpui rendering.
//!
//! Ported from Comet (https://github.com/zeronsh/comet), MIT.
//!
//! Two halves on purpose. `parser` is pure and unit tested without a window;
//! `render` turns the tree into elements and knows about theme and selection.

pub mod parser;
pub mod render;
