//! The box primitive: outer shadows, background, inset shadows, border; and the outline.
//!
//! A border whose four sides agree is the quad's own border (the SDF shader draws it inside the
//! rounded rect, what VIS1 measures). Sides that differ in colour become one filled quad per side
//! (square joins; radii are not followed). A `linear-gradient` background is the quad's linear fill
//! over the background colour; `radial-gradient` has no primitive yet and paints the background
//! colour only.

use super::{PaintCx, StyleView, color_of};
use presto_base::color::{Color, Fill, GradientStop};
use presto_base::display::{BorderStyle, Prim, Quad, Shadow as ShadowPrim};
use presto_base::geom::{Corners, Insets, Point, Rect, Vec2};
use smallvec::SmallVec;

/// The three boxes of a node: border box (the layout rect), padding box (inside the border), and
/// content box (inside the padding).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxGeom {
    pub border: Rect,
    pub padding: Rect,
    pub content: Rect,
}

/// The boxes of `border_box` under `s` (no painting).
pub fn box_geom(s: &StyleView, border_box: Rect) -> BoxGeom {
    let [bl, br, bt, bb] = s.sides("border-width");
    let [pl, pr, pt, pb] = s.sides("padding");
    let padding = border_box.inset(Insets { top: bt, right: br, bottom: bb, left: bl });
    let content = padding.inset(Insets { top: pt, right: pr, bottom: pb, left: pl });
    let content = if content.width() < 0.0 || content.height() < 0.0 {
        let c = Point::new((content.min.x + content.max.x) / 2.0, (content.min.y + content.max.y) / 2.0);
        Rect { min: c, max: c }
    } else {
        content
    };
    BoxGeom { border: border_box, padding, content }
}

/// Padding plus border around the content: what `measure` adds to the content size (width, height).
pub fn box_extra(s: &StyleView) -> Vec2 {
    let [bl, br, bt, bb] = s.sides("border-width");
    let [pl, pr, pt, pb] = s.sides("padding");
    Vec2::new(bl + br + pl + pr, bt + bb + pt + pb)
}

/// Paints the box of `border_box` under `s`: outer shadows, background, inset shadows, border.
/// Returns its geometry for the content.
pub fn paint_box(cx: &mut PaintCx, s: &StyleView, border_box: Rect) -> BoxGeom {
    paint_box_layers(cx, s, border_box, true)
}

fn corners([tl, tr, br, bl]: [f32; 4]) -> Corners {
    Corners { tl, tr, br, bl }
}

/// The colour of a CSS shadow (`currentcolor` = the view's `color`).
fn shadow_color(s: &StyleView, sh: &svelters_style::Shadow) -> Color {
    match sh.color {
        Some(k) => color_of(k),
        None => s.color("color").unwrap_or(Color::BLACK),
    }
}

/// The shadows of `list` with `inset` as primitives, bottom first (CSS paints the first on top).
fn shadows(cx: &mut PaintCx, s: &StyleView, r: Rect, radii: Corners, inset: bool) {
    let Some(list) = s.shadows() else { return };
    for sh in list.iter().rev().filter(|sh| sh.inset == inset) {
        let color = shadow_color(s, sh);
        if color.a <= 0.0 {
            continue;
        }
        let p = ShadowPrim { rect: r, radii, color, blur: sh.blur.max(0.0), spread: sh.spread, offset: Vec2::new(sh.x, sh.y), inset };
        let ink = if inset { r } else { r.translate(Vec2::new(sh.x, sh.y)).inflate(sh.spread.max(0.0) + sh.blur.max(0.0)) };
        cx.push(Prim::Shadow(p), ink);
    }
}

/// [`paint_box`] without its inset shadows (`inset: false`), for a host that draws content over its
/// background and its inset shadows over the content ([`paint_inset_shadows`]): a scroll area's edge
/// shadows lie over the scrolled children.
pub fn paint_box_layers(cx: &mut PaintCx, s: &StyleView, border_box: Rect, inset: bool) -> BoxGeom {
    let g = box_geom(s, border_box);
    let r = cx.snap_rect(border_box);
    let radii = corners(s.radii());
    shadows(cx, s, r, radii, false);
    let bg = s.color("background-color").unwrap_or(Color::TRANSPARENT);
    let sides = s.border_sides().map(|(w, c)| (cx.snap_width(w), c));
    let [(wl, cl), (wr, cr), (wt, ct), (wb, cb)] = sides;
    let one_color = cl == cr && cl == ct && cl == cb;
    let border_style = match s.keyword("border-top-style").or(s.keyword("border-style")) {
        Some("dashed") => BorderStyle::Dashed,
        Some("dotted") => BorderStyle::Dotted,
        _ => BorderStyle::Solid,
    };
    let gradient = s.engine().and_then(|e| e.gradient().cloned());
    let mut fill = Fill::Solid(bg);
    if let Some(svelters_style::Gradient::Linear(lg)) = &gradient {
        if bg.a > 0.0 {
            // the gradient lies over the background colour: the colour first, then the gradient
            cx.push(Prim::Quad(Quad { rect: r, radii, fill: Fill::Solid(bg), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style }), r);
        }
        let stops: SmallVec<[GradientStop; 4]> = lg.stops.iter().map(|st| GradientStop { offset: st.position, color: color_of(st.color) }).collect();
        fill = Fill::Linear { angle_deg: lg.angle, stops };
    }
    let has_border = one_color && cl.a > 0.0 && (wl > 0.0 || wr > 0.0 || wt > 0.0 || wb > 0.0);
    let (widths, border_color) = if has_border { (Insets { top: wt, right: wr, bottom: wb, left: wl }, cl) } else { (Insets::ZERO, Color::TRANSPARENT) };
    let visible_fill = match &fill {
        Fill::Solid(c) => c.a > 0.0,
        Fill::Linear { .. } => true,
    };
    if visible_fill || has_border {
        cx.push(Prim::Quad(Quad { rect: r, radii, fill, border_widths: widths, border_color, border_style }), r);
    }
    if inset {
        paint_inset_shadows(cx, s, border_box);
    }
    if !one_color {
        paint_sides(cx, r, sides);
    }
    g
}

/// Borders whose sides differ in colour: one filled quad per side [left, right, top, bottom].
fn paint_sides(cx: &mut PaintCx, r: Rect, [(wl, cl), (wr, cr), (wt, ct), (wb, cb)]: [(f32, Color); 4]) {
    let side = |rect: Rect, c: Color| Quad { rect, radii: Corners::ZERO, fill: Fill::Solid(c), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid };
    let parts = [
        (wt, ct, Rect::new(r.min.x, r.min.y, r.max.x, r.min.y + wt)),
        (wb, cb, Rect::new(r.min.x, r.max.y - wb, r.max.x, r.max.y)),
        (wl, cl, Rect::new(r.min.x, r.min.y + wt, r.min.x + wl, r.max.y - wb)),
        (wr, cr, Rect::new(r.max.x - wr, r.min.y + wt, r.max.x, r.max.y - wb)),
    ];
    for (w, c, rect) in parts {
        if w > 0.0 && c.a > 0.0 && !rect.is_empty() {
            cx.push(Prim::Quad(side(rect, c)), rect);
        }
    }
}

/// The inset `box-shadow`s of `border_box` alone (the shader keeps them inside the rounded rect);
/// true if there were any.
pub fn paint_inset_shadows(cx: &mut PaintCx, s: &StyleView, border_box: Rect) -> bool {
    let before = cx.list.items.len();
    let r = cx.snap_rect(border_box);
    shadows(cx, s, r, corners(s.radii()), true);
    cx.list.items.len() > before
}

/// Paints the outline of `border_box` under `s` (`outline-width`, `outline-color`, `outline-style`,
/// `outline-offset`): outside the border box by the offset, never affecting layout or hit-testing.
/// The focus ring is a `:focus-visible` outline (DESIGN R8).
pub fn paint_outline(cx: &mut PaintCx, s: &StyleView, border_box: Rect) {
    let style = match s.keyword("outline-style") {
        None | Some("none" | "hidden") => return,
        Some("dashed") => BorderStyle::Dashed,
        Some("dotted") => BorderStyle::Dotted,
        Some(_) => BorderStyle::Solid,
    };
    let w = cx.snap_width(s.px("outline-width").unwrap_or(0.0));
    let Some(col) = s.color("outline-color") else { return };
    if w <= 0.0 || col.a <= 0.0 {
        return;
    }
    let off = s.px("outline-offset").unwrap_or(0.0);
    // the stroke lies outside the offset rect: a quad grown by the width, its border inside
    let r = cx.snap_rect(border_box.inflate(off + w));
    let base = (s.px("border-radius").unwrap_or(0.0) + off.max(0.0)).max(0.0);
    let radii = Corners::all(if base > 0.0 { base + w } else { 0.0 });
    cx.push(Prim::Quad(Quad { rect: r, radii, fill: Fill::Solid(Color::TRANSPARENT), border_widths: Insets::all(w), border_color: col, border_style: style }), r);
}
