//! World space (architecture §9.5): `f64` coordinates for the integer-nanometre geometry of the CAD
//! canvases, the [`Camera`] that looks at it, and the projection of a chunk into device pixels.
//!
//! Instances are stored as `f32` offsets from their chunk's `f64` origin. A chunk is projected with
//! one affine whose translation is `(origin − camera.center) · zoom`, computed in `f64` and only then
//! rounded to `f32`: the GPU never sees a large world coordinate, so a feature a nanometre wide stays
//! exact at any distance from the world origin.

use skelter_base::geom::{Affine, Point, Rect};

/// A point (or vector) in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DVec2 {
    pub x: f64,
    pub y: f64,
}

impl DVec2 {
    pub const ZERO: DVec2 = DVec2 { x: 0.0, y: 0.0 };
    pub const fn new(x: f64, y: f64) -> Self {
        DVec2 { x, y }
    }
    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

impl std::ops::Add for DVec2 {
    type Output = DVec2;
    fn add(self, o: DVec2) -> DVec2 {
        DVec2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for DVec2 {
    type Output = DVec2;
    fn sub(self, o: DVec2) -> DVec2 {
        DVec2::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f64> for DVec2 {
    type Output = DVec2;
    fn mul(self, k: f64) -> DVec2 {
        DVec2::new(self.x * k, self.y * k)
    }
}

/// An axis-aligned world rectangle. [`WorldRect::EMPTY`] (min > max) is the identity of `union`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldRect {
    pub min: DVec2,
    pub max: DVec2,
}

impl WorldRect {
    pub const EMPTY: WorldRect =
        WorldRect { min: DVec2 { x: f64::INFINITY, y: f64::INFINITY }, max: DVec2 { x: f64::NEG_INFINITY, y: f64::NEG_INFINITY } };

    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        WorldRect { min: DVec2 { x: x0, y: y0 }, max: DVec2 { x: x1, y: y1 } }
    }
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y
    }
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
    pub fn center(&self) -> DVec2 {
        DVec2::new((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
    }
    pub fn intersects(&self, o: &WorldRect) -> bool {
        self.min.x <= o.max.x && o.min.x <= self.max.x && self.min.y <= o.max.y && o.min.y <= self.max.y
    }
    pub fn union(&self, o: &WorldRect) -> WorldRect {
        WorldRect {
            min: DVec2::new(self.min.x.min(o.min.x), self.min.y.min(o.min.y)),
            max: DVec2::new(self.max.x.max(o.max.x), self.max.y.max(o.max.y)),
        }
    }
    pub fn include(&mut self, p: DVec2) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }
    pub fn inflate(&self, d: f64) -> WorldRect {
        WorldRect::new(self.min.x - d, self.min.y - d, self.max.x + d, self.max.y + d)
    }
}

/// Which way world `y` points on screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum YAxis {
    /// Screen convention: `y` grows downwards (schematics, KiCad's internal units).
    #[default]
    Down,
    /// Mathematical convention: `y` grows upwards (Gerber, most MCAD sketches).
    Up,
}

/// What a canvas looks at: the world point at the centre of the canvas rectangle and the scale. The
/// camera is a compositor property (`CompositorHandle::set_canvas_camera`): panning and zooming
/// change it without a UI commit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center: DVec2,
    /// Points per world unit.
    pub zoom: f64,
    pub y_axis: YAxis,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { center: DVec2::ZERO, zoom: 1.0, y_axis: YAxis::Down }
    }
}

impl Camera {
    pub fn new(center: DVec2, zoom: f64) -> Self {
        Camera { center, zoom, ..Default::default() }
    }

    fn y_sign(&self) -> f64 {
        match self.y_axis {
            YAxis::Down => 1.0,
            YAxis::Up => -1.0,
        }
    }

    /// A world point in canvas points: `(0, 0)` is the canvas rectangle's top-left corner, `size`
    /// its size in points.
    pub fn world_to_view(&self, p: DVec2, size: (f64, f64)) -> (f64, f64) {
        let d = p - self.center;
        (d.x * self.zoom + size.0 * 0.5, d.y * self.zoom * self.y_sign() + size.1 * 0.5)
    }

    /// The inverse of [`Camera::world_to_view`].
    pub fn view_to_world(&self, v: (f64, f64), size: (f64, f64)) -> DVec2 {
        let z = self.zoom.max(f64::MIN_POSITIVE);
        DVec2::new(
            self.center.x + (v.0 - size.0 * 0.5) / z,
            self.center.y + (v.1 - size.1 * 0.5) / (z * self.y_sign()),
        )
    }

    /// Zooms by `factor` keeping the world point under `anchor` (canvas points) where it is: what a
    /// wheel zoom about the pointer does.
    pub fn zoom_about(&self, factor: f64, anchor: (f64, f64), size: (f64, f64)) -> Camera {
        let fixed = self.view_to_world(anchor, size);
        let zoom = self.zoom * factor;
        let mut c = Camera { zoom, ..*self };
        let moved = c.view_to_world(anchor, size);
        c.center = c.center + (fixed - moved);
        c
    }

    /// Pans by a drag of `delta` canvas points (the content follows the pointer).
    pub fn pan_by(&self, delta: (f64, f64)) -> Camera {
        let z = self.zoom.max(f64::MIN_POSITIVE);
        Camera {
            center: self.center - DVec2::new(delta.0 / z, delta.1 / (z * self.y_sign())),
            ..*self
        }
    }
}

/// Where a canvas is drawn this frame: its rectangle in device pixels, the transform from canvas points
/// to device pixels around it, and the camera. Built by the renderer from the display list's
/// `Prim::Canvas`, the item's transform and the compositor's camera and view properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    pub camera: Camera,
    /// The canvas rectangle's size in points.
    pub size: (f64, f64),
    /// Canvas points (`(0, 0)` = the rectangle's top-left) → device pixels of the target.
    pub to_device: [f64; 6],
}

/// A chunk's transform: chunk-local `f32` offsets → device pixels, `p' = m · p + t` with `m` column
/// major `[a, b, c, d]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkXform {
    pub m: [f32; 4],
    pub t: [f32; 2],
    /// Device pixels per world unit (`sqrt |det m|`): what LOD thresholds and minimum widths use.
    pub px_per_unit: f32,
    /// `+1` when world `y` points down on screen, `−1` when up: labels flip their glyphs to stay upright.
    pub y_sign: f32,
}

impl Projection {
    /// `canvas_to_device` maps canvas points (top-left of the canvas rectangle at the origin) to device
    /// pixels, as `[a, b, c, d, e, f]` of a column-vector affine (skelter's [`Affine`] in `f64`).
    pub fn new(camera: Camera, size: (f64, f64), canvas_to_device: [f64; 6]) -> Self {
        Projection { camera, size, to_device: canvas_to_device }
    }

    /// From skelter's `f32` affine: the canvas rectangle's placement and the compositor's view
    /// transform, already composed.
    pub fn from_affine(camera: Camera, size: (f64, f64), m: &Affine) -> Self {
        let a = [m.a, m.b, m.c, m.d, m.e, m.f].map(|v| v as f64);
        Projection::new(camera, size, a)
    }

    fn device(&self, v: (f64, f64)) -> (f64, f64) {
        let m = &self.to_device;
        (m[0] * v.0 + m[2] * v.1 + m[4], m[1] * v.0 + m[3] * v.1 + m[5])
    }

    /// A world point in device pixels.
    pub fn world_to_device(&self, p: DVec2) -> (f64, f64) {
        self.device(self.camera.world_to_view(p, self.size))
    }

    /// The transform of a chunk whose instances are offsets from `origin`.
    pub fn chunk(&self, origin: DVec2) -> ChunkXform {
        let c = &self.camera;
        let ys = c.y_sign();
        let m = &self.to_device;
        // world offset (u, v) → view: (u·z + ox, v·z·ys + oy) with (ox, oy) the origin's view point.
        let (ox, oy) = c.world_to_view(origin, self.size);
        let (sx, sy) = (c.zoom, c.zoom * ys);
        let a = m[0] * sx;
        let b = m[1] * sx;
        let cc = m[2] * sy;
        let d = m[3] * sy;
        let (tx, ty) = self.device((ox, oy));
        let det = (a * d - b * cc).abs();
        ChunkXform {
            m: [a as f32, b as f32, cc as f32, d as f32],
            t: [tx as f32, ty as f32],
            px_per_unit: det.sqrt() as f32,
            y_sign: if (m[0] * m[3] - m[1] * m[2]) * ys >= 0.0 { 1.0 } else { -1.0 },
        }
    }

    /// The world rectangle seen through device rect `r` (its corners mapped back, bounding box).
    pub fn visible_world(&self, r: &Rect) -> WorldRect {
        let m = &self.to_device;
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 1e-300 {
            return WorldRect::EMPTY;
        }
        let inv = |p: Point| -> (f64, f64) {
            let (x, y) = (p.x as f64 - m[4], p.y as f64 - m[5]);
            ((m[3] * x - m[2] * y) / det, (-m[1] * x + m[0] * y) / det)
        };
        let mut out = WorldRect::EMPTY;
        for p in [r.min, Point::new(r.max.x, r.min.y), r.max, Point::new(r.min.x, r.max.y)] {
            out.include(self.camera.view_to_world(inv(p), self.size));
        }
        out
    }

    /// Device pixels per world unit.
    pub fn px_per_unit(&self) -> f64 {
        let m = &self.to_device;
        (m[0] * m[3] - m[1] * m[2]).abs().sqrt() * self.camera.zoom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_round_trip_and_zoom_about_keeps_the_anchor() {
        let c = Camera::new(DVec2::new(1.0e9, -2.5e8), 1e-6);
        let size = (800.0, 600.0);
        let p = DVec2::new(1.0e9 + 1234.0, -2.5e8 + 99.0);
        let v = c.world_to_view(p, size);
        let back = c.view_to_world(v, size);
        assert!((back - p).length() < 1e-6);
        let z = c.zoom_about(3.0, (100.0, 50.0), size);
        let before = c.view_to_world((100.0, 50.0), size);
        let after = z.view_to_world((100.0, 50.0), size);
        assert!((before - after).length() < 1e-3);
    }

    #[test]
    fn chunk_transform_is_exact_far_from_the_origin() {
        // 1 device pixel per nanometre, half a metre from the world origin.
        let cam = Camera::new(DVec2::new(5.0e8, 5.0e8), 1.0);
        let p = Projection::new(cam, (100.0, 100.0), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let x = p.chunk(DVec2::new(5.0e8 + 3.0, 5.0e8 - 7.0));
        assert_eq!(x.t, [53.0, 43.0]);
        assert_eq!(x.m, [1.0, 0.0, 0.0, 1.0]);
        // A 1 nm offset inside the chunk lands one pixel over.
        let dev = |u: f32, v: f32| (x.m[0] * u + x.m[2] * v + x.t[0], x.m[1] * u + x.m[3] * v + x.t[1]);
        assert_eq!(dev(1.0, 0.0), (54.0, 43.0));
    }

    #[test]
    fn y_up_cameras_flip_and_report_it() {
        let cam = Camera { y_axis: YAxis::Up, ..Camera::new(DVec2::ZERO, 2.0) };
        let p = Projection::new(cam, (10.0, 10.0), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let x = p.chunk(DVec2::ZERO);
        assert_eq!(x.m, [2.0, 0.0, 0.0, -2.0]);
        assert_eq!(x.y_sign, -1.0);
        let w = p.visible_world(&Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(w, WorldRect::new(-2.5, -2.5, 2.5, 2.5));
    }
}
