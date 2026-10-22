//! `Anchored`: where a floating surface (popup, menu, submenu, select list, tooltip) goes next to
//! its anchor: a side and an alignment (`bottom-start`, `right-start`, …), a gap, then Floating
//! UI's `flip` (the other side when this one has no room and the other has more) and `shift`
//! (slid back inside the bounds). [`place`] is the geometry; the unit reports it as an intent.
//! Writer lane: OW3.

use super::{InputCx, Intents, Unit};
use crate::provider::StateBits;
use egui::{Pos2, Rect, Vec2, pos2};

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
    fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
    fn vertical(self) -> bool {
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

    /// `top|bottom|left|right`, optionally `-start|-center|-end` (no suffix: start, as the
    /// `popup` kind's placements always meant).
    pub fn parse(s: &str) -> Option<Placement> {
        let (side, align) = s.trim().split_once('-').unwrap_or((s.trim(), "start"));
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

/// What [`Anchored`] is given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorParams {
    /// The anchor (screen); a zero-size rect for a point (a context menu at the pointer).
    pub anchor: Rect,
    /// The surface's size.
    pub size: Vec2,
    /// Where it must stay (the viewport).
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
    /// At least as wide as the anchor (a select's list).
    pub match_anchor_width: bool,
}

impl Default for AnchorParams {
    fn default() -> Self {
        AnchorParams { anchor: Rect::NOTHING, size: Vec2::ZERO, bounds: Rect::EVERYTHING, placement: Placement::default(), gap: 0.0, cross: 0.0, flip: true, shift: true, match_anchor_width: false }
    }
}

/// Where the surface went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The side it ended on (after `flip`).
    pub side: Side,
    pub rect: Rect,
}

fn at(p: &AnchorParams, side: Side, size: Vec2) -> Pos2 {
    let a = p.anchor;
    let main = match side {
        Side::Bottom => a.bottom() + p.gap,
        Side::Top => a.top() - p.gap - size.y,
        Side::Right => a.right() + p.gap,
        Side::Left => a.left() - p.gap - size.x,
    };
    let (lo, hi, len) = if side.vertical() { (a.left(), a.right(), size.x) } else { (a.top(), a.bottom(), size.y) };
    let cross = p.cross
        + match p.placement.align {
            Align::Start => lo,
            Align::Center => (lo + hi - len) / 2.0,
            Align::End => hi - len,
        };
    if side.vertical() { pos2(cross, main) } else { pos2(main, cross) }
}

/// The room on `side` of the anchor inside the bounds.
fn room(p: &AnchorParams, side: Side) -> f32 {
    let (a, b) = (p.anchor, p.bounds);
    match side {
        Side::Bottom => b.bottom() - a.bottom() - p.gap,
        Side::Top => a.top() - b.top() - p.gap,
        Side::Right => b.right() - a.right() - p.gap,
        Side::Left => a.left() - b.left() - p.gap,
    }
}

/// Places a surface of `p.size` next to `p.anchor` (Floating UI's `offset`, `flip`, `shift`).
pub fn place(p: &AnchorParams) -> Placed {
    let mut size = p.size;
    if p.match_anchor_width && p.anchor.is_finite() {
        size.x = size.x.max(p.anchor.width());
    }
    let mut side = p.placement.side;
    let need = if side.vertical() { size.y } else { size.x };
    if p.flip && room(p, side) < need {
        let other = side.opposite();
        if room(p, other) > room(p, side) {
            side = other;
        }
    }
    let mut min = at(p, side, size);
    if p.shift && p.bounds.is_finite() {
        let b = p.bounds;
        // never past the far edge, and never before the near one (a surface larger than the
        // bounds starts at their start)
        min.x = min.x.min(b.right() - size.x).max(b.left());
        min.y = min.y.min(b.bottom() - size.y).max(b.top());
    }
    Placed { side, rect: Rect::from_min_size(min, size) }
}

/// Places the host's floating surface each pass (ARCH-WIDGETS §1.2): no state bits, no keys.
#[derive(Clone, Debug, Default)]
pub struct Anchored {
    last: Option<Placed>,
}

impl Anchored {
    /// The last placement.
    pub fn placed(&self) -> Option<Placed> {
        self.last
    }
}

impl Unit for Anchored {
    type Params = AnchorParams;
    type Intent = Placed;
    const OWNS: StateBits = StateBits::NONE;

    fn input(&mut self, p: &AnchorParams, _cx: &mut InputCx<'_>, out: &mut Intents<Placed>) {
        let placed = place(p);
        self.last = Some(placed);
        out.push(placed);
    }
}

/// The screen area surfaces stay in (`content_rect` of the viewport).
pub fn bounds(ctx: &egui::Context) -> Rect {
    ctx.content_rect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn p(anchor: Rect, size: Vec2, placement: &str) -> AnchorParams {
        AnchorParams { anchor, size, bounds: Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0)), placement: Placement::parse(placement).unwrap(), gap: 2.0, ..AnchorParams::default() }
    }

    #[test]
    fn placements_flip_and_shift() {
        let a = Rect::from_min_size(pos2(100.0, 100.0), vec2(50.0, 20.0));
        assert_eq!(place(&p(a, vec2(80.0, 40.0), "bottom-start")).rect.min, pos2(100.0, 122.0));
        assert_eq!(place(&p(a, vec2(80.0, 40.0), "bottom-end")).rect.min, pos2(70.0, 122.0));
        assert_eq!(place(&p(a, vec2(80.0, 40.0), "top")).rect.min, pos2(100.0, 58.0));
        assert_eq!(place(&p(a, vec2(80.0, 40.0), "right-start")).rect.min, pos2(152.0, 100.0));
        // no room below, more above: flipped
        let low = Rect::from_min_size(pos2(100.0, 270.0), vec2(50.0, 20.0));
        let r = place(&p(low, vec2(80.0, 40.0), "bottom-start"));
        assert_eq!((r.side, r.rect.max.y), (Side::Top, 268.0));
        // a submenu at the right edge flips left
        let right = Rect::from_min_size(pos2(330.0, 10.0), vec2(60.0, 20.0));
        let r = place(&p(right, vec2(100.0, 50.0), "right-start"));
        assert_eq!((r.side, r.rect.max.x), (Side::Left, 328.0));
        // shift keeps it inside on the cross axis
        let r = place(&p(right, vec2(100.0, 50.0), "bottom-start"));
        assert_eq!(r.rect.max.x, 400.0);
        // match the anchor's width
        let mut q = p(a, vec2(30.0, 40.0), "bottom-start");
        q.match_anchor_width = true;
        assert_eq!(place(&q).rect.width(), 50.0);
        assert_eq!(Placement::parse("left-center"), Some(Placement { side: Side::Left, align: Align::Center }));
        assert_eq!(Placement::parse("middle"), None);
    }
}
