//! Virtualized Kit data tables backed by framework-owned row and column data.
use super::elements::{boolean, change, size};
use crate::custom_elements::{
    CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
};
use crate::renderer::{EventCallback, GpuiView};
use gpui::{AnyElement, App, Context, Entity, Subscription, Window, prelude::*};
use gpui_component::{
    Sizable as _,
    table::{Column, ColumnSort, DataTable, TableDelegate, TableEvent, TableState},
};
use serde_json::{Value, json};

pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[(
    "kit-data-table",
    &["columns", "rows", "stripe", "bordered", "loading", "size"],
)];
pub(crate) fn register(registry: &mut CustomElementRegistry) {
    registry.register(Box::new(Factory));
}
struct Factory;
impl CustomElementFactory for Factory {
    fn element_type(&self) -> &'static str {
        "kit-data-table"
    }
    fn create(&self, _: u64) -> Box<dyn CustomElement> {
        Box::new(Control {
            state: None,
            subscription: None,
            dirty: true,
        })
    }
}
struct Control {
    state: Option<Entity<TableState<TableData>>>,
    subscription: Option<Subscription>,
    dirty: bool,
}
struct TableData {
    columns: Vec<Value>,
    rows: Vec<Value>,
    id: u64,
    callback: Option<EventCallback>,
    loading: bool,
    selection: crate::text::SharedSelection,
    selectable: bool,
    wash: gpui::Hsla,
    highlight: Option<std::sync::Arc<crate::text::HighlightContext>>,
}
impl TableData {
    fn from_context(ctx: &CustomRenderContext<'_>) -> Self {
        Self {
            columns: ctx
                .props
                .get("columns")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            rows: ctx
                .props
                .get("rows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            id: ctx.id,
            callback: ctx.event_callback.clone(),
            loading: boolean(ctx, "loading"),
            selection: ctx.selection.clone(),
            selectable: ctx.selectable,
            wash: ctx.selection_wash,
            highlight: ctx.highlight_set.clone(),
        }
    }
    fn text(&self, sub: usize, text: String) -> AnyElement {
        crate::text::selectable_text(crate::text::SelectableText {
            selectable: self.selectable,
            highlight: self
                .highlight
                .clone()
                .map(crate::text::HighlightSource::Native),
            ..crate::text::SelectableText::new(
                self.id,
                sub,
                text.into(),
                None,
                self.selection.clone(),
                self.wash,
            )
        })
    }
    fn cell(&self, row: usize, column: usize) -> String {
        let key = self
            .columns
            .get(column)
            .and_then(|v| v.get("key"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        match self.rows.get(row).and_then(|v| v.get(key)) {
            Some(Value::String(v)) => v.clone(),
            Some(Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
        }
    }
}
impl TableDelegate for TableData {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }
    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }
    fn column(&self, index: usize, _: &App) -> Column {
        let value = &self.columns[index];
        let key = value
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let label = value
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(&key)
            .to_owned();
        let mut column = Column::new(key, label).resizable(
            value
                .get("resizable")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        );
        if value.get("sortable").and_then(Value::as_bool) == Some(true) {
            column = column.sortable();
        }
        if value.get("fixed").and_then(Value::as_bool) == Some(true) {
            column = column.fixed_left();
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "validated column width uses GPUI f32 geometry"
        )]
        if let Some(width) = value.get("width").and_then(Value::as_f64) {
            column = column.width(gpui::px(width as f32));
        }
        column
    }
    fn render_th(
        &mut self,
        index: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        self.text(
            index,
            self.columns[index]
                .get("label")
                .or_else(|| self.columns[index].get("key"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
        )
    }
    fn render_td(
        &mut self,
        row: usize,
        column: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        self.text(
            (row + 1) * self.columns.len() + column,
            self.cell(row, column),
        )
    }
    fn cell_text(&self, row: usize, column: usize, _: &App) -> String {
        self.cell(row, column)
    }
    fn loading(&self, _: &App) -> bool {
        self.loading
    }
    fn perform_sort(
        &mut self,
        column: usize,
        sort: ColumnSort,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        let direction = match sort {
            ColumnSort::Default => "none",
            ColumnSort::Ascending => "ascending",
            ColumnSort::Descending => "descending",
        };
        change(
            &self.callback,
            self.id,
            json!({"type":"sort","column":column,"direction":direction}),
        );
    }
    fn visible_rows_changed(
        &mut self,
        range: std::ops::Range<usize>,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) {
        crate::renderer::emit_event_full(&self.callback, self.id, "visibleRange", |event| {
            #[allow(
                clippy::cast_precision_loss,
                reason = "row indices are bounded by JavaScript array lengths"
            )]
            {
                event.start_index = Some(range.start as f64);
                event.end_index = Some(range.end as f64);
            }
        });
    }
}
impl CustomElement for Control {
    fn native_focus_handle(&self, cx: &gpui::App) -> Option<gpui::FocusHandle> {
        use gpui::Focusable as _;
        Some(self.state.as_ref()?.focus_handle(cx))
    }
    fn set_prop(&mut self, _: &str, _: Value) {
        self.dirty = true;
    }
    fn supported_props(&self) -> &'static [&'static str] {
        COMPONENTS[0].1
    }
    fn supported_events(&self) -> &'static [&'static str] {
        &[
            "change",
            "visibleRange",
            "mouseEnter",
            "mouseLeave",
            "keyDown",
            "keyUp",
            "focus",
            "blur",
            "click",
            "auxClick",
            "mouseDown",
            "mouseUp",
            "mouseMove",
            "mouseDownOutside",
            "scroll",
            "highlight",
        ]
    }
    fn destroy(&mut self) {
        self.subscription = None;
        self.state = None;
    }
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        window: &mut Window,
        cx: &mut Context<GpuiView>,
    ) -> AnyElement {
        if self.state.is_none() {
            let state = cx.new(|cx| TableState::new(TableData::from_context(&ctx), window, cx));
            let callback = ctx.event_callback.clone();
            let id = ctx.id;
            self.subscription=Some(cx.subscribe(&state,move |_,_,event,cx|{
                let value=match event{
                    TableEvent::SelectRow(row)=>json!({"type":"selectRow","row":row}),
                    TableEvent::DoubleClickedRow(row)=>json!({"type":"doubleClickRow","row":row}),
                    TableEvent::SelectColumn(column)=>json!({"type":"selectColumn","column":column}),
                    TableEvent::SelectCell(row,column)=>json!({"type":"selectCell","row":row,"column":column}),
                    TableEvent::DoubleClickedCell(row,column)=>json!({"type":"doubleClickCell","row":row,"column":column}),
                    TableEvent::ColumnWidthsChanged(widths)=>json!({"type":"columnWidths","widths":widths.iter().map(|v|f32::from(*v)).collect::<Vec<_>>()}),
                    TableEvent::MoveColumn(from,to)=>json!({"type":"moveColumn","from":from,"to":to}),
                    TableEvent::RightClickedRow(row)=>json!({"type":"contextRow","row":row}),
                    TableEvent::RightClickedCell(row,column)=>json!({"type":"contextCell","row":row,"column":column}),
                    TableEvent::ClearSelection=>json!({"type":"clearSelection"}),
                };change(&callback,id,value);cx.notify();
            }));
            self.state = Some(state);
        }
        let Some(state) = &self.state else {
            return gpui::Empty.into_any_element();
        };
        if self.dirty {
            state.update(cx, |state, cx| {
                *state.delegate_mut() = TableData::from_context(&ctx);
                state.refresh(cx);
            });
            self.dirty = false;
        }
        state.update(cx, |state, _| {
            let data = state.delegate_mut();
            data.selection = ctx.selection.clone();
            data.selectable = ctx.selectable;
            data.wash = ctx.selection_wash;
            data.highlight.clone_from(&ctx.highlight_set);
        });
        let component = DataTable::new(state)
            .stripe(boolean(&ctx, "stripe"))
            .bordered(boolean(&ctx, "bordered"))
            .with_size(size(&ctx));
        super::elements::surface(&ctx)
            .child(component)
            .into_any_element()
    }
}
