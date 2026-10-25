//! Geometry in device-independent points (`f32`). A window's scale factor turns points into device
//! pixels; renderer and rasteriser work in device pixels, everything above them in points.

use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

/// An axis-aligned rectangle, `min` inclusive, `max` exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

/// Per-corner radii: top-left, top-right, bottom-right, bottom-left (CSS order).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoundedRect {
    pub rect: Rect,
    pub radii: Corners,
}

/// Edge widths or paddings: top, right, bottom, left (CSS order).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// A 2D affine transform `[a c e; b d f; 0 0 1]` (column-vector convention, as CSS `matrix(a,b,c,d,e,f)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

/// A window's scale: device pixels per point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale(pub f32);

/// A rectangle in device pixels (integers), for scissors, damage and surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// A size in device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DeviceSize {
    pub w: u32,
    pub h: u32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }
    pub fn to_vec(self) -> Vec2 {
        Vec2 { x: self.x, y: self.y }
    }
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl Size {
    pub const ZERO: Size = Size { w: 0.0, h: 0.0 };
    pub const fn new(w: f32, h: f32) -> Self {
        Size { w, h }
    }
}

impl Add<Vec2> for Point {
    type Output = Point;
    fn add(self, v: Vec2) -> Point {
        Point::new(self.x + v.x, self.y + v.y)
    }
}
impl Sub<Vec2> for Point {
    type Output = Point;
    fn sub(self, v: Vec2) -> Point {
        Point::new(self.x - v.x, self.y - v.y)
    }
}
impl Sub for Point {
    type Output = Vec2;
    fn sub(self, o: Point) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Rect {
    pub const ZERO: Rect = Rect { min: Point::ZERO, max: Point::ZERO };
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Rect { min: Point { x: x0, y: y0 }, max: Point { x: x1, y: y1 } }
    }
    pub fn from_origin_size(o: Point, s: Size) -> Self {
        Rect::new(o.x, o.y, o.x + s.w, o.y + s.h)
    }
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }
    pub fn is_empty(&self) -> bool {
        !(self.max.x > self.min.x && self.max.y > self.min.y)
    }
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min.x && p.y >= self.min.y && p.x < self.max.x && p.y < self.max.y
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.min.x < o.max.x && o.min.x < self.max.x && self.min.y < o.max.y && o.min.y < self.max.y
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        Rect::new(self.min.x.max(o.min.x), self.min.y.max(o.min.y), self.max.x.min(o.max.x), self.max.y.min(o.max.y))
    }
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        Rect::new(self.min.x.min(o.min.x), self.min.y.min(o.min.y), self.max.x.max(o.max.x), self.max.y.max(o.max.y))
    }
    pub fn translate(&self, v: Vec2) -> Rect {
        Rect { min: self.min + v, max: self.max + v }
    }
    pub fn inflate(&self, d: f32) -> Rect {
        Rect::new(self.min.x - d, self.min.y - d, self.max.x + d, self.max.y + d)
    }
    pub fn inset(&self, i: Insets) -> Rect {
        Rect::new(self.min.x + i.left, self.min.y + i.top, self.max.x - i.right, self.max.y - i.bottom)
    }
    /// The smallest device rect covering this rect at `scale`.
    pub fn to_device(&self, scale: Scale) -> DeviceRect {
        let x0 = (self.min.x * scale.0).floor() as i32;
        let y0 = (self.min.y * scale.0).floor() as i32;
        let x1 = (self.max.x * scale.0).ceil() as i32;
        let y1 = (self.max.y * scale.0).ceil() as i32;
        DeviceRect { x: x0, y: y0, w: (x1 - x0).max(0) as u32, h: (y1 - y0).max(0) as u32 }
    }
}

impl Corners {
    pub const ZERO: Corners = Corners { tl: 0.0, tr: 0.0, br: 0.0, bl: 0.0 };
    pub const fn all(r: f32) -> Self {
        Corners { tl: r, tr: r, br: r, bl: r }
    }
    pub fn is_zero(&self) -> bool {
        self.tl == 0.0 && self.tr == 0.0 && self.br == 0.0 && self.bl == 0.0
    }
}

impl Insets {
    pub const ZERO: Insets = Insets { top: 0.0, right: 0.0, bottom: 0.0, left: 0.0 };
    pub const fn all(v: f32) -> Self {
        Insets { top: v, right: v, bottom: v, left: v }
    }
}

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };
    pub const fn translate(x: f32, y: f32) -> Self {
        Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: x, f: y }
    }
    pub const fn scale(sx: f32, sy: f32) -> Self {
        Affine { a: sx, b: 0.0, c: 0.0, d: sy, e: 0.0, f: 0.0 }
    }
    pub fn rotate(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    /// `self ∘ other`: apply `other` first, then `self`.
    pub fn then(&self, other: &Affine) -> Affine {
        // self * other
        Affine {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }
    pub fn apply(&self, p: Point) -> Point {
        Point::new(self.a * p.x + self.c * p.y + self.e, self.b * p.x + self.d * p.y + self.f)
    }
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < f32::EPSILON {
            return None;
        }
        let inv = 1.0 / det;
        Some(Affine {
            a: self.d * inv,
            b: -self.b * inv,
            c: -self.c * inv,
            d: self.a * inv,
            e: (self.c * self.f - self.d * self.e) * inv,
            f: (self.b * self.e - self.a * self.f) * inv,
        })
    }
    pub fn is_translation(&self) -> bool {
        self.a == 1.0 && self.b == 0.0 && self.c == 0.0 && self.d == 1.0
    }
    /// The bounding box of `r` transformed.
    pub fn apply_rect(&self, r: &Rect) -> Rect {
        let ps = [
            self.apply(r.min),
            self.apply(Point::new(r.max.x, r.min.y)),
            self.apply(r.max),
            self.apply(Point::new(r.min.x, r.max.y)),
        ];
        let mut out = Rect { min: ps[0], max: ps[0] };
        for p in &ps[1..] {
            out.min.x = out.min.x.min(p.x);
            out.min.y = out.min.y.min(p.y);
            out.max.x = out.max.x.max(p.x);
            out.max.y = out.max.y.max(p.y);
        }
        out
    }
}

impl DeviceRect {
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
    pub fn union(&self, o: &DeviceRect) -> DeviceRect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = (self.x + self.w as i32).max(o.x + o.w as i32);
        let y1 = (self.y + self.h as i32).max(o.y + o.h as i32);
        DeviceRect { x: x0, y: y0, w: (x1 - x0) as u32, h: (y1 - y0) as u32 }
    }
    pub fn clip_to(&self, size: DeviceSize) -> DeviceRect {
        let x0 = self.x.clamp(0, size.w as i32);
        let y0 = self.y.clamp(0, size.h as i32);
        let x1 = (self.x + self.w as i32).clamp(0, size.w as i32);
        let y1 = (self.y + self.h as i32).clamp(0, size.h as i32);
        DeviceRect { x: x0, y: y0, w: (x1 - x0).max(0) as u32, h: (y1 - y0).max(0) as u32 }
    }
}
