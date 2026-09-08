//! CSS-style application for GPUI host elements.

#![allow(
    clippy::cast_possible_truncation,
    reason = "validated CSS f64 dimensions narrow to GPUI f32 geometry"
)]

use crate::style::{
    AlignValue, DisplayValue, FlexDirectionValue, FlexWrapValue, OverflowValue, PositionValue,
    StyleDesc, TextAlignValue, TextOverflowValue, WhiteSpaceValue,
};

pub(crate) fn apply_width<E: gpui::Styled>(el: E, dim: &crate::style::DimensionValue) -> E {
    match dim {
        crate::style::DimensionValue::Pixels(v) => el.w(gpui::px(*v as f32)),
        crate::style::DimensionValue::Percentage(v) if *v >= 0.999 => el.w_full(),
        crate::style::DimensionValue::Percentage(v) => el.w(gpui::relative(*v as f32)),
        crate::style::DimensionValue::Auto => el,
    }
}

pub(crate) fn apply_height<E: gpui::Styled>(el: E, dim: &crate::style::DimensionValue) -> E {
    match dim {
        crate::style::DimensionValue::Pixels(v) => el.h(gpui::px(*v as f32)),
        crate::style::DimensionValue::Percentage(v) if *v >= 0.999 => el.h_full(),
        crate::style::DimensionValue::Percentage(v) => el.h(gpui::relative(*v as f32)),
        crate::style::DimensionValue::Auto => el,
    }
}

/// Apply base styles and the stateful hover/active refinements.
///
/// GPUI stores those refinements behind the element identity, so callers must
/// assign a stable `.id(..)` before using this helper.
pub(crate) fn apply_interactive_styles<E>(mut el: E, style: &StyleDesc) -> E
where
    E: gpui::Styled + gpui::StatefulInteractiveElement,
{
    el = apply_styles(el, style);
    if let Some(hover_style) = style.hover.as_deref() {
        el = el.hover(|refinement| apply_styles(refinement, hover_style));
    }
    if let Some(active_style) = style.active.as_deref() {
        el = el.active(|refinement| apply_styles(refinement, active_style));
    }
    el
}

#[allow(
    clippy::too_many_lines,
    reason = "CSS-to-GPUI style application is one exhaustive property table"
)]
pub(crate) fn apply_styles<E: gpui::Styled>(mut el: E, style: &StyleDesc) -> E {
    match style.resolved_display() {
        Some(DisplayValue::Flex) => el = el.flex(),
        Some(DisplayValue::Grid) => el = el.grid(),
        _ => {}
    }
    if let Some(cols) = style.grid_template_columns {
        let count = cols.round().clamp(1.0, 64.0) as u16;
        el = match style.grid_column_min.as_deref() {
            Some("min-content") => el.grid_cols_min_content(count),
            Some("max-content") => el.grid_cols_max_content(count),
            _ => el.grid_cols(count),
        };
    }
    if let Some(rows) = style.grid_template_rows {
        let count = rows.round().clamp(1.0, 64.0) as u16;
        el = match style.grid_row_min.as_deref() {
            Some("min-content") => el.grid_rows_min_content(count),
            Some("max-content") => el.grid_rows_max_content(count),
            _ => el.grid_rows(count),
        };
    }
    match style.resolved_flex_direction() {
        Some(FlexDirectionValue::Column) => el = el.flex_col(),
        Some(FlexDirectionValue::Row) => el = el.flex_row(),
        None => {}
    }
    match style.resolved_flex_wrap() {
        Some(FlexWrapValue::Wrap) => el = el.flex_wrap(),
        Some(FlexWrapValue::WrapReverse) => el = el.flex_wrap_reverse(),
        Some(FlexWrapValue::NoWrap) => el = el.flex_nowrap(),
        _ => {}
    }
    if let Some(grow) = style.flex_grow {
        el.style().flex_grow = Some(grow as f32);
    }
    if let Some(shrink) = style.flex_shrink {
        el.style().flex_shrink = Some(shrink as f32);
    }
    if let Some(basis) = style.flex_basis {
        el = el.flex_basis(gpui::px(basis as f32));
    }
    match style.resolved_align_items() {
        Some(AlignValue::Center) => el = el.items_center(),
        Some(AlignValue::Start) => el = el.items_start(),
        Some(AlignValue::End) => el = el.items_end(),
        _ => {}
    }
    match style.resolved_align_content() {
        Some(AlignValue::Center) => el = el.content_center(),
        Some(AlignValue::Start) => el = el.content_start(),
        Some(AlignValue::End) => el = el.content_end(),
        Some(AlignValue::Between) => el = el.content_between(),
        Some(AlignValue::Around) => el = el.content_around(),
        Some(AlignValue::Evenly) => el = el.content_evenly(),
        Some(AlignValue::Stretch) => el = el.content_stretch(),
        Some(AlignValue::Normal) => el = el.content_normal(),
        _ => {}
    }
    match style.resolved_justify_content() {
        Some(AlignValue::Center) => el = el.justify_center(),
        Some(AlignValue::Start) => el = el.justify_start(),
        Some(AlignValue::End) => el = el.justify_end(),
        Some(AlignValue::Between) => el = el.justify_between(),
        Some(AlignValue::Around) => el = el.justify_around(),
        _ => {}
    }
    match style.resolved_align_self() {
        Some(AlignValue::Center) => {
            el.style().align_self = Some(gpui::AlignItems::Center);
        }
        Some(AlignValue::Start) => {
            el.style().align_self = Some(gpui::AlignItems::FlexStart);
        }
        Some(AlignValue::End) => {
            el.style().align_self = Some(gpui::AlignItems::FlexEnd);
        }
        Some(AlignValue::Stretch) => {
            el.style().align_self = Some(gpui::AlignItems::Stretch);
        }
        Some(AlignValue::Baseline) => {
            el.style().align_self = Some(gpui::AlignItems::Baseline);
        }
        _ => {}
    }
    if let Some(gap) = style.gap {
        el = el.gap(gpui::px(gap as f32));
    }
    // Per-axis gaps were in the style type and implemented nowhere. They come
    // after `gap` so the axis value wins, matching CSS shorthand order.
    if let Some(gap) = style.row_gap {
        el = el.gap_y(gpui::px(gap as f32));
    }
    if let Some(gap) = style.column_gap {
        el = el.gap_x(gpui::px(gap as f32));
    }
    if let Some(ref w) = style.width {
        el = apply_width(el, w);
    }
    if let Some(ref h) = style.height {
        el = apply_height(el, h);
    }
    if let Some(ref min_w) = style.min_width {
        match min_w {
            crate::style::DimensionValue::Pixels(v) => el = el.min_w(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.min_w(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(ref min_h) = style.min_height {
        match min_h {
            crate::style::DimensionValue::Pixels(v) => el = el.min_h(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.min_h(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(ref max_w) = style.max_width {
        match max_w {
            crate::style::DimensionValue::Pixels(v) => el = el.max_w(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.max_w(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(ref max_h) = style.max_height {
        match max_h {
            crate::style::DimensionValue::Pixels(v) => el = el.max_h(gpui::px(*v as f32)),
            crate::style::DimensionValue::Percentage(v) => el = el.max_h(gpui::relative(*v as f32)),
            crate::style::DimensionValue::Auto => {}
        }
    }
    if let Some(p) = style.padding {
        el = el.p(gpui::px(p as f32));
    }
    if let Some(pt) = style.padding_top {
        el = el.pt(gpui::px(pt as f32));
    }
    if let Some(pr) = style.padding_right {
        el = el.pr(gpui::px(pr as f32));
    }
    if let Some(pb) = style.padding_bottom {
        el = el.pb(gpui::px(pb as f32));
    }
    if let Some(pl) = style.padding_left {
        el = el.pl(gpui::px(pl as f32));
    }
    if let Some(m) = style.margin {
        el = el.m(gpui::px(m as f32));
    }
    if let Some(mt) = style.margin_top {
        el = el.mt(gpui::px(mt as f32));
    }
    if let Some(mr) = style.margin_right {
        el = el.mr(gpui::px(mr as f32));
    }
    if let Some(mb) = style.margin_bottom {
        el = el.mb(gpui::px(mb as f32));
    }
    if let Some(ml) = style.margin_left {
        el = el.ml(gpui::px(ml as f32));
    }
    // Taffy has no viewport-fixed position, and GPUI has no scrolling document,
    // so "fixed" lays out exactly like "absolute". `should_occlude` already
    // treated the two the same; without this arm a "fixed" box stayed in flow.
    match style.resolved_position() {
        Some(PositionValue::Absolute) => el = el.absolute(),
        Some(PositionValue::Relative) => el = el.relative(),
        _ => {}
    }
    if let Some(top) = style.top {
        el = el.top(gpui::px(top as f32));
    }
    if let Some(right) = style.right {
        el = el.right(gpui::px(right as f32));
    }
    if let Some(bottom) = style.bottom {
        el = el.bottom(gpui::px(bottom as f32));
    }
    if let Some(left) = style.left {
        el = el.left(gpui::px(left as f32));
    }
    if let Some(color) = style.resolved_background() {
        el = el.bg(color);
    }
    if let Some(color) = style.resolved_color() {
        el = el.text_color(color);
    }
    if let Some(size) = style.font_size {
        el = el.text_size(gpui::px(size as f32));
    }
    if let Some(ref family) = style.font_family {
        el = el.font_family(family.clone());
    }
    if let Some(weight) = style.resolved_font_weight() {
        el = el.font_weight(weight);
    }
    if style.resolved_visibility() == Some(false) {
        el = el.invisible();
    }
    // `textAlign` was in the style type but implemented nowhere.
    match style.resolved_text_align() {
        Some(TextAlignValue::Center) => el = el.text_center(),
        Some(TextAlignValue::Right) => el = el.text_right(),
        Some(TextAlignValue::Left) => el = el.text_left(),
        _ => {}
    }
    match style.resolved_white_space() {
        Some(WhiteSpaceValue::NoWrap) => el = el.whitespace_nowrap(),
        Some(WhiteSpaceValue::Normal) => el = el.whitespace_normal(),
        _ => {}
    }
    match style.resolved_text_overflow() {
        Some(TextOverflowValue::Ellipsis) => el = el.text_ellipsis(),
        Some(TextOverflowValue::EllipsisStart) => el = el.text_ellipsis_start(),
        _ => {}
    }
    if let Some(clamp) = style.line_clamp
        && clamp >= 1.0
    {
        el = el.line_clamp(clamp as usize);
    }
    // `line_height` was accepted by the style type but never applied, so
    // multi-line text always used gpui's default leading.
    match style.text_decoration.as_deref() {
        Some("underline") => el = el.underline(),
        Some("line-through") => el = el.line_through(),
        Some("none") => el = el.text_decoration_none(),
        _ => {}
    }
    if let Some(line_height) = style.line_height
        && line_height > 0.0
    {
        el = el.line_height(gpui::px(line_height as f32));
    }
    if let Some(radius) = style.border_radius {
        el = el.rounded(gpui::px(radius as f32));
    }
    // Apply corner longhands after the shorthand so the explicit corner wins.
    if let Some(radius) = style.border_top_left_radius {
        el = el.rounded_tl(gpui::px(radius as f32));
    }
    if let Some(radius) = style.border_top_right_radius {
        el = el.rounded_tr(gpui::px(radius as f32));
    }
    if let Some(radius) = style.border_bottom_left_radius {
        el = el.rounded_bl(gpui::px(radius as f32));
    }
    if let Some(radius) = style.border_bottom_right_radius {
        el = el.rounded_br(gpui::px(radius as f32));
    }
    // `borderWidth: 0` must clear a border, not be ignored: an element that
    // draws its own border needs a way for the caller to remove it.
    if let Some(width) = style.border_width {
        el = el.border(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_top_width {
        el = el.border_t(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_right_width {
        el = el.border_r(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_bottom_width {
        el = el.border_b(gpui::px(width.max(0.0) as f32));
    }
    if let Some(width) = style.border_left_width {
        el = el.border_l(gpui::px(width.max(0.0) as f32));
    }
    if let Some(color) = style.resolved_border_color() {
        el = el.border_color(color);
    }
    if let Some(ref shadow) = style.box_shadow
        && let Some(color) = style.resolved_shadow_color()
    {
        let shadow = gpui::BoxShadow::new(
            gpui::px(shadow.offset_x as f32),
            gpui::px(shadow.offset_y as f32),
            color.into(),
        )
        .blur_radius(gpui::px(shadow.blur_radius.max(0.0) as f32))
        .spread_radius(gpui::px(shadow.spread_radius as f32));
        el = el.shadow(vec![shadow]);
    }
    if let Some(opacity) = style.opacity {
        el = el.opacity(opacity as f32);
    }
    if let Some(cursor) = style.resolved_cursor() {
        el = el.cursor(cursor);
    }
    // Overflow: hidden is on the Styled trait, so we handle it here.
    // overflow: "scroll" requires StatefulInteractiveElement — handled in build_div().
    // CSS precedence: axis-specific (overflowX/Y) overrides the shorthand (overflow).
    {
        let (resolved_x, resolved_y) = style.resolved_overflow();
        // Only apply hidden here — scroll is handled in build_div.
        if resolved_x == Some(OverflowValue::Hidden) && resolved_y == Some(OverflowValue::Hidden) {
            el = el.overflow_hidden();
        } else if resolved_x == Some(OverflowValue::Hidden) {
            el = el.overflow_x_hidden();
        } else if resolved_y == Some(OverflowValue::Hidden) {
            el = el.overflow_y_hidden();
        }
    }

    el
}
