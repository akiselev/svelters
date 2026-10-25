//! Small recording helpers the views share (filled and bordered boxes, shadows, lines), private until
//! lane WIDGETS-BASIC's `paint` helpers land.

// used by the view modules as they land
#![allow(dead_code)]
use presto_base::color::{Color, Fill};
use presto_base::display::{BorderStyle, DisplayList, Line, LineCap, Prim, Quad, Shadow};
use presto_base::geom::{Corners, Insets, Point, Rect, Vec2};

/// A filled rounded box.
pub(crate) fn fill(list: &mut DisplayList, rect: Rect, radius: f32, color: Color) {
    if color.a <= 0.0 || rect.is_empty() {
        return;
    }
    list.push(
        Prim::Quad(Quad {
            rect,
            radii: Corners::all(radius),
            fill: Fill::Solid(color),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: BorderStyle::Solid,
        }),
        rect,
    );
}

/// A filled rounded box with a uniform border.
pub(crate) fn boxed(list: &mut DisplayList, rect: Rect, radius: f32, color: Color, border: f32, border_color: Color) {
    if rect.is_empty() {
        return;
    }
    list.push(
        Prim::Quad(Quad {
            rect,
            radii: Corners::all(radius),
            fill: Fill::Solid(color),
            border_widths: Insets::all(border),
            border_color,
            border_style: BorderStyle::Solid,
        }),
        rect,
    );
}

/// A drop shadow under `rect`.
pub(crate) fn shadow(list: &mut DisplayList, rect: Rect, radius: f32, color: Color, blur: f32, offset: Vec2) {
    let ink = rect.translate(offset).inflate(blur * 2.0);
    list.push(
        Prim::Shadow(Shadow { rect, radii: Corners::all(radius), color, blur, spread: 0.0, offset, inset: false }),
        ink,
    );
}

/// A straight hairline.
pub(crate) fn line(list: &mut DisplayList, from: Point, to: Point, width: f32, color: Color) {
    let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(width);
    list.push(Prim::Line(Line { from, to, width, color, cap: LineCap::Butt, dash: None, wavy: false }), ink);
}
