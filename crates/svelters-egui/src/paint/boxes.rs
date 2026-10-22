//! The box primitive: outer shadows, background, inset shadows, border; and the outline.
//!
//! A uniform border is one `RectShape` (`StrokeKind::Inside`, what VIS1 measures). OW-CSS adds
//! per-corner radii, per-side borders (one quad per side; square joins) and gradient backgrounds
//! (`linear-gradient`/`radial-gradient`, WIDGET-PARITY P4) as a vertex-coloured mesh over the
//! background colour.

use super::{PaintCx, StyleView, TraceKind};
use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, vec2};

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
    let padding = Rect::from_min_max(border_box.min + vec2(bl, bt), border_box.max - vec2(br, bb));
    let content = Rect::from_min_max(padding.min + vec2(pl, pt), padding.max - vec2(pr, pb));
    BoxGeom { border: border_box, padding, content: if content.is_negative() { Rect::from_center_size(content.center(), egui::Vec2::ZERO) } else { content } }
}

/// Padding plus border around the content: what `measure` adds to the content size (width,
/// height). No egui `Spacing` involved.
pub fn box_extra(s: &StyleView) -> egui::Vec2 {
    let [bl, br, bt, bb] = s.sides("border-width");
    let [pl, pr, pt, pb] = s.sides("padding");
    vec2(bl + br + pl + pr, bt + bb + pt + pb)
}

/// Paints the box of `border_box` under `s`: outer shadows, background, inset shadows, border.
/// Returns its geometry for the content.
pub fn paint_box(cx: &mut PaintCx, s: &StyleView, border_box: Rect) -> BoxGeom {
    paint_box_layers(cx, s, border_box, true)
}

/// OW2: [`paint_box`] without its inset shadows (`inset: false`), for a host that draws content
/// over its background and its inset shadows over the content ([`paint_inset_shadows`]): a scroll
/// area's R15 edge shadows lie over the scrolled children.
pub fn paint_box_layers(cx: &mut PaintCx, s: &StyleView, border_box: Rect, inset: bool) -> BoxGeom {
    let g = box_geom(s, border_box);
    let r = cx.snap_rect(border_box);
    let radius = s.px("border-radius").unwrap_or(0.0).max(0.0);
    let shadows = s.shadows();
    let shadow_color = |sh: &svelters_style::Shadow| match sh.color {
        Some(k) => {
            let [r, g, b, a] = k.rgba8();
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        None => s.color("color").unwrap_or(egui::Color32::BLACK),
    };
    if let Some(list) = shadows.as_deref() {
        for sh in crate::style::shadow::shapes(list, r, radius, false, &shadow_color) {
            cx.painter.add(sh);
        }
    }
    let fill = s.color("background-color").unwrap_or(egui::Color32::TRANSPARENT);
    let cr = corner_radius(s.radii());
    let sides = s.border_sides().map(|(w, c)| (cx.snap_width(w), c));
    let uniform = sides.iter().all(|x| *x == sides[0]);
    let (bw, bc) = if uniform { sides[0] } else { (0.0, Color32::TRANSPARENT) };
    let stroke = if bw > 0.0 && bc.a() > 0 { Stroke::new(bw, bc) } else { Stroke::NONE };
    // WIDGET-PARITY Q1: `background-image: paint(name)` between the inset shadows and the border
    let painter = super::painters::selected(s, "background-image");
    let gradient = s.engine().and_then(|e| e.gradient().cloned());
    // the border goes on top of what paints over the background
    let border_last = painter.is_some() || gradient.is_some();
    let first = if border_last { Stroke::NONE } else { stroke };
    if fill.a() > 0 || first.width > 0.0 {
        cx.painter.add(egui::epaint::RectShape::new(r, cr, fill, first, egui::StrokeKind::Inside));
    }
    // P4: the gradient over the background colour
    if let Some(g) = &gradient {
        cx.painter.add(gradient_mesh(g, r, cr));
    }
    if gradient.is_some() && painter.is_none() && stroke.width > 0.0 {
        cx.painter.add(egui::epaint::RectShape::new(r, cr, Color32::TRANSPARENT, stroke, egui::StrokeKind::Inside));
    }
    cx.record(TraceKind::Box, r, fill, stroke, 0.0);
    if inset {
        paint_inset_shadows(cx, s, border_box);
    }
    if let Some((_, p)) = painter {
        let clipped = cx.painter.with_clip_rect(cx.painter.clip_rect().intersect(r));
        let mut pc = PaintCx { painter: &clipped, ppp: cx.ppp, trace: cx.trace.as_deref_mut(), target: cx.target, sink: cx.sink.clone(), tag: cx.tag, args: cx.args };
        pc.run_painter(&*p, s, g.padding);
        if stroke.width > 0.0 {
            cx.painter.add(egui::epaint::RectShape::new(r, cr, egui::Color32::TRANSPARENT, stroke, egui::StrokeKind::Inside));
        }
    }
    if !uniform {
        paint_sides(cx, r, sides);
    }
    g
}

/// Radii in points as egui's per-corner radius.
fn corner_radius([tl, tr, br, bl]: [f32; 4]) -> CornerRadius {
    let b = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    CornerRadius { nw: b(tl), ne: b(tr), se: b(br), sw: b(bl) }
}

/// Non-uniform borders: one quad per side [left, right, top, bottom], mitred at the corners
/// (radii are not followed).
fn paint_sides(cx: &mut PaintCx, r: Rect, [(wl, cl), (wr, crr), (wt, ct), (wb, cb)]: [(f32, Color32); 4]) {
    let (a, b, c, d) = (r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom());
    let (ia, ib, ic, id) = (a + vec2(wl, wt), b + vec2(-wr, wt), c + vec2(-wr, -wb), d + vec2(wl, -wb));
    for (w, col, quad) in [(wt, ct, [a, b, ib, ia]), (wr, crr, [b, c, ic, ib]), (wb, cb, [c, d, id, ic]), (wl, cl, [d, a, ia, id])] {
        if w > 0.0 && col.a() > 0 {
            cx.painter.add(egui::Shape::convex_polygon(quad.to_vec(), col, Stroke::NONE));
            cx.record(TraceKind::Box, Rect::from_points(&quad), Color32::TRANSPARENT, Stroke::new(w, col), 0.0);
        }
    }
}

/// A gradient over the rounded rect `r` as a vertex-coloured mesh: the outline (edges split every
/// few points) fanned from the centre in rings, each vertex coloured from the gradient. Exact for
/// two-stop linear gradients; finer stops and radial gradients are sampled at the vertices.
fn gradient_mesh(g: &svelters_style::Gradient, r: Rect, cr: CornerRadius) -> egui::Mesh {
    const RINGS: usize = 8;
    const STEP: f32 = 6.0;
    let mut outline = Vec::new();
    egui::epaint::tessellator::path::rounded_rectangle(&mut outline, egui::epaint::RoundedRect::new(r, cr));
    let mut path: Vec<Pos2> = Vec::with_capacity(outline.len() * 2);
    for (k, &p) in outline.iter().enumerate() {
        let q = outline[(k + 1) % outline.len()];
        let n = ((p.distance(q) / STEP).ceil() as usize).clamp(1, 64);
        path.extend((0..n).map(|j| p.lerp(q, j as f32 / n as f32)));
    }
    let size = [r.width(), r.height()];
    let col = |p: Pos2| {
        let [cr, cg, cb, ca] = g.color_at(g.t_at(size, [p.x - r.min.x, p.y - r.min.y])).rgba8();
        Color32::from_rgba_unmultiplied(cr, cg, cb, ca)
    };
    let mut m = egui::Mesh::default();
    let c = r.center();
    m.colored_vertex(c, col(c));
    for &p in &path {
        for j in 1..=RINGS {
            let q = c + (p - c) * (j as f32 / RINGS as f32);
            m.colored_vertex(q, col(q));
        }
    }
    let n = path.len();
    let v = |k: usize, j: usize| (1 + (k % n) * RINGS + (j - 1)) as u32;
    for k in 0..n {
        m.add_triangle(0, v(k, 1), v(k + 1, 1));
        for j in 1..RINGS {
            m.add_triangle(v(k, j), v(k, j + 1), v(k + 1, j + 1));
            m.add_triangle(v(k, j), v(k + 1, j + 1), v(k + 1, j));
        }
    }
    m
}

/// OW2: the inset `box-shadow`s of `border_box` alone, clipped to it; true if there were any.
pub fn paint_inset_shadows(cx: &mut PaintCx, s: &StyleView, border_box: Rect) -> bool {
    let Some(shadows) = s.shadows() else { return false };
    let list: &[svelters_style::Shadow] = &shadows;
    let r = cx.snap_rect(border_box);
    let radius = s.px("border-radius").unwrap_or(0.0).max(0.0);
    let shadow_color = |sh: &svelters_style::Shadow| match sh.color {
        Some(k) => {
            let [r, g, b, a] = k.rgba8();
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        None => s.color("color").unwrap_or(egui::Color32::BLACK),
    };
    let inset = crate::style::shadow::shapes(list, r, radius, true, &shadow_color);
    if inset.is_empty() {
        return false;
    }
    let p = cx.painter.with_clip_rect(cx.painter.clip_rect().intersect(r));
    for sh in inset {
        p.add(sh);
    }
    true
}

/// Paints the outline of `border_box` under `s` (`outline-width`, `outline-color`,
/// `outline-style`, `outline-offset`): outside the border box by the offset, never affecting layout
/// or hit-testing. The focus ring is a `:focus-visible` outline (DESIGN R8).
pub fn paint_outline(cx: &mut PaintCx, s: &StyleView, border_box: Rect) {
    if matches!(s.keyword("outline-style"), None | Some("none" | "hidden")) {
        return;
    }
    let w = cx.snap_width(s.px("outline-width").unwrap_or(0.0));
    let Some(col) = s.color("outline-color") else { return };
    if w <= 0.0 || col.a() == 0 {
        return;
    }
    let off = s.px("outline-offset").unwrap_or(0.0);
    let r = cx.snap_rect(border_box.expand(off));
    let radius = (s.px("border-radius").unwrap_or(0.0) + off.max(0.0)).max(0.0);
    let stroke = Stroke::new(w, col);
    cx.painter.add(egui::epaint::RectShape::new(r, radius, egui::Color32::TRANSPARENT, stroke, egui::StrokeKind::Outside));
    cx.record(TraceKind::Outline, r, egui::Color32::TRANSPARENT, stroke, 0.0);
}
