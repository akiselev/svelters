//! `box-shadow` on egui (CS5g, plans/milestones/CS5g-egui-style-deferrals.md): the engine's typed
//! list ([`svelters_style::Shadow`]) painted with epaint's blurred `RectShape`, as egui's own
//! `Shadow::as_shape` does, but with `f32` geometry (egui's `Shadow` is `i8`/`u8`).
//!
//! - **Outer shadows** paint below the box's background, the first shadow of the list on top. The
//!   shadow is the box moved by (x, y), grown by the spread (its radius too), and blurred: CSS's
//!   blur radius `b` is a penumbra reaching `b` to each side of the edge, epaint's `blur_width` is
//!   the whole penumbra, so `blur_width = 2b`. Unlike CSS, the part under the box is not cut out:
//!   it shows only through a translucent background (egui's `Frame` shadows do the same).
//! - **Inset shadows** paint above the background, clipped to the box: four blurred bands around
//!   the "hole" (the box moved by (x, y) and shrunk by the spread). The hole's corners are square
//!   (the clip is a rect), an approximation of CSS's rounded inset corners.
//! - **Bounds.** [`paint_bounds`] is the node's rect grown by its outer shadows; the walk culls by
//!   it, so a shadow still paints when only it is on screen.
//! - `currentcolor` with no `color` anywhere is the text colour of egui's visuals.
//!
//! Writer lane: STYLE.

use crate::scene::Scene;
use svelters_style::{ComputedStyle, ComputedValue, Shadow};

/// The node's `box-shadow` list (empty or `None`: no shadows).
pub(crate) fn shadows(c: &ComputedStyle) -> Option<&[Shadow]> {
    match c.get("box-shadow") {
        Some(ComputedValue::Shadows(l)) if !l.is_empty() => Some(l),
        _ => None,
    }
}

/// How far the outer shadows reach past the box on each side (left, right, top, bottom; ≥ 0).
pub(crate) fn outer_margin(list: &[Shadow]) -> egui::epaint::MarginF32 {
    let mut m = [0.0f32; 4];
    for s in list.iter().filter(|s| !s.inset) {
        let grow = s.spread + s.blur;
        m[0] = m[0].max(grow - s.x);
        m[1] = m[1].max(grow + s.x);
        m[2] = m[2].max(grow - s.y);
        m[3] = m[3].max(grow + s.y);
    }
    egui::epaint::MarginF32 { left: m[0], right: m[1], top: m[2], bottom: m[3] }
}

/// The rect node `i` paints in: `r` grown by its outer shadows (the walk's culling rect).
pub(crate) fn paint_bounds(s: &Scene, i: usize, r: egui::Rect) -> egui::Rect {
    if !s.style.is_active() {
        return r;
    }
    let Some(list) = s.slots.id_at(i).and_then(|id| s.style.computed(id)).and_then(|c| shadows(c)) else { return r };
    let m = outer_margin(list);
    egui::Rect::from_min_max(r.min - egui::vec2(m.left, m.top), r.max + egui::vec2(m.right, m.bottom))
}

fn color32(s: &Shadow, p: &egui::Painter) -> egui::Color32 {
    match s.color {
        Some(k) => {
            let [r, g, b, a] = k.rgba8();
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        None => p.ctx().global_style().visuals.text_color(),
    }
}

/// The shapes of `list`'s outer (`inset == false`) or inset shadows for a box `r` with corner
/// `radius`, in paint order. Inset shapes are meant to be clipped to `r`.
pub(crate) fn shapes(list: &[Shadow], r: egui::Rect, radius: f32, inset: bool, color: &dyn Fn(&Shadow) -> egui::Color32) -> Vec<egui::epaint::RectShape> {
    let mut out = Vec::new();
    // the first shadow is on top: paint the list backwards
    for s in list.iter().rev().filter(|s| s.inset == inset) {
        let col = color(s);
        if col.a() == 0 {
            continue;
        }
        let blur = 2.0 * s.blur.max(0.0);
        let moved = r.translate(egui::vec2(s.x, s.y));
        if !inset {
            let rect = moved.expand(s.spread);
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                continue;
            }
            let cr = (radius + s.spread).max(0.0);
            out.push(egui::epaint::RectShape::filled(rect, cr, col).with_blur_width(blur));
            continue;
        }
        let hole = moved.shrink(s.spread);
        if hole.width() <= 0.0 || hole.height() <= 0.0 {
            out.push(egui::epaint::RectShape::filled(r, radius, col));
            continue;
        }
        // four bands around the hole, reaching well past `r` (the clip cuts them)
        let big = r.width().max(r.height()) + s.x.abs() + s.y.abs() + s.spread.abs() + blur + 1.0;
        let b = s.blur.max(0.0);
        let band = |x0: f32, y0: f32, x1: f32, y1: f32| egui::epaint::RectShape::filled(egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)), 0.0, col).with_blur_width(blur);
        out.push(band(hole.left() - big, hole.top() - big, hole.right() + big, hole.top()));
        out.push(band(hole.left() - big, hole.bottom(), hole.right() + big, hole.bottom() + big));
        out.push(band(hole.left() - big, hole.top() - b, hole.left(), hole.bottom() + b));
        out.push(band(hole.right(), hole.top() - b, hole.right() + big, hole.bottom() + b));
    }
    out
}

/// Paints `c`'s outer or inset shadows for a box `r` with corner `radius` into `p`.
pub(crate) fn paint(c: &ComputedStyle, r: egui::Rect, radius: f32, inset: bool, p: &egui::Painter) {
    let Some(list) = shadows(c) else { return };
    let shapes = shapes(list, r, radius, inset, &|s| color32(s, p));
    if shapes.is_empty() {
        return;
    }
    let p = if inset { p.with_clip_rect(p.clip_rect().intersect(r)) } else { p.clone() };
    for s in shapes {
        p.add(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use svelters_style::Color;

    fn sh(x: f32, y: f32, blur: f32, spread: f32, inset: bool) -> Shadow {
        Shadow { x, y, blur, spread, color: Some(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), inset }
    }

    #[test]
    fn outer_shapes_and_margin() {
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 40.0));
        let list = [sh(2.0, 4.0, 6.0, 1.0, false), sh(0.0, 0.0, 0.0, 3.0, false), sh(1.0, 1.0, 1.0, 1.0, true)];
        let black = |_: &Shadow| egui::Color32::BLACK;
        let out = shapes(&list, r, 4.0, false, &black);
        assert_eq!(out.len(), 2, "the inset one is not an outer shape");
        // painted backwards: the first shadow last (on top)
        assert_eq!(out[0].rect, r.expand(3.0));
        assert_eq!(out[0].blur_width, 0.0);
        assert_eq!(out[1].rect, r.translate(egui::vec2(2.0, 4.0)).expand(1.0));
        assert_eq!(out[1].blur_width, 12.0, "CSS blur radius 6 = a 12 point penumbra");
        assert_eq!(out[1].corner_radius, egui::CornerRadius::same(5));
        let m = outer_margin(&list);
        assert_eq!((m.left, m.right, m.top, m.bottom), (5.0, 9.0, 3.0, 11.0));
        // a spread shrinking the shadow away paints nothing
        assert!(shapes(&[sh(0.0, 0.0, 0.0, -60.0, false)], r, 0.0, false, &black).is_empty());
        // transparent shadows are skipped
        assert!(shapes(&list, r, 0.0, false, &|_| egui::Color32::TRANSPARENT).is_empty());
    }

    #[test]
    fn inset_bands_frame_the_hole() {
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        let out = shapes(&[sh(3.0, 0.0, 2.0, 5.0, true)], r, 0.0, true, &|_| egui::Color32::BLACK);
        assert_eq!(out.len(), 4);
        let hole = r.translate(egui::vec2(3.0, 0.0)).shrink(5.0);
        assert_eq!(out[0].rect.bottom(), hole.top());
        assert_eq!(out[1].rect.top(), hole.bottom());
        assert_eq!(out[2].rect.right(), hole.left());
        assert_eq!(out[3].rect.left(), hole.right());
        for s in &out {
            assert!(s.rect.contains_rect(s.rect.intersect(r)) && s.blur_width == 4.0);
            assert!(!s.rect.intersect(hole).is_positive(), "no band covers the hole");
        }
        // a hole shrunk away: the whole box is shadow
        let full = shapes(&[sh(0.0, 0.0, 0.0, 80.0, true)], r, 2.0, true, &|_| egui::Color32::BLACK);
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].rect, r);
    }
}
