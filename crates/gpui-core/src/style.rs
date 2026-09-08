#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "CSS numbers arrive as serde f64 values and are normalized into GPUI f32 geometry"
)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Font weight value — accepts both CSS strings ("bold", "700") and numbers (700).
/// JS style objects commonly use both `fontWeight: "bold"` and `fontWeight: 700`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FontWeightValue {
    Num(f64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxShadowValue {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur_radius: f64,
    pub spread_radius: f64,
    pub color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayValue {
    Flex,
    Grid,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositionValue {
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverflowValue {
    Hidden,
    Scroll,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectValue {
    None,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerEventsValue {
    None,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlexDirectionValue {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlexWrapValue {
    Wrap,
    WrapReverse,
    NoWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlignValue {
    Center,
    Start,
    End,
    Between,
    Around,
    Evenly,
    Stretch,
    Baseline,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextAlignValue {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WhiteSpaceValue {
    Normal,
    NoWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextOverflowValue {
    Ellipsis,
    EllipsisStart,
}

pub(crate) fn parse_font_weight(value: &FontWeightValue) -> gpui::FontWeight {
    match value {
        FontWeightValue::Num(number) => gpui::FontWeight((*number as f32).clamp(1.0, 1000.0)),
        FontWeightValue::Str(value) => {
            let lower = value.trim().to_ascii_lowercase();
            match lower.as_str() {
                "100" | "thin" => gpui::FontWeight(100.0),
                "200" | "extralight" | "extra-light" => gpui::FontWeight(200.0),
                "300" | "light" => gpui::FontWeight(300.0),
                "400" | "normal" => gpui::FontWeight(400.0),
                "500" | "medium" => gpui::FontWeight(500.0),
                "600" | "semibold" | "semi-bold" => gpui::FontWeight(600.0),
                "700" | "bold" => gpui::FontWeight(700.0),
                "800" | "extrabold" | "extra-bold" => gpui::FontWeight(800.0),
                "900" | "black" => gpui::FontWeight(900.0),
                _ => lower
                    .parse::<f32>()
                    .map_or(gpui::FontWeight::NORMAL, |number| {
                        gpui::FontWeight(number.clamp(1.0, 1000.0))
                    }),
            }
        }
    }
}

/// Renderer-ready values computed once when a style enters the intern table.
#[doc(hidden)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedStyle {
    initialized: bool,
    background: Option<gpui::Rgba>,
    color: Option<gpui::Rgba>,
    border_color: Option<gpui::Rgba>,
    shadow_color: Option<gpui::Rgba>,
    selection_color: Option<gpui::Rgba>,
    cursor: Option<gpui::CursorStyle>,
    pub(crate) display: Option<DisplayValue>,
    pub(crate) position: Option<PositionValue>,
    pub(crate) overflow_x: Option<OverflowValue>,
    pub(crate) overflow_y: Option<OverflowValue>,
    pub(crate) user_select: Option<SelectValue>,
    pub(crate) pointer_events: Option<PointerEventsValue>,
    pub(crate) flex_direction: Option<FlexDirectionValue>,
    pub(crate) flex_wrap: Option<FlexWrapValue>,
    pub(crate) align_items: Option<AlignValue>,
    pub(crate) align_content: Option<AlignValue>,
    pub(crate) justify_content: Option<AlignValue>,
    pub(crate) align_self: Option<AlignValue>,
    pub(crate) text_align: Option<TextAlignValue>,
    pub(crate) white_space: Option<WhiteSpaceValue>,
    pub(crate) text_overflow: Option<TextOverflowValue>,
    pub(crate) font_weight: Option<gpui::FontWeight>,
    pub(crate) visible: Option<bool>,
}

/// A dimension value that can be a number (pixels) or a string (percentage, auto, etc.)
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DimensionValue {
    Pixels(f64),
    Percentage(f64), // 0.0 to 1.0
    #[default]
    Auto,
}

impl Serialize for DimensionValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Pixels(value) => serializer.serialize_f64(*value),
            Self::Percentage(value) => serializer.serialize_str(&format!("{}%", value * 100.0)),
            Self::Auto => serializer.serialize_str("auto"),
        }
    }
}

impl<'de> Deserialize<'de> for DimensionValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct DimensionVisitor;

        impl Visitor<'_> for DimensionVisitor {
            type Value = DimensionValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a number or a string like '100%' or 'auto'")
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.is_finite() {
                    Ok(DimensionValue::Pixels(v))
                } else {
                    Err(de::Error::custom("dimension must be finite"))
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DimensionValue::Pixels(v as f64))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DimensionValue::Pixels(v as f64))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v == "auto" {
                    Ok(DimensionValue::Auto)
                } else if v.ends_with('%') {
                    let num_str = v.trim_end_matches('%');
                    match num_str.parse::<f64>() {
                        Ok(n) if n.is_finite() => Ok(DimensionValue::Percentage(n / 100.0)),
                        Err(_) => Err(de::Error::custom(format!("invalid percentage: {v}"))),
                        Ok(_) => Err(de::Error::custom("percentage must be finite")),
                    }
                } else {
                    // Try to parse as a number
                    match v.parse::<f64>() {
                        Ok(n) if n.is_finite() => Ok(DimensionValue::Pixels(n)),
                        Err(_) => Err(de::Error::custom(format!("invalid dimension: {v}"))),
                        Ok(_) => Err(de::Error::custom("dimension must be finite")),
                    }
                }
            }
        }

        deserializer.deserialize_any(DimensionVisitor)
    }
}

/// Style description that can be serialized from JS
/// Note: This is only used for JSON deserialization, not direct napi binding
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleDesc {
    // Display
    pub display: Option<String>,
    pub visibility: Option<String>,

    // Flexbox
    pub flex_direction: Option<String>,
    pub flex_wrap: Option<String>,
    pub flex_grow: Option<f64>,
    pub flex_shrink: Option<f64>,
    pub flex_basis: Option<f64>,
    pub align_items: Option<String>,
    pub align_self: Option<String>,
    pub align_content: Option<String>,
    pub justify_content: Option<String>,
    pub gap: Option<f64>,
    pub row_gap: Option<f64>,
    pub column_gap: Option<f64>,
    pub grid_template_columns: Option<f64>,
    pub grid_template_rows: Option<f64>,
    pub grid_column_min: Option<String>,
    pub grid_row_min: Option<String>,

    // Sizing - now supports both numbers and strings like "100%" or "auto"
    pub width: Option<DimensionValue>,
    pub height: Option<DimensionValue>,
    pub min_width: Option<DimensionValue>,
    pub min_height: Option<DimensionValue>,
    pub max_width: Option<DimensionValue>,
    pub max_height: Option<DimensionValue>,

    // Spacing (padding)
    pub padding: Option<f64>,
    pub padding_top: Option<f64>,
    pub padding_right: Option<f64>,
    pub padding_bottom: Option<f64>,
    pub padding_left: Option<f64>,

    // Spacing (margin)
    pub margin: Option<f64>,
    pub margin_top: Option<f64>,
    pub margin_right: Option<f64>,
    pub margin_bottom: Option<f64>,
    pub margin_left: Option<f64>,

    // Position
    pub position: Option<String>,
    pub top: Option<f64>,
    pub right: Option<f64>,
    pub bottom: Option<f64>,
    pub left: Option<f64>,

    // Background & Colors
    pub background: Option<String>,
    pub background_color: Option<String>,
    pub color: Option<String>,
    pub opacity: Option<f64>,

    // Border
    pub border_width: Option<f64>,
    pub border_top_width: Option<f64>,
    pub border_right_width: Option<f64>,
    pub border_bottom_width: Option<f64>,
    pub border_left_width: Option<f64>,
    pub border_color: Option<String>,
    pub border_radius: Option<f64>,
    pub border_top_left_radius: Option<f64>,
    pub border_top_right_radius: Option<f64>,
    pub border_bottom_left_radius: Option<f64>,
    pub border_bottom_right_radius: Option<f64>,
    pub box_shadow: Option<BoxShadowValue>,

    // Text
    pub font_size: Option<f64>,
    pub font_family: Option<String>,
    pub font_weight: Option<FontWeightValue>,
    pub text_align: Option<String>,
    pub line_height: Option<f64>,
    pub white_space: Option<String>,
    pub text_overflow: Option<String>,
    pub line_clamp: Option<f64>,

    // Overflow
    pub overflow: Option<String>,
    pub overflow_x: Option<String>,
    pub overflow_y: Option<String>,

    // Cursor
    pub cursor: Option<String>,
    /// `"auto"` blocks mouse hits behind this element. `"none"` never does.
    /// Unset: block when this element paints a fill or is absolutely positioned.
    pub pointer_events: Option<String>,

    // Text selection. "none" opts an element and its subtree out of the
    // selection registry, so buttons and toolbars never start a drag.
    // Inherited down the tree like the CSS property of the same name.
    pub user_select: Option<String>,
    /// Selection wash colour for this subtree. Defaults to the theme accent at
    /// 35% opacity, the same tone Comet uses.
    pub selection_color: Option<String>,

    // Pseudo-selector styles — applied by GPUI natively (no JS round-trip).
    // Uses Box to avoid infinite-size struct (StyleDesc contains StyleDesc).
    pub hover: Option<Box<StyleDesc>>,
    pub active: Option<Box<StyleDesc>>,

    /// Parsed paint values. Skipped by JSON so `getTreeJson` remains a faithful
    /// representation of the user's style object.
    #[serde(skip)]
    #[doc(hidden)]
    pub resolved: ResolvedStyle,
}

impl StyleDesc {
    pub(crate) fn resolve_cached_values(&mut self) {
        let background = self
            .background_color
            .as_deref()
            .or(self.background.as_deref())
            .and_then(crate::color::parse_color_rgba);
        self.resolved = ResolvedStyle {
            initialized: true,
            background,
            color: self
                .color
                .as_deref()
                .and_then(crate::color::parse_color_rgba),
            border_color: self
                .border_color
                .as_deref()
                .and_then(crate::color::parse_color_rgba),
            shadow_color: self
                .box_shadow
                .as_ref()
                .and_then(|shadow| crate::color::parse_color_rgba(&shadow.color)),
            selection_color: self
                .selection_color
                .as_deref()
                .and_then(crate::color::parse_color_rgba),
            cursor: self.cursor.as_deref().and_then(parse_cursor),
            display: parse_display(self.display.as_deref()),
            position: parse_position(self.position.as_deref()),
            overflow_x: parse_overflow(self.overflow_x.as_deref().or(self.overflow.as_deref())),
            overflow_y: parse_overflow(self.overflow_y.as_deref().or(self.overflow.as_deref())),
            user_select: parse_user_select(self.user_select.as_deref()),
            pointer_events: parse_pointer_events(self.pointer_events.as_deref()),
            flex_direction: parse_flex_direction(self.flex_direction.as_deref()),
            flex_wrap: parse_flex_wrap(self.flex_wrap.as_deref()),
            align_items: parse_align(self.align_items.as_deref()),
            align_content: parse_align(self.align_content.as_deref()),
            justify_content: parse_align(self.justify_content.as_deref()),
            align_self: parse_align(self.align_self.as_deref()),
            text_align: parse_text_align(self.text_align.as_deref()),
            white_space: parse_white_space(self.white_space.as_deref()),
            text_overflow: parse_text_overflow(self.text_overflow.as_deref()),
            font_weight: self.font_weight.as_ref().map(parse_font_weight),
            visible: match self.visibility.as_deref() {
                Some("hidden") => Some(false),
                Some("visible") => Some(true),
                _ => None,
            },
        };
        if let Some(hover) = &mut self.hover {
            hover.resolve_cached_values();
        }
        if let Some(active) = &mut self.active {
            active.resolve_cached_values();
        }
    }

    pub(crate) fn resolved_background(&self) -> Option<gpui::Rgba> {
        if self.resolved.initialized {
            self.resolved.background
        } else {
            self.background_color
                .as_deref()
                .or(self.background.as_deref())
                .and_then(crate::color::parse_color_rgba)
        }
    }

    pub(crate) fn resolved_color(&self) -> Option<gpui::Rgba> {
        if self.resolved.initialized {
            self.resolved.color
        } else {
            self.color
                .as_deref()
                .and_then(crate::color::parse_color_rgba)
        }
    }

    pub(crate) fn resolved_border_color(&self) -> Option<gpui::Rgba> {
        if self.resolved.initialized {
            self.resolved.border_color
        } else {
            self.border_color
                .as_deref()
                .and_then(crate::color::parse_color_rgba)
        }
    }

    pub(crate) fn resolved_shadow_color(&self) -> Option<gpui::Rgba> {
        if self.resolved.initialized {
            self.resolved.shadow_color
        } else {
            self.box_shadow
                .as_ref()
                .and_then(|shadow| crate::color::parse_color_rgba(&shadow.color))
        }
    }

    pub(crate) fn resolved_selection_color(&self) -> Option<gpui::Rgba> {
        if self.resolved.initialized {
            self.resolved.selection_color
        } else {
            self.selection_color
                .as_deref()
                .and_then(crate::color::parse_color_rgba)
        }
    }

    pub(crate) fn resolved_cursor(&self) -> Option<gpui::CursorStyle> {
        if self.resolved.initialized {
            self.resolved.cursor
        } else {
            self.cursor.as_deref().and_then(parse_cursor)
        }
    }

    pub(crate) fn resolved_display(&self) -> Option<DisplayValue> {
        if self.resolved.initialized {
            self.resolved.display
        } else {
            parse_display(self.display.as_deref())
        }
    }

    pub(crate) fn resolved_position(&self) -> Option<PositionValue> {
        if self.resolved.initialized {
            self.resolved.position
        } else {
            parse_position(self.position.as_deref())
        }
    }

    pub(crate) fn resolved_overflow(&self) -> (Option<OverflowValue>, Option<OverflowValue>) {
        if self.resolved.initialized {
            (self.resolved.overflow_x, self.resolved.overflow_y)
        } else {
            (
                parse_overflow(self.overflow_x.as_deref().or(self.overflow.as_deref())),
                parse_overflow(self.overflow_y.as_deref().or(self.overflow.as_deref())),
            )
        }
    }

    pub(crate) fn resolved_user_select(&self) -> Option<SelectValue> {
        if self.resolved.initialized {
            self.resolved.user_select
        } else {
            parse_user_select(self.user_select.as_deref())
        }
    }

    pub(crate) fn resolved_pointer_events(&self) -> Option<PointerEventsValue> {
        if self.resolved.initialized {
            self.resolved.pointer_events
        } else {
            parse_pointer_events(self.pointer_events.as_deref())
        }
    }

    pub(crate) fn resolved_flex_direction(&self) -> Option<FlexDirectionValue> {
        if self.resolved.initialized {
            self.resolved.flex_direction
        } else {
            parse_flex_direction(self.flex_direction.as_deref())
        }
    }

    pub(crate) fn resolved_flex_wrap(&self) -> Option<FlexWrapValue> {
        if self.resolved.initialized {
            self.resolved.flex_wrap
        } else {
            parse_flex_wrap(self.flex_wrap.as_deref())
        }
    }

    pub(crate) fn resolved_align_items(&self) -> Option<AlignValue> {
        if self.resolved.initialized {
            self.resolved.align_items
        } else {
            parse_align(self.align_items.as_deref())
        }
    }

    pub(crate) fn resolved_align_content(&self) -> Option<AlignValue> {
        if self.resolved.initialized {
            self.resolved.align_content
        } else {
            parse_align(self.align_content.as_deref())
        }
    }

    pub(crate) fn resolved_justify_content(&self) -> Option<AlignValue> {
        if self.resolved.initialized {
            self.resolved.justify_content
        } else {
            parse_align(self.justify_content.as_deref())
        }
    }

    pub(crate) fn resolved_align_self(&self) -> Option<AlignValue> {
        if self.resolved.initialized {
            self.resolved.align_self
        } else {
            parse_align(self.align_self.as_deref())
        }
    }

    pub(crate) fn resolved_text_align(&self) -> Option<TextAlignValue> {
        if self.resolved.initialized {
            self.resolved.text_align
        } else {
            parse_text_align(self.text_align.as_deref())
        }
    }

    pub(crate) fn resolved_white_space(&self) -> Option<WhiteSpaceValue> {
        if self.resolved.initialized {
            self.resolved.white_space
        } else {
            parse_white_space(self.white_space.as_deref())
        }
    }

    pub(crate) fn resolved_text_overflow(&self) -> Option<TextOverflowValue> {
        if self.resolved.initialized {
            self.resolved.text_overflow
        } else {
            parse_text_overflow(self.text_overflow.as_deref())
        }
    }

    pub(crate) fn resolved_font_weight(&self) -> Option<gpui::FontWeight> {
        if self.resolved.initialized {
            self.resolved.font_weight
        } else {
            self.font_weight.as_ref().map(parse_font_weight)
        }
    }

    pub(crate) fn resolved_visibility(&self) -> Option<bool> {
        if self.resolved.initialized {
            self.resolved.visible
        } else {
            match self.visibility.as_deref() {
                Some("hidden") => Some(false),
                Some("visible") => Some(true),
                _ => None,
            }
        }
    }
}

fn parse_display(value: Option<&str>) -> Option<DisplayValue> {
    match value {
        Some("flex") => Some(DisplayValue::Flex),
        Some("grid") => Some(DisplayValue::Grid),
        Some("none") => Some(DisplayValue::None),
        _ => None,
    }
}

fn parse_position(value: Option<&str>) -> Option<PositionValue> {
    match value {
        Some("relative") => Some(PositionValue::Relative),
        Some("absolute" | "fixed") => Some(PositionValue::Absolute),
        _ => None,
    }
}

fn parse_overflow(value: Option<&str>) -> Option<OverflowValue> {
    match value {
        Some("hidden") => Some(OverflowValue::Hidden),
        Some("scroll") => Some(OverflowValue::Scroll),
        Some("visible") => Some(OverflowValue::Visible),
        _ => None,
    }
}

fn parse_user_select(value: Option<&str>) -> Option<SelectValue> {
    match value {
        Some("none") => Some(SelectValue::None),
        Some("text" | "auto") => Some(SelectValue::Text),
        _ => None,
    }
}

fn parse_pointer_events(value: Option<&str>) -> Option<PointerEventsValue> {
    match value {
        Some("none") => Some(PointerEventsValue::None),
        Some("auto") => Some(PointerEventsValue::Auto),
        _ => None,
    }
}

fn parse_flex_direction(value: Option<&str>) -> Option<FlexDirectionValue> {
    match value {
        Some("row") => Some(FlexDirectionValue::Row),
        Some("column") => Some(FlexDirectionValue::Column),
        _ => None,
    }
}

fn parse_flex_wrap(value: Option<&str>) -> Option<FlexWrapValue> {
    match value {
        Some("wrap") => Some(FlexWrapValue::Wrap),
        Some("wrap-reverse") => Some(FlexWrapValue::WrapReverse),
        Some("nowrap") => Some(FlexWrapValue::NoWrap),
        _ => None,
    }
}

fn parse_align(value: Option<&str>) -> Option<AlignValue> {
    match value {
        Some("center") => Some(AlignValue::Center),
        Some("start" | "flex-start") => Some(AlignValue::Start),
        Some("end" | "flex-end") => Some(AlignValue::End),
        Some("between" | "space-between") => Some(AlignValue::Between),
        Some("around" | "space-around") => Some(AlignValue::Around),
        Some("evenly" | "space-evenly") => Some(AlignValue::Evenly),
        Some("stretch") => Some(AlignValue::Stretch),
        Some("baseline") => Some(AlignValue::Baseline),
        Some("normal") => Some(AlignValue::Normal),
        _ => None,
    }
}

fn parse_text_align(value: Option<&str>) -> Option<TextAlignValue> {
    match value {
        Some("center") => Some(TextAlignValue::Center),
        Some("right" | "end") => Some(TextAlignValue::Right),
        Some("left" | "start") => Some(TextAlignValue::Left),
        _ => None,
    }
}

fn parse_white_space(value: Option<&str>) -> Option<WhiteSpaceValue> {
    match value {
        Some("nowrap") => Some(WhiteSpaceValue::NoWrap),
        Some("normal") => Some(WhiteSpaceValue::Normal),
        _ => None,
    }
}

fn parse_text_overflow(value: Option<&str>) -> Option<TextOverflowValue> {
    match value {
        Some("ellipsis") => Some(TextOverflowValue::Ellipsis),
        Some("ellipsis-start") => Some(TextOverflowValue::EllipsisStart),
        _ => None,
    }
}

pub use crate::color::{parse_color, parse_color_hex};

/// Whether this style should insert a mouse hitbox.
///
/// GPUI only hit-tests elements that own a hitbox. A painted overlay without
/// one stays visible while clicks fall through. CSS `pointer-events` maps
/// here: `none` never blocks, `auto` always does. Unset follows the painted
/// surface: a fill or an absolute/fixed box blocks.
///
/// In-flow fills use `BlockMouseExceptScroll` so a parent scroller still gets
/// the wheel. `occlude()` (`BlockMouse`) is only for overlays that steal it.
#[must_use]
pub fn should_occlude(style: &StyleDesc) -> bool {
    match style.resolved_pointer_events() {
        Some(PointerEventsValue::None) => return false,
        Some(PointerEventsValue::Auto) => return true,
        _ => {}
    }
    if style.resolved_position() == Some(PositionValue::Absolute) {
        return true;
    }
    let fill_declared = style.background_color.is_some() || style.background.is_some();
    if !fill_declared {
        return false;
    }
    match style.resolved_background() {
        Some(color) => color.a > 0.0,
        None => true,
    }
}

/// Map a CSS `cursor` keyword onto a GPUI cursor. Unknown keywords return
/// `None` so the property is ignored, like every other invalid style value.
///
/// `ResizeUpLeftDownRight` is the NorthWest/SouthEast cursor on every backend,
/// so it is `nwse-resize`. GPUI's doc comments and its browser backend named
/// the opposite CSS values until the pinned fork corrected them, so do not
/// "fix" this pair back by reading an older GPUI.
#[must_use]
pub fn parse_cursor(name: &str) -> Option<gpui::CursorStyle> {
    use gpui::CursorStyle;
    Some(match name {
        "default" | "auto" => CursorStyle::Arrow,
        "pointer" => CursorStyle::PointingHand,
        "text" => CursorStyle::IBeam,
        "vertical-text" => CursorStyle::IBeamCursorForVerticalLayout,
        "crosshair" => CursorStyle::Crosshair,
        "grab" => CursorStyle::OpenHand,
        "grabbing" | "move" | "all-scroll" => CursorStyle::ClosedHand,
        "col-resize" => CursorStyle::ResizeColumn,
        "row-resize" => CursorStyle::ResizeRow,
        "ew-resize" => CursorStyle::ResizeLeftRight,
        "ns-resize" => CursorStyle::ResizeUpDown,
        "nwse-resize" | "nw-resize" | "se-resize" => CursorStyle::ResizeUpLeftDownRight,
        "nesw-resize" | "ne-resize" | "sw-resize" => CursorStyle::ResizeUpRightDownLeft,
        "w-resize" => CursorStyle::ResizeLeft,
        "e-resize" => CursorStyle::ResizeRight,
        "n-resize" => CursorStyle::ResizeUp,
        "s-resize" => CursorStyle::ResizeDown,
        "not-allowed" | "no-drop" => CursorStyle::OperationNotAllowed,
        "alias" => CursorStyle::DragLink,
        "copy" => CursorStyle::DragCopy,
        "context-menu" => CursorStyle::ContextualMenu,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_fill(fill: &str) -> StyleDesc {
        StyleDesc {
            background_color: Some(fill.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn transparent_function_does_not_occlude() {
        assert!(!should_occlude(&with_fill("transparent")));
        assert!(!should_occlude(&with_fill("oklch(50% 0.2 30 / 0%)")));
    }

    #[test]
    fn invalid_fill_keeps_conservative_occlusion() {
        assert!(should_occlude(&with_fill("not-a-color")));
    }

    #[test]
    fn maps_the_timeline_cursors() {
        assert_eq!(
            parse_cursor("col-resize"),
            Some(gpui::CursorStyle::ResizeColumn)
        );
        assert_eq!(parse_cursor("grab"), Some(gpui::CursorStyle::OpenHand));
        assert_eq!(
            parse_cursor("grabbing"),
            Some(gpui::CursorStyle::ClosedHand)
        );
        assert_eq!(
            parse_cursor("pointer"),
            Some(gpui::CursorStyle::PointingHand)
        );
        assert_eq!(parse_cursor("default"), Some(gpui::CursorStyle::Arrow));
    }

    #[test]
    fn ignores_an_unknown_cursor() {
        assert_eq!(parse_cursor("zoom-in"), None);
        assert_eq!(parse_cursor("POINTER"), None);
    }

    #[test]
    fn dimensions_round_trip_without_losing_their_units() {
        for (json, expected) in [
            ("12.5", DimensionValue::Pixels(12.5)),
            (r#""50%""#, DimensionValue::Percentage(0.5)),
            (r#""auto""#, DimensionValue::Auto),
        ] {
            let parsed: DimensionValue = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
    }

    #[test]
    fn dimensions_reject_non_finite_strings() {
        for value in [r#""NaN""#, r#""inf""#, r#""NaN%""#] {
            assert!(serde_json::from_str::<DimensionValue>(value).is_err());
        }
    }
}
