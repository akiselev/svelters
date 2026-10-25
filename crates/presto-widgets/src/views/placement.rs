//! Where a floating surface (popup, popover, menu, submenu, select list, tooltip) goes next to its
//! anchor: a side and an alignment (`bottom-start`, `right-start`, …), a gap, then Floating UI's
//! `flip` (the other side when this one has no room and the other has more), `shift` (slid back
//! inside the bounds) and `size` (shrunk to the bounds, so a long menu scrolls instead of leaving the
//! screen). Ported from svelters-egui `interact/anchor.rs`; private to the views until lane
//! WIDGETS-BASIC's `interact::anchor` lands.

use presto_base::geom::{Point, Rect, Size};

/// The side of the anchor the surface goes on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
    pub fn vertical(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// Where along the anchor's edge the surface starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// A side and an alignment: `bottom-start`, `right-start`, `top`, `left-end`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    pub side: Side,
    pub align: Align,
}

impl Placement {
    pub const BOTTOM_START: Placement = Placement { side: Side::Bottom, align: Align::Start };
    pub const RIGHT_START: Placement = Placement { side: Side::Right, align: Align::Start };
    pub const TOP_CENTER: Placement = Placement { side: Side::Top, align: Align::Center };

    /// `top|bottom|left|right`, optionally `-start|-center|-end` (no suffix: start).
    pub fn parse(s: &str) -> Option<Placement> {
        let s = s.trim();
        let (side, align) = s.split_once('-').unwrap_or((s, "start"));
        let side = match side {
            "top" => Side::Top,
            "bottom" => Side::Bottom,
            "left" => Side::Left,
            "right" => Side::Right,
            _ => return None,
        };
        let align = match align {
            "start" => Align::Start,
            "center" => Align::Center,
            "end" => Align::End,
            _ => return None,
        };
        Some(Placement { side, align })
    }
}

/// What [`place`] is given. All rects in one space (window points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorParams {
    /// The anchor; a zero-size rect for a point (a context menu at the pointer).
    pub anchor: Rect,
    /// The surface's natural size.
    pub size: Size,
    /// Where it must stay (the window's content area or the monitor's work area).
    pub bounds: Rect,
    pub placement: Placement,
    /// The gap between the anchor and the surface (main axis).
    pub gap: f32,
    /// A cross-axis offset (a submenu moves up by its padding so its first row lines up).
    pub cross: f32,
    /// Try the opposite side when this one lacks room.
    pub flip: bool,
    /// Slide back inside `bounds`.
    pub shift: bool,
    /// Shrink to the room on the chosen side (main axis) and to the bounds (cross axis).
    pub fit: bool,
    /// At least as wide as the anchor (a select's list).
    pub match_anchor_width: bool,
}

impl Default for AnchorParams {
    fn default() -> Self {
        AnchorParams {
            anchor: Rect::ZERO,
            size: Size::ZERO,
            bounds: Rect::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY),
            placement: Placement::default(),
            gap: 0.0,
            cross: 0.0,
            flip: true,
            shift: true,
            fit: false,
            match_anchor_width: false,
        }
    }
}

/// Where the surface went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The side it ended on (after `flip`).
    pub side: Side,
    pub rect: Rect,
}

fn at(p: &AnchorParams, side: Side, size: Size) -> Point {
    let a = p.anchor;
    let main = match side {
        Side::Bottom => a.max.y + p.gap,
        Side::Top => a.min.y - p.gap - size.h,
        Side::Right => a.max.x + p.gap,
        Side::Left => a.min.x - p.gap - size.w,
    };
    let (lo, hi, len) = if side.vertical() { (a.min.x, a.max.x, size.w) } else { (a.min.y, a.max.y, size.h) };
    let cross = p.cross
        + match p.placement.align {
            Align::Start => lo,
            Align::Center => (lo + hi - len) / 2.0,
            Align::End => hi - len,
        };
    if side.vertical() { Point::new(cross, main) } else { Point::new(main, cross) }
}

/// The room on `side` of the anchor inside the bounds.
fn room(p: &AnchorParams, side: Side) -> f32 {
    let (a, b) = (p.anchor, p.bounds);
    match side {
        Side::Bottom => b.max.y - a.max.y - p.gap,
        Side::Top => a.min.y - b.min.y - p.gap,
        Side::Right => b.max.x - a.max.x - p.gap,
        Side::Left => a.min.x - b.min.x - p.gap,
    }
}

fn finite(r: &Rect) -> bool {
    r.min.x.is_finite() && r.min.y.is_finite() && r.max.x.is_finite() && r.max.y.is_finite()
}

/// Places a surface of `p.size` next to `p.anchor` (Floating UI's `offset`, `flip`, `shift`, `size`).
pub fn place(p: &AnchorParams) -> Placed {
    let mut size = p.size;
    if p.match_anchor_width {
        size.w = size.w.max(p.anchor.width());
    }
    let mut side = p.placement.side;
    let need = if side.vertical() { size.h } else { size.w };
    if p.flip && room(p, side) < need {
        let other = side.opposite();
        if room(p, other) > room(p, side) {
            side = other;
        }
    }
    if p.fit && finite(&p.bounds) {
        let r = room(p, side).max(0.0);
        if side.vertical() {
            size.h = size.h.min(r);
            size.w = size.w.min(p.bounds.width());
        } else {
            size.w = size.w.min(r);
            size.h = size.h.min(p.bounds.height());
        }
    }
    let mut min = at(p, side, size);
    if p.shift && finite(&p.bounds) {
        let b = p.bounds;
        // never past the far edge, and never before the near one (a surface larger than the bounds
        // starts at their start)
        min.x = min.x.min(b.max.x - size.w).max(b.min.x);
        min.y = min.y.min(b.max.y - size.h).max(b.min.y);
    }
    Placed { side, rect: Rect::from_origin_size(min, size) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_origin_size(Point::new(x, y), Size::new(w, h))
    }

    fn p(anchor: Rect, size: Size, placement: &str) -> AnchorParams {
        AnchorParams {
            anchor,
            size,
            bounds: r(0.0, 0.0, 400.0, 300.0),
            placement: Placement::parse(placement).unwrap(),
            gap: 2.0,
            ..AnchorParams::default()
        }
    }

    #[test]
    fn placements_align_on_each_side() {
        let a = r(100.0, 100.0, 50.0, 20.0);
        let s = Size::new(80.0, 40.0);
        assert_eq!(place(&p(a, s, "bottom-start")).rect.min, Point::new(100.0, 122.0));
        assert_eq!(place(&p(a, s, "bottom-end")).rect.min, Point::new(70.0, 122.0));
        assert_eq!(place(&p(a, s, "bottom-center")).rect.min, Point::new(85.0, 122.0));
        assert_eq!(place(&p(a, s, "top")).rect.min, Point::new(100.0, 58.0));
        assert_eq!(place(&p(a, s, "right-start")).rect.min, Point::new(152.0, 100.0));
        assert_eq!(place(&p(a, s, "left-end")).rect.min, Point::new(18.0, 80.0));
    }

    #[test]
    fn flips_when_the_other_side_has_more_room() {
        let low = r(100.0, 270.0, 50.0, 20.0);
        let placed = place(&p(low, Size::new(80.0, 40.0), "bottom-start"));
        assert_eq!((placed.side, placed.rect.max.y), (Side::Top, 268.0));
        // a submenu at the right edge flips left
        let right = r(330.0, 10.0, 60.0, 20.0);
        let placed = place(&p(right, Size::new(100.0, 50.0), "right-start"));
        assert_eq!((placed.side, placed.rect.max.x), (Side::Left, 328.0));
        // no flip asked: stays, shifted back inside
        let mut q = p(low, Size::new(80.0, 40.0), "bottom-start");
        q.flip = false;
        let placed = place(&q);
        assert_eq!((placed.side, placed.rect.max.y), (Side::Bottom, 300.0));
    }

    #[test]
    fn shift_keeps_it_inside_on_the_cross_axis() {
        let right = r(330.0, 10.0, 60.0, 20.0);
        let placed = place(&p(right, Size::new(100.0, 50.0), "bottom-start"));
        assert_eq!(placed.rect.max.x, 400.0);
        // larger than the bounds: starts at their start
        let placed = place(&p(right, Size::new(500.0, 50.0), "bottom-start"));
        assert_eq!(placed.rect.min.x, 0.0);
    }

    #[test]
    fn fit_shrinks_a_long_menu_to_the_room() {
        let a = r(10.0, 100.0, 60.0, 20.0);
        let mut q = p(a, Size::new(120.0, 600.0), "bottom-start");
        q.fit = true;
        let placed = place(&q);
        assert_eq!(placed.side, Side::Bottom, "below has 178 pt, above 98 pt");
        assert_eq!(placed.rect.height(), 178.0);
        assert_eq!(placed.rect.max.y, 300.0);
    }

    #[test]
    fn a_point_anchor_and_anchor_width() {
        // a context menu at the pointer
        let pt = r(390.0, 290.0, 0.0, 0.0);
        let placed = place(&p(pt, Size::new(100.0, 80.0), "bottom-start"));
        assert_eq!(placed.side, Side::Top);
        assert_eq!(placed.rect.max.x, 400.0);
        let mut q = p(r(100.0, 100.0, 50.0, 20.0), Size::new(30.0, 40.0), "bottom-start");
        q.match_anchor_width = true;
        assert_eq!(place(&q).rect.width(), 50.0);
        assert_eq!(Placement::parse("left-center"), Some(Placement { side: Side::Left, align: Align::Center }));
        assert_eq!(Placement::parse("middle"), None);
    }
}
