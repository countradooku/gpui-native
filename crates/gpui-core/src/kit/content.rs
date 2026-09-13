//! Deferred overlay content uses the same retained builder as ordinary framework children.
use gpui::{AnyElement, App, Context, Render, Window};
use std::rc::Rc;
pub(crate) type ChildRenderer = Rc<dyn Fn(Option<usize>, &mut Window, &mut App) -> AnyElement>;
pub(crate) struct Content {
    pub(crate) render: ChildRenderer,
}
impl Render for Content {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        (self.render)(None, window, cx)
    }
}
