//! Retained, GPU-painted canvas commands.
//!
//! Vue sends a command list only when the `commands` prop changes. Paths are
//! tessellated here once and retained by the element; GPUI only clones the
//! already-tessellated vertices while constructing a frame. There is no JS
//! callback and no N-API crossing in the paint loop.

#![allow(
    clippy::cast_possible_truncation,
    reason = "validated canvas numbers and bounded vertex indices narrow to GPUI's f32/u32 path representation"
)]

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Deserialize;

use super::{CustomElement, CustomElementFactory, CustomRenderContext};

pub struct CanvasFactory;

impl CustomElementFactory for CanvasFactory {
    fn element_type(&self) -> &'static str {
        "canvas"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(CanvasElement::default())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum PathOperation {
    MoveTo {
        x: f64,
        y: f64,
    },
    LineTo {
        x: f64,
        y: f64,
    },
    QuadraticTo {
        cx: f64,
        cy: f64,
        x: f64,
        y: f64,
    },
    BezierTo {
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    },
    ArcTo {
        rx: f64,
        ry: f64,
        #[serde(default)]
        rotation: f64,
        #[serde(default)]
        large_arc: bool,
        #[serde(default)]
        sweep: bool,
        x: f64,
        y: f64,
    },
    Close,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum CanvasCommand {
    Path {
        operations: Vec<PathOperation>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width")]
        stroke_width: f64,
        #[serde(default)]
        dash: Vec<f64>,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[serde(default)]
        radius: f64,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width")]
        stroke_width: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        radius: f64,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width")]
        stroke_width: f64,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke: String,
        #[serde(default = "default_stroke_width")]
        stroke_width: f64,
        #[serde(default)]
        dash: Vec<f64>,
    },
    Polyline {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        closed: bool,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width")]
        stroke_width: f64,
        #[serde(default)]
        dash: Vec<f64>,
    },
}

fn default_stroke_width() -> f64 {
    1.0
}

#[derive(Clone, Debug)]
enum PaintItem {
    Path(gpui::Path<gpui::Pixels>, gpui::Hsla),
    Quad {
        bounds: gpui::Bounds<gpui::Pixels>,
        radius: gpui::Pixels,
        fill: gpui::Hsla,
        stroke: gpui::Hsla,
        stroke_width: gpui::Pixels,
    },
}

#[derive(Default)]
struct TranslatedPaintCache {
    origin: Option<gpui::Point<gpui::Pixels>>,
    source_revision: u64,
    items: Arc<[PaintItem]>,
}

#[derive(Default)]
pub struct CanvasElement {
    source: serde_json::Value,
    gpu_source: Option<u32>,
    paint_items: Arc<[PaintItem]>,
    source_revision: u64,
    translated: Arc<Mutex<TranslatedPaintCache>>,
}

impl CanvasElement {
    fn set_commands(&mut self, value: serde_json::Value) {
        if self.source == value {
            return;
        }
        self.source = value.clone();
        self.source_revision = self.source_revision.wrapping_add(1);
        match compile_commands(value) {
            Ok(items) => self.paint_items = items.into(),
            Err(error) => {
                self.paint_items = Arc::default();
                log::warn!("Invalid canvas commands: {error}");
            }
        }
    }
}

impl CustomElement for CanvasElement {
    #[allow(
        clippy::too_many_lines,
        reason = "event wiring is a linear declarative table for one retained canvas element"
    )]
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<crate::renderer::GpuiView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let gpu_source = self.gpu_source;
        let canvas_frames = ctx.canvas_frames.clone();
        let paint_items = self.paint_items.clone();
        let source_revision = self.source_revision;
        let translated_cache = self.translated.clone();
        let drawing = gpui::canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                if let Some(frame) = gpu_source.and_then(|id| canvas_frames.lock().gpu_frame(id)) {
                    #[cfg(target_os = "macos")]
                    window.paint_metal_texture(
                        bounds,
                        frame.paint.clone(),
                        frame.opaque,
                        frame.clone(),
                    );
                    #[cfg(any(
                        target_os = "linux",
                        target_os = "freebsd",
                        target_family = "wasm"
                    ))]
                    window.paint_gpu_texture(
                        bounds,
                        frame.paint.clone(),
                        gpui::size(
                            gpui::DevicePixels(frame.texture.width().cast_signed()),
                            gpui::DevicePixels(frame.texture.height().cast_signed()),
                        ),
                        frame.opaque,
                        Some(frame.clone()),
                    );
                }
                let image = gpu_source.and_then(|id| canvas_frames.lock().image(id));
                if let Some(image) = image
                    && let Err(error) = window.paint_image(
                        bounds,
                        bounds,
                        gpui::Corners::default(),
                        image,
                        0,
                        false,
                    )
                {
                    log::warn!("canvas paint failed: {error}");
                }
                let translated = {
                    let mut cache = translated_cache.lock();
                    if cache.origin != Some(bounds.origin)
                        || cache.source_revision != source_revision
                    {
                        cache.items = paint_items
                            .iter()
                            .cloned()
                            .map(|mut item| {
                                match &mut item {
                                    PaintItem::Path(path, _) => {
                                        translate_path(path, bounds.origin);
                                    }
                                    PaintItem::Quad {
                                        bounds: quad_bounds,
                                        ..
                                    } => quad_bounds.origin += bounds.origin,
                                }
                                item
                            })
                            .collect::<Vec<_>>()
                            .into();
                        cache.origin = Some(bounds.origin);
                        cache.source_revision = source_revision;
                    }
                    cache.items.clone()
                };
                for item in translated.iter() {
                    match item {
                        PaintItem::Path(path, color) => {
                            window.paint_path(path.clone(), *color);
                        }
                        PaintItem::Quad {
                            bounds: quad_bounds,
                            radius,
                            fill,
                            stroke,
                            stroke_width,
                        } => {
                            window.paint_quad(gpui::quad(
                                *quad_bounds,
                                *radius,
                                *fill,
                                *stroke_width,
                                *stroke,
                                gpui::BorderStyle::default(),
                            ));
                        }
                    }
                }
            },
        )
        .absolute()
        .size_full();

        let element_id = ctx.id;
        let mut container = gpui::div()
            .id(super::custom_element_id("__gpui_vue_canvas", element_id))
            .relative()
            .child(crate::automation::bounds_tracker(element_id, None))
            .child(drawing)
            .children(ctx.children);
        if let Some(style) = ctx.style {
            container = crate::renderer::apply_interactive_styles(container, style);
        }
        if let Some(handle) = ctx.focus_handle {
            container = container.track_focus(handle);
        }

        if ctx.events.contains("click") {
            let callback = ctx.event_callback.clone();
            container = container.on_click(move |event, _, _| {
                crate::renderer::emit_event_full(&callback, element_id, "click", |payload| {
                    payload.x = Some(f64::from(f32::from(event.position().x)));
                    payload.y = Some(f64::from(f32::from(event.position().y)));
                    payload.click_count = Some(event.click_count() as u32);
                    payload.is_right_click = Some(event.is_right_click());
                    payload.modifiers = Some(event.modifiers().into());
                });
            });
        }
        if ctx.events.contains("mouseMove") {
            let callback = ctx.event_callback.clone();
            container = container.on_mouse_move(move |event, _, _| {
                crate::renderer::emit_event_full(&callback, element_id, "mouseMove", |payload| {
                    payload.x = Some(f64::from(f32::from(event.position.x)));
                    payload.y = Some(f64::from(f32::from(event.position.y)));
                    payload.pressed_button = event.pressed_button.map(mouse_button);
                    payload.modifiers = Some(event.modifiers.into());
                });
            });
        }
        if ctx.events.contains("mouseDown") {
            for button in [
                gpui::MouseButton::Left,
                gpui::MouseButton::Middle,
                gpui::MouseButton::Right,
            ] {
                let callback = ctx.event_callback.clone();
                container = container.on_mouse_down(button, move |event, _, _| {
                    crate::renderer::emit_event_full(
                        &callback,
                        element_id,
                        "mouseDown",
                        |payload| {
                            payload.x = Some(f64::from(f32::from(event.position.x)));
                            payload.y = Some(f64::from(f32::from(event.position.y)));
                            payload.button = Some(mouse_button(event.button));
                            payload.click_count = Some(event.click_count as u32);
                            payload.modifiers = Some(event.modifiers.into());
                        },
                    );
                });
            }
        }
        if ctx.events.contains("mouseUp") {
            for button in [
                gpui::MouseButton::Left,
                gpui::MouseButton::Middle,
                gpui::MouseButton::Right,
            ] {
                let callback = ctx.event_callback.clone();
                container = container.on_mouse_up(button, move |event, _, _| {
                    crate::renderer::emit_event_full(&callback, element_id, "mouseUp", |payload| {
                        payload.x = Some(f64::from(f32::from(event.position.x)));
                        payload.y = Some(f64::from(f32::from(event.position.y)));
                        payload.button = Some(mouse_button(event.button));
                        payload.click_count = Some(event.click_count as u32);
                        payload.modifiers = Some(event.modifiers.into());
                    });
                });
            }
        }
        if ctx.events.contains("mouseEnter") || ctx.events.contains("mouseLeave") {
            let enter = ctx
                .events
                .contains("mouseEnter")
                .then(|| ctx.event_callback.clone())
                .flatten();
            let leave = ctx
                .events
                .contains("mouseLeave")
                .then(|| ctx.event_callback.clone())
                .flatten();
            container = container.on_hover(move |&hovered, _, _| {
                let (callback, event_type) = if hovered {
                    (&enter, "mouseEnter")
                } else {
                    (&leave, "mouseLeave")
                };
                crate::renderer::emit_event_full(callback, element_id, event_type, |payload| {
                    payload.hovered = Some(hovered);
                });
            });
        }
        if ctx.events.contains("mouseDownOutside") {
            let callback = ctx.event_callback.clone();
            container = container.on_mouse_down_out(move |event, _, _| {
                crate::renderer::emit_event_full(
                    &callback,
                    element_id,
                    "mouseDownOutside",
                    |payload| {
                        payload.x = Some(f64::from(f32::from(event.position.x)));
                        payload.y = Some(f64::from(f32::from(event.position.y)));
                        payload.button = Some(mouse_button(event.button));
                        payload.modifiers = Some(event.modifiers.into());
                    },
                );
            });
        }
        if ctx.events.contains("scroll") {
            let callback = ctx.event_callback.clone();
            container = container.on_scroll_wheel(move |event, _, _| {
                let delta = event.delta.pixel_delta(gpui::px(20.0));
                crate::renderer::emit_event_full(&callback, element_id, "scroll", |payload| {
                    payload.x = Some(f64::from(f32::from(event.position.x)));
                    payload.y = Some(f64::from(f32::from(event.position.y)));
                    payload.delta_x = Some(f64::from(f32::from(delta.x)));
                    payload.delta_y = Some(f64::from(f32::from(delta.y)));
                    payload.precise = Some(event.delta.precise());
                    payload.modifiers = Some(event.modifiers.into());
                });
            });
        }
        if ctx.events.contains("keyDown") {
            let callback = ctx.event_callback.clone();
            container = container.on_key_down(move |event, _, _| {
                crate::renderer::emit_event_full(&callback, element_id, "keyDown", |payload| {
                    payload.key = Some(event.keystroke.key.clone());
                    payload.key_char.clone_from(&event.keystroke.key_char);
                    payload.is_held = Some(event.is_held);
                    payload.modifiers = Some(event.keystroke.modifiers.into());
                });
            });
        }
        if ctx.events.contains("keyUp") {
            let callback = ctx.event_callback.clone();
            container = container.on_key_up(move |event, _, _| {
                crate::renderer::emit_event_full(&callback, element_id, "keyUp", |payload| {
                    payload.key = Some(event.keystroke.key.clone());
                    payload.key_char.clone_from(&event.keystroke.key_char);
                    payload.modifiers = Some(event.keystroke.modifiers.into());
                });
            });
        }

        container.into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        if key == "source" {
            self.gpu_source = value.as_u64().and_then(|id| u32::try_from(id).ok());
        } else if key == "commands" {
            self.set_commands(value);
        }
    }

    fn supported_props(&self) -> &'static [&'static str] {
        &["commands", "source"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &[
            "click",
            "mouseDown",
            "mouseUp",
            "mouseMove",
            "mouseEnter",
            "mouseLeave",
            "mouseDownOutside",
            "scroll",
            "keyDown",
            "keyUp",
            "focus",
            "blur",
        ]
    }

    fn destroy(&mut self) {
        self.gpu_source = None;
        self.paint_items = Arc::default();
        self.translated.lock().items = Arc::default();
    }
}

fn mouse_button(button: gpui::MouseButton) -> u32 {
    match button {
        gpui::MouseButton::Left => 0,
        gpui::MouseButton::Middle => 1,
        gpui::MouseButton::Right => 2,
        gpui::MouseButton::Navigate(_) => 3,
    }
}

fn translate_path(path: &mut gpui::Path<gpui::Pixels>, offset: gpui::Point<gpui::Pixels>) {
    path.bounds.origin += offset;
    for vertex in &mut path.vertices {
        vertex.xy_position += offset;
    }
}

fn compile_commands(value: serde_json::Value) -> Result<Vec<PaintItem>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let commands: Vec<CanvasCommand> =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if commands.len() > 100_000 {
        return Err("canvas accepts at most 100,000 commands".to_string());
    }

    let mut items = Vec::with_capacity(commands.len());
    for command in commands {
        compile_command(command, &mut items)?;
    }
    Ok(items)
}

#[allow(
    clippy::too_many_lines,
    reason = "canvas command variants form one exhaustive validation and compilation table"
)]
fn compile_command(command: CanvasCommand, items: &mut Vec<PaintItem>) -> Result<(), String> {
    match command {
        CanvasCommand::Path {
            operations,
            fill,
            stroke,
            stroke_width,
            dash,
        } => compile_operations(
            &operations,
            fill.as_deref(),
            stroke.as_deref(),
            stroke_width,
            &dash,
            items,
        ),
        CanvasCommand::Rect {
            x,
            y,
            width,
            height,
            radius,
            fill,
            stroke,
            stroke_width,
        } => {
            validate_numbers(&[x, y, width, height, radius, stroke_width])?;
            if width < 0.0 || height < 0.0 || radius < 0.0 || stroke_width < 0.0 {
                return Err("canvas rectangle sizes must be non-negative".to_string());
            }
            items.push(PaintItem::Quad {
                bounds: gpui::Bounds::new(
                    gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                    gpui::size(gpui::px(width as f32), gpui::px(height as f32)),
                ),
                radius: gpui::px(radius as f32),
                fill: parse_color(fill.as_deref(), true)?,
                stroke: parse_color(stroke.as_deref(), true)?,
                stroke_width: gpui::px(if stroke.is_some() {
                    stroke_width as f32
                } else {
                    0.0
                }),
            });
            Ok(())
        }
        CanvasCommand::Circle {
            cx,
            cy,
            radius,
            fill,
            stroke,
            stroke_width,
        } => {
            validate_numbers(&[cx, cy, radius, stroke_width])?;
            if radius < 0.0 || stroke_width < 0.0 {
                return Err("canvas circle radius and strokeWidth must be non-negative".to_string());
            }
            let operations = circle_operations(cx, cy, radius);
            compile_operations(
                &operations,
                fill.as_deref(),
                stroke.as_deref(),
                stroke_width,
                &[],
                items,
            )
        }
        CanvasCommand::Line {
            x1,
            y1,
            x2,
            y2,
            stroke,
            stroke_width,
            dash,
        } => compile_operations(
            &[
                PathOperation::MoveTo { x: x1, y: y1 },
                PathOperation::LineTo { x: x2, y: y2 },
            ],
            None,
            Some(stroke.as_str()),
            stroke_width,
            &dash,
            items,
        ),
        CanvasCommand::Polyline {
            points,
            closed,
            fill,
            stroke,
            stroke_width,
            dash,
        } => {
            let mut operations = Vec::with_capacity(points.len() + usize::from(closed));
            for (index, [x, y]) in points.into_iter().enumerate() {
                operations.push(if index == 0 {
                    PathOperation::MoveTo { x, y }
                } else {
                    PathOperation::LineTo { x, y }
                });
            }
            if closed {
                operations.push(PathOperation::Close);
            }
            compile_operations(
                &operations,
                fill.as_deref(),
                stroke.as_deref(),
                stroke_width,
                &dash,
                items,
            )
        }
    }
}

fn compile_operations(
    operations: &[PathOperation],
    fill: Option<&str>,
    stroke: Option<&str>,
    stroke_width: f64,
    dash: &[f64],
    items: &mut Vec<PaintItem>,
) -> Result<(), String> {
    if operations.len() > 1_000_000 {
        return Err("a canvas path accepts at most 1,000,000 operations".to_string());
    }
    validate_numbers(&[stroke_width])?;
    validate_numbers(dash)?;
    if stroke_width < 0.0 || dash.iter().any(|value| *value < 0.0) {
        return Err("canvas strokeWidth and dash values must be non-negative".to_string());
    }
    if let Some(color) = fill {
        let mut builder = gpui::PathBuilder::fill();
        apply_operations(&mut builder, operations)?;
        if let Ok(path) = builder.build() {
            items.push(PaintItem::Path(path, parse_color(Some(color), false)?));
        }
    }
    if let Some(color) = stroke {
        let mut builder = gpui::PathBuilder::stroke(gpui::px(stroke_width as f32));
        if !dash.is_empty() {
            let dash: Vec<_> = dash.iter().map(|value| gpui::px(*value as f32)).collect();
            builder = builder.dash_array(&dash);
        }
        apply_operations(&mut builder, operations)?;
        if let Ok(path) = builder.build() {
            items.push(PaintItem::Path(path, parse_color(Some(color), false)?));
        }
    }
    Ok(())
}

fn apply_operations(
    builder: &mut gpui::PathBuilder,
    operations: &[PathOperation],
) -> Result<(), String> {
    for operation in operations {
        match *operation {
            PathOperation::MoveTo { x, y } => {
                validate_numbers(&[x, y])?;
                builder.move_to(gpui::point(gpui::px(x as f32), gpui::px(y as f32)));
            }
            PathOperation::LineTo { x, y } => {
                validate_numbers(&[x, y])?;
                builder.line_to(gpui::point(gpui::px(x as f32), gpui::px(y as f32)));
            }
            PathOperation::QuadraticTo { cx, cy, x, y } => {
                validate_numbers(&[cx, cy, x, y])?;
                builder.curve_to(
                    gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                    gpui::point(gpui::px(cx as f32), gpui::px(cy as f32)),
                );
            }
            PathOperation::BezierTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => {
                validate_numbers(&[cp1x, cp1y, cp2x, cp2y, x, y])?;
                builder.cubic_bezier_to(
                    gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                    gpui::point(gpui::px(cp1x as f32), gpui::px(cp1y as f32)),
                    gpui::point(gpui::px(cp2x as f32), gpui::px(cp2y as f32)),
                );
            }
            PathOperation::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                validate_numbers(&[rx, ry, rotation, x, y])?;
                if rx < 0.0 || ry < 0.0 {
                    return Err("canvas arc radii must be non-negative".to_string());
                }
                builder.arc_to(
                    gpui::point(gpui::px(rx as f32), gpui::px(ry as f32)),
                    gpui::px(rotation as f32),
                    large_arc,
                    sweep,
                    gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
                );
            }
            PathOperation::Close => builder.close(),
        }
    }
    Ok(())
}

fn circle_operations(cx: f64, cy: f64, radius: f64) -> Vec<PathOperation> {
    vec![
        PathOperation::MoveTo {
            x: cx + radius,
            y: cy,
        },
        PathOperation::ArcTo {
            rx: radius,
            ry: radius,
            rotation: 0.0,
            large_arc: false,
            sweep: false,
            x: cx - radius,
            y: cy,
        },
        PathOperation::ArcTo {
            rx: radius,
            ry: radius,
            rotation: 0.0,
            large_arc: false,
            sweep: false,
            x: cx + radius,
            y: cy,
        },
        PathOperation::Close,
    ]
}

fn parse_color(value: Option<&str>, transparent_default: bool) -> Result<gpui::Hsla, String> {
    let Some(value) = value else {
        return if transparent_default {
            Ok(gpui::transparent_black())
        } else {
            Err("canvas paint color is required".to_string())
        };
    };
    crate::style::parse_color_hex(value)
        .map(|hex| gpui::rgba(hex).into())
        .ok_or_else(|| format!("invalid canvas color: {value}"))
}

fn validate_numbers(values: &[f64]) -> Result<(), String> {
    if values
        .iter()
        .any(|value| !value.is_finite() || value.abs() > f64::from(f32::MAX))
    {
        return Err("canvas coordinates must fit finite 32-bit floats".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_retained_shapes_and_paths() {
        let items = compile_commands(serde_json::json!([
            { "type": "rect", "x": 1, "y": 2, "width": 30, "height": 40, "fill": "#ff0000" },
            { "type": "path", "operations": [
                { "op": "moveTo", "x": 0, "y": 0 },
                { "op": "lineTo", "x": 20, "y": 20 }
            ], "stroke": "#00ff00", "strokeWidth": 2 }
        ]))
        .unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn rejects_non_finite_geometry() {
        let error = compile_commands(serde_json::json!([{
            "type": "rect", "x": 0, "y": 0, "width": f64::MAX,
            "height": 10, "fill": "#fff"
        }]))
        .unwrap_err();
        assert!(error.contains("finite"));
    }

    #[test]
    fn unchanged_commands_reuse_tessellated_storage() {
        let source = serde_json::json!([{
            "type": "line", "x1": 0, "y1": 0, "x2": 10, "y2": 10,
            "stroke": "#fff"
        }]);
        let mut canvas = CanvasElement::default();
        canvas.set_commands(source.clone());
        let storage = canvas.paint_items.as_ptr();
        canvas.set_commands(source);
        assert_eq!(canvas.paint_items.as_ptr(), storage);
    }
}
