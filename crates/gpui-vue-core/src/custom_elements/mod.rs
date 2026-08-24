/// Custom element trait infrastructure for GPUI Vue.
///
/// Allows native GPUI components (input, editor, diff) to be used as
/// Vue custom elements with props and callbacks. The renderer dispatches
/// to trait objects at render time — each custom element lives in its own
/// file with its own dependencies, cleanly separated from the core renderer.
///
/// Architecture:
///   build_element()
///     "div"  → build_div()             (built-in)
///     "text" → build_text()            (built-in)
///     _      → registry.render(ctx)    (trait dispatch)
use std::collections::{HashMap, HashSet};

use crate::renderer::EventCallback;

pub mod anchored;
pub mod canvas;
pub mod code;
pub mod diff;
pub mod img;
pub mod input;
pub mod markdown;

// ── Render context ───────────────────────────────────────────────────

/// Context passed to CustomElement::render() with everything needed
/// to build GPUI elements with events and focus.
pub struct CustomRenderContext<'a> {
    /// Numeric element ID (matches Vue's instance ID).
    pub id: u64,
    /// Event types registered by Vue (e.g. "keyDown", "click").
    pub events: &'a HashSet<String>,
    /// Callback for emitting events back to JS.
    pub event_callback: &'a Option<EventCallback>,
    /// Pre-created FocusHandle for this element (if it has keyboard/focus listeners).
    pub focus_handle: Option<&'a gpui::FocusHandle>,
    /// Style object from the retained element for layout and appearance.
    pub style: Option<&'a crate::style::StyleDesc>,
    /// Built child elements from the retained tree for this custom node.
    pub children: Vec<gpui::AnyElement>,
    /// Live text selection. Elements that paint text MUST route it through
    /// `crate::text::selectable_text` with this handle, otherwise their glyphs
    /// are invisible to a drag that starts outside them.
    pub selection: crate::text::SharedSelection,
    /// False when an ancestor set `userSelect: "none"`.
    pub selectable: bool,
    /// Inherited selection wash colour.
    pub selection_wash: gpui::Hsla,
}

impl CustomRenderContext<'_> {
    /// Build a selectable text run for this element. `sub` distinguishes
    /// multiple runs painted by the same element, such as code-block lines, and
    /// must be stable across frames or the selection flickers.
    pub fn text(
        &self,
        sub: usize,
        text: impl Into<gpui::SharedString>,
        runs: Option<Vec<gpui::TextRun>>,
    ) -> gpui::AnyElement {
        let text = text.into();
        if !self.selectable {
            return crate::text::chrome_text(text, runs);
        }
        crate::text::selectable_text(crate::text::SelectableText::new(
            text,
            runs,
            crate::text::selection_key(self.id, sub),
            self.selection.clone(),
            self.selection_wash,
        ))
    }

    /// Chrome text: line numbers, language tags, file headers. Painted and
    /// logged for tests, but never part of a selection, so copying a code block
    /// yields code and not a column of line numbers.
    pub fn chrome_text(
        &self,
        text: impl Into<gpui::SharedString>,
        runs: Option<Vec<gpui::TextRun>>,
    ) -> gpui::AnyElement {
        crate::text::chrome_text(text.into(), runs)
    }
}

// ── Traits ───────────────────────────────────────────────────────────

/// A custom element that renders native GPUI content.
///
/// Lifecycle:
///   1. Factory creates instance via CustomElementFactory::create()
///   2. Vue sends props via set_prop() (called each GPUI frame before render)
///   3. Each GPUI frame calls render() → returns AnyElement
///   4. Vue unmounts → destroy() for cleanup
pub trait CustomElement: 'static {
    /// Build GPUI elements for this frame.
    /// Called on every GPUI render cycle (immediate mode).
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<crate::renderer::GpuiView>,
    ) -> gpui::AnyElement;

    /// Set a named prop from JS. Values are JSON-encoded.
    /// Called when Vue updates props on this element.
    fn set_prop(&mut self, key: &str, value: serde_json::Value);

    /// Return known prop keys. Missing keys are reset to null/default each frame.
    fn supported_props(&self) -> &[&str];

    /// Read a prop value back to JS. Returns None if prop doesn't exist.
    fn get_prop(&self, key: &str) -> Option<serde_json::Value>;

    /// Return which event types this element can emit.
    fn supported_events(&self) -> &[&str];

    /// Clean up resources (GPUI entities, subscriptions, etc.)
    fn destroy(&mut self);
}

/// Factory for creating CustomElement instances.
/// One factory per element type, registered at startup.
pub trait CustomElementFactory: 'static {
    /// The element type name that Vue uses (e.g. "input", "editor", "diff").
    fn element_type(&self) -> &str;

    /// Create a new element instance.
    fn create(&self, id: u64) -> Box<dyn CustomElement>;
}

// ── Registry ─────────────────────────────────────────────────────────

/// Stores factories (one per type) and live instances (one per element ID).
pub struct CustomElementRegistry {
    factories: HashMap<String, Box<dyn CustomElementFactory>>,
    instances: HashMap<u64, Box<dyn CustomElement>>,
}

impl CustomElementRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            instances: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in custom elements.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(input::InputFactory));
        registry.register(Box::new(input::TextareaFactory));
        registry.register(Box::new(anchored::AnchoredFactory));
        registry.register(Box::new(canvas::CanvasFactory));
        registry.register(Box::new(img::ImgFactory));
        registry.register(Box::new(img::SvgFactory));
        registry.register(Box::new(code::CodeFactory));
        registry.register(Box::new(diff::DiffFactory));
        registry.register(Box::new(markdown::MarkdownFactory));
        registry
    }

    pub fn register(&mut self, factory: Box<dyn CustomElementFactory>) {
        self.factories
            .insert(factory.element_type().to_string(), factory);
    }

    /// Get an existing instance or create one via the factory.
    /// Returns None if no factory is registered for this element type.
    pub fn get_or_create(
        &mut self,
        id: u64,
        element_type: &str,
    ) -> Option<&mut Box<dyn CustomElement>> {
        if !self.instances.contains_key(&id) {
            let factory = self.factories.get(element_type)?;
            let instance = factory.create(id);
            self.instances.insert(id, instance);
        }
        self.instances.get_mut(&id)
    }

    /// Called when Vue destroys an element.
    pub fn destroy(&mut self, id: u64) {
        if let Some(mut el) = self.instances.remove(&id) {
            el.destroy();
        }
    }

    /// Remove and destroy instances whose IDs no longer exist in the tree.
    pub fn prune_missing<F>(&mut self, mut is_live: F)
    where
        F: FnMut(u64) -> bool,
    {
        let stale_ids: Vec<u64> = self
            .instances
            .keys()
            .copied()
            .filter(|id| !is_live(*id))
            .collect();

        for id in stale_ids {
            self.destroy(id);
        }
    }

    /// Check if a type name has a registered factory.
    pub fn is_custom_type(&self, element_type: &str) -> bool {
        self.factories.contains_key(element_type)
    }
}
