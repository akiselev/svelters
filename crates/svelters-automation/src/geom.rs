//! Points, vectors and rects in the scene's points (y down), egui-shaped so the egui scene converts
//! without loss (feature `egui`: `From` both ways) and other scenes (skelter) need no egui.

use std::ops::{Add, Sub};

/// A point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

/// A vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

pub const fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2 { x, y }
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Pos2 {
    pub const ZERO: Pos2 = pos2(0.0, 0.0);

    pub fn to_vec2(self) -> Vec2 {
        vec2(self.x, self.y)
    }

    /// The point a fraction `t` of the way to `to`.
    pub fn lerp(self, to: Pos2, t: f32) -> Pos2 {
        pos2(self.x + (to.x - self.x) * t, self.y + (to.y - self.y) * t)
    }
}

impl Vec2 {
    pub const ZERO: Vec2 = vec2(0.0, 0.0);
}

impl Add<Vec2> for Pos2 {
    type Output = Pos2;
    fn add(self, v: Vec2) -> Pos2 {
        pos2(self.x + v.x, self.y + v.y)
    }
}

impl Sub<Vec2> for Pos2 {
    type Output = Pos2;
    fn sub(self, v: Vec2) -> Pos2 {
        pos2(self.x - v.x, self.y - v.y)
    }
}

impl Sub for Pos2 {
    type Output = Vec2;
    fn sub(self, p: Pos2) -> Vec2 {
        vec2(self.x - p.x, self.y - p.y)
    }
}

/// An axis-aligned rect: `min` is its top left, `max` its bottom right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Pos2,
    pub max: Pos2,
}

impl Rect {
    /// Nothing: contains no point, and any rect's intersection with it is negative.
    pub const NOTHING: Rect = Rect { min: pos2(f32::INFINITY, f32::INFINITY), max: pos2(f32::NEG_INFINITY, f32::NEG_INFINITY) };

    pub const fn from_min_max(min: Pos2, max: Pos2) -> Rect {
        Rect { min, max }
    }

    pub fn from_min_size(min: Pos2, size: Vec2) -> Rect {
        Rect { min, max: min + size }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.width(), self.height())
    }

    pub fn left(&self) -> f32 {
        self.min.x
    }

    pub fn top(&self) -> f32 {
        self.min.y
    }

    pub fn right(&self) -> f32 {
        self.max.x
    }

    pub fn bottom(&self) -> f32 {
        self.max.y
    }

    pub fn center(&self) -> Pos2 {
        pos2((self.min.x + self.max.x) / 2.0, (self.min.y + self.max.y) / 2.0)
    }

    /// Width × height (negative or zero for an empty rect).
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Both sides are positive.
    pub fn is_positive(&self) -> bool {
        self.width() > 0.0 && self.height() > 0.0
    }

    /// A side is negative (the rect is empty and not even a line).
    pub fn is_negative(&self) -> bool {
        self.width() < 0.0 || self.height() < 0.0
    }

    pub fn contains(&self, p: Pos2) -> bool {
        self.min.x <= p.x && p.x <= self.max.x && self.min.y <= p.y && p.y <= self.max.y
    }

    /// `o` is wholly inside.
    pub fn contains_rect(&self, o: Rect) -> bool {
        self.contains(o.min) && self.contains(o.max)
    }

    /// The rects overlap (touching edges count).
    pub fn intersects(&self, o: Rect) -> bool {
        self.min.x <= o.max.x && o.min.x <= self.max.x && self.min.y <= o.max.y && o.min.y <= self.max.y
    }

    /// The overlap (negative when the rects don't overlap).
    pub fn intersect(&self, o: Rect) -> Rect {
        Rect {
            min: pos2(self.min.x.max(o.min.x), self.min.y.max(o.min.y)),
            max: pos2(self.max.x.min(o.max.x), self.max.y.min(o.max.y)),
        }
    }

    /// Grown by `amount` on every side.
    pub fn expand(&self, amount: f32) -> Rect {
        Rect { min: pos2(self.min.x - amount, self.min.y - amount), max: pos2(self.max.x + amount, self.max.y + amount) }
    }

    /// Moved by `v`.
    pub fn translate(&self, v: Vec2) -> Rect {
        Rect { min: self.min + v, max: self.max + v }
    }
}

#[cfg(feature = "egui")]
mod egui_conv {
    use super::*;

    impl From<egui::Pos2> for Pos2 {
        fn from(p: egui::Pos2) -> Pos2 {
            pos2(p.x, p.y)
        }
    }
    impl From<Pos2> for egui::Pos2 {
        fn from(p: Pos2) -> egui::Pos2 {
            egui::pos2(p.x, p.y)
        }
    }
    impl From<egui::Vec2> for Vec2 {
        fn from(v: egui::Vec2) -> Vec2 {
            vec2(v.x, v.y)
        }
    }
    impl From<Vec2> for egui::Vec2 {
        fn from(v: Vec2) -> egui::Vec2 {
            egui::vec2(v.x, v.y)
        }
    }
    impl From<egui::Rect> for Rect {
        fn from(r: egui::Rect) -> Rect {
            Rect { min: r.min.into(), max: r.max.into() }
        }
    }
    impl From<Rect> for egui::Rect {
        fn from(r: Rect) -> egui::Rect {
            egui::Rect::from_min_max(r.min.into(), r.max.into())
        }
    }
}
