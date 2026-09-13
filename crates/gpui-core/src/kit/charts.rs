//! Kit's complete chart family, with labels in the shared native text pipeline.
use super::elements::{boolean, integer, number, string, surface};
use crate::{
    custom_elements::{
        CustomElement, CustomElementFactory, CustomElementRegistry, CustomRenderContext,
        custom_element_id,
    },
    renderer::GpuiView,
};
use gpui::{
    AnyElement, App, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId,
    LayoutId, Pixels, Window, prelude::*,
};
use gpui_component::{
    ActiveTheme as _,
    chart::{AreaChart, BarChart, CandlestickChart, LineChart, PieChart, RadarChart, SankeyChart},
    plot::label::{LabelPainter, with_label_painter},
};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};
pub(crate) const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "kit-line-chart",
        &[
            "data",
            "name",
            "stroke",
            "curve",
            "dot",
            "grid",
            "xAxis",
            "tickMargin",
        ],
    ),
    (
        "kit-area-chart",
        &[
            "data",
            "name",
            "stroke",
            "fill",
            "curve",
            "grid",
            "xAxis",
            "tickMargin",
        ],
    ),
    (
        "kit-bar-chart",
        &[
            "data",
            "name",
            "fill",
            "horizontal",
            "grid",
            "labelAxis",
            "valueAxis",
            "tickMargin",
        ],
    ),
    (
        "kit-pie-chart",
        &["data", "innerRadius", "outerRadius", "padAngle"],
    ),
    (
        "kit-candlestick-chart",
        &["data", "grid", "xAxis", "tickMargin", "bodyWidthRatio"],
    ),
    (
        "kit-radar-chart",
        &[
            "data",
            "name",
            "stroke",
            "fill",
            "grid",
            "gridLevels",
            "maxValue",
            "dot",
        ],
    ),
    (
        "kit-sankey-chart",
        &["data", "links", "nodeWidth", "nodePadding", "linkOpacity"],
    ),
];
pub(crate) fn register(registry: &mut CustomElementRegistry) {
    for &(name, props) in COMPONENTS {
        registry.register(Box::new(Factory { name, props }));
    }
}
struct Factory {
    name: &'static str,
    props: &'static [&'static str],
}
impl CustomElementFactory for Factory {
    fn element_type(&self) -> &str {
        self.name
    }
    fn create(&self, _: u64) -> Box<dyn CustomElement> {
        Box::new(Control {
            name: self.name,
            props: self.props,
        })
    }
}
struct Control {
    name: &'static str,
    props: &'static [&'static str],
}
#[derive(Clone)]
struct Datum {
    label: gpui::SharedString,
    value: f64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    color: gpui::Hsla,
}
fn color(value: Option<&Value>, default: gpui::Hsla) -> gpui::Hsla {
    value
        .and_then(Value::as_str)
        .and_then(crate::color::parse_color_rgba)
        .map_or(default, Into::into)
}
fn flag(ctx: &CustomRenderContext<'_>, key: &str) -> bool {
    ctx.props.get(key).and_then(Value::as_bool).unwrap_or(true)
}
fn painter(ctx: &CustomRenderContext<'_>, index: Rc<Cell<usize>>) -> LabelPainter {
    let selection = ctx.selection.clone();
    let highlight = ctx.highlight_set.clone();
    let selectable = ctx.selectable;
    let wash = ctx.selection_wash;
    let id = ctx.id;
    Rc::new(move |text, line, origin, height, align, window, cx| {
        let sub = index.get();
        index.set(sub + 1);
        crate::text::paint::shaped_text(
            &crate::text::SelectableText {
                selectable,
                highlight: highlight.clone().map(crate::text::HighlightSource::Native),
                ..crate::text::SelectableText::new(id, sub, text, None, selection.clone(), wash)
            },
            line,
            origin,
            height,
            align,
            window,
            cx,
        );
    })
}
/// Preserve Kit's element state and tooltips while scoping its canvas text painter.
struct Chart<P: Element> {
    plot: P,
    painter: LabelPainter,
    index: Rc<Cell<usize>>,
}
impl<P: Element> IntoElement for Chart<P> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl<P: Element> Element for Chart<P> {
    type RequestLayoutState = P::RequestLayoutState;
    type PrepaintState = P::PrepaintState;
    fn id(&self) -> Option<ElementId> {
        self.plot.id()
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.plot.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.plot
            .prepaint(id, inspector, bounds, layout, window, cx)
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.index.set(0);
        with_label_painter(self.painter.clone(), || {
            self.plot
                .paint(id, inspector, bounds, layout, prepaint, window, cx);
        });
    }
}
fn chart<P: Element>(plot: P, ctx: &CustomRenderContext<'_>) -> AnyElement {
    let index = Rc::new(Cell::new(0));
    Chart {
        plot,
        painter: painter(ctx, index.clone()),
        index,
    }
    .into_any_element()
}
impl CustomElement for Control {
    fn set_prop(&mut self, _: &str, _: Value) {}
    fn supported_props(&self) -> &'static [&'static str] {
        self.props
    }
    fn supported_events(&self) -> &'static [&'static str] {
        &[
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
    fn destroy(&mut self) {}
    #[allow(
        clippy::too_many_lines,
        reason = "each native builder stays beside its prop and event mapping"
    )]
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        _: &mut Window,
        cx: &mut Context<GpuiView>,
    ) -> AnyElement {
        let default = *cx.theme().tokens.primary;
        let stroke = color(ctx.props.get("stroke"), default);
        let fill = color(ctx.props.get("fill"), default);
        let data = ctx
            .props
            .get("data")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |data| {
                data.iter()
                    .map(|value| {
                        let number = |key| value.get(key).and_then(Value::as_f64).unwrap_or(0.);
                        Datum {
                            label: value
                                .get("label")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned()
                                .into(),
                            value: number("value"),
                            open: number("open"),
                            high: number("high"),
                            low: number("low"),
                            close: number("close"),
                            color: color(value.get("color"), fill),
                        }
                    })
                    .collect::<Vec<_>>()
            });
        if data.is_empty() {
            return surface(&ctx).into_any_element();
        }
        let id = custom_element_id("gpui-kit", ctx.id);
        let ticks = integer(&ctx, "tickMargin", 1);
        let plot = match self.name {
            "kit-line-chart" => {
                let mut plot = LineChart::new(data)
                    .id(id)
                    .name(string(&ctx, "name"))
                    .x(|v: &Datum| v.label.clone())
                    .y(|v| v.value)
                    .stroke(stroke)
                    .grid(flag(&ctx, "grid"))
                    .x_axis(flag(&ctx, "xAxis"))
                    .tick_margin(ticks);
                plot = match string(&ctx, "curve").as_str() {
                    "natural" => plot.natural(),
                    "step" => plot.step_after(),
                    _ => plot.linear(),
                };
                if boolean(&ctx, "dot") {
                    plot = plot.dot();
                }
                chart(plot, &ctx)
            }
            "kit-area-chart" => {
                let mut plot = AreaChart::new(data)
                    .id(id)
                    .name(string(&ctx, "name"))
                    .x(|v: &Datum| v.label.clone())
                    .y(|v| v.value)
                    .stroke(stroke)
                    .fill(fill)
                    .grid(flag(&ctx, "grid"))
                    .x_axis(flag(&ctx, "xAxis"))
                    .tick_margin(ticks);
                plot = match string(&ctx, "curve").as_str() {
                    "natural" => plot.natural(),
                    "step" => plot.step_after(),
                    _ => plot.linear(),
                };
                chart(plot, &ctx)
            }
            "kit-bar-chart" => chart(
                BarChart::new(data)
                    .id(id)
                    .name(string(&ctx, "name"))
                    .band(|v: &Datum| v.label.clone())
                    .value(|v| v.value)
                    .fill(|v, _, _, _| v.color)
                    .grid(flag(&ctx, "grid"))
                    .label_axis(flag(&ctx, "labelAxis"))
                    .value_axis(flag(&ctx, "valueAxis"))
                    .tick_margin(ticks)
                    .alignment(if boolean(&ctx, "horizontal") {
                        gpui_component::plot::shape::BarAlignment::Left
                    } else {
                        gpui_component::plot::shape::BarAlignment::Bottom
                    }),
                &ctx,
            ),
            "kit-pie-chart" => {
                let mut plot = PieChart::new(data)
                    .value(|v: &Datum| {
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "chart data is validated to fit f32"
                        )]
                        {
                            v.value as f32
                        }
                    })
                    .label(|v| v.label.clone())
                    .color(|v| v.color)
                    .inner_radius(number(&ctx, "innerRadius", 0.))
                    .pad_angle(number(&ctx, "padAngle", 0.));
                if ctx.props.contains_key("outerRadius") {
                    plot = plot.outer_radius(number(&ctx, "outerRadius", 1.));
                }
                chart(plot, &ctx)
            }
            "kit-candlestick-chart" => chart(
                CandlestickChart::new(data)
                    .x(|v: &Datum| v.label.clone())
                    .open(|v| v.open)
                    .high(|v| v.high)
                    .low(|v| v.low)
                    .close(|v| v.close)
                    .grid(flag(&ctx, "grid"))
                    .x_axis(flag(&ctx, "xAxis"))
                    .tick_margin(ticks)
                    .body_width_ratio(number(&ctx, "bodyWidthRatio", 0.7)),
                &ctx,
            ),
            "kit-radar-chart" => {
                let mut plot = RadarChart::new(data)
                    .id(id)
                    .name(string(&ctx, "name"))
                    .value(|v: &Datum| v.value)
                    .label(|v| v.label.clone())
                    .stroke(stroke)
                    .fill(fill)
                    .grid(flag(&ctx, "grid"))
                    .grid_levels(integer(&ctx, "gridLevels", 5));
                if ctx.props.contains_key("maxValue") {
                    plot = plot.max_value(f64::from(number(&ctx, "maxValue", 100.)));
                }
                if boolean(&ctx, "dot") {
                    plot = plot.dot();
                }
                chart(plot, &ctx)
            }
            "kit-sankey-chart" => {
                let links = ctx
                    .props
                    .get("links")
                    .and_then(Value::as_array)
                    .map_or_else(Vec::new, |links| {
                        links
                            .iter()
                            .map(|v| {
                                gpui_component::plot::shape::SankeyLink::new(
                                    v.get("source")
                                        .and_then(Value::as_u64)
                                        .and_then(|v| usize::try_from(v).ok())
                                        .unwrap_or(0),
                                    v.get("target")
                                        .and_then(Value::as_u64)
                                        .and_then(|v| usize::try_from(v).ok())
                                        .unwrap_or(0),
                                    v.get("value").and_then(Value::as_f64).unwrap_or(0.),
                                )
                            })
                            .collect()
                    });
                chart(
                    SankeyChart::new(data, links)
                        .node_label(|v: &Datum| v.label.clone())
                        .node_color(|v| v.color)
                        .node_width(number(&ctx, "nodeWidth", 24.))
                        .node_padding(number(&ctx, "nodePadding", 8.))
                        .link_opacity(number(&ctx, "linkOpacity", 0.4)),
                    &ctx,
                )
            }
            _ => unreachable!("registered chart family"),
        };
        surface(&ctx).child(plot).into_any_element()
    }
}
