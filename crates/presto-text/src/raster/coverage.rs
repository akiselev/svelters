//! Area-coverage rasterisation of glyph outlines into 8-bit linear coverage masks.
//!
//! The accumulation rasteriser follows font-rs's design (Raph Levien, `src/raster.rs` of
//! raphlinus/font-rs, Apache-2.0; modifications by the svelters authors): each line segment adds
//! the exact signed area it covers to a dense per-pixel accumulation buffer, and a running sum along
//! each row turns the deposits into coverage. Curves are flattened to lines first.
//!
//! Two modes:
//! - **Symmetric** (grayscale): exact area coverage in x and y.
//! - **Horizontal only** (DirectWrite NATURAL): each row samples the outline on its centre
//!   scanline, so coverage is exact in x and binary in y. Every segment crossing a row's centre adds
//!   a unit-height vertical edge at its crossing x, which the row sum turns into exact horizontal
//!   coverage.
//!
//! Everything is scalar `f32` without fused multiply-add, so masks are byte-identical across
//! targets for the same outline.

/// A point in device pixels, y down.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Pt { x, y }
    }
    fn lerp(self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

/// Path commands of one glyph, already in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Move(Pt),
    Line(Pt),
    Quad(Pt, Pt),
    Cubic(Pt, Pt, Pt),
    Close,
}

/// Flattening tolerance in pixels.
const TOLERANCE: f32 = 0.05;

/// Flattens a path into closed polygons (one per contour).
pub fn flatten(cmds: &[Cmd]) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let mut last = Pt::default();
    let mut start = Pt::default();
    let finish = |cur: &mut Vec<Pt>, out: &mut Vec<Vec<Pt>>| {
        if cur.len() > 2 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for c in cmds {
        match *c {
            Cmd::Move(p) => {
                finish(&mut cur, &mut out);
                cur.push(p);
                last = p;
                start = p;
            }
            Cmd::Line(p) => {
                if cur.is_empty() {
                    cur.push(last);
                }
                cur.push(p);
                last = p;
            }
            Cmd::Quad(c1, p) => {
                if cur.is_empty() {
                    cur.push(last);
                }
                let dd = (last.x - 2.0 * c1.x + p.x).hypot(last.y - 2.0 * c1.y + p.y);
                let n = (1 + ((dd / (8.0 * TOLERANCE)).sqrt()) as usize).min(64);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let a = last.lerp(c1, t);
                    let b = c1.lerp(p, t);
                    cur.push(a.lerp(b, t));
                }
                last = p;
            }
            Cmd::Cubic(c1, c2, p) => {
                if cur.is_empty() {
                    cur.push(last);
                }
                let d1 = (last.x - 2.0 * c1.x + c2.x).hypot(last.y - 2.0 * c1.y + c2.y);
                let d2 = (c1.x - 2.0 * c2.x + p.x).hypot(c1.y - 2.0 * c2.y + p.y);
                let dd = d1.max(d2);
                let n = (1 + ((3.0 * dd / (4.0 * TOLERANCE)).sqrt()) as usize).min(96);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let a = last.lerp(c1, t);
                    let b = c1.lerp(c2, t);
                    let cc = c2.lerp(p, t);
                    let ab = a.lerp(b, t);
                    let bc = b.lerp(cc, t);
                    cur.push(ab.lerp(bc, t));
                }
                last = p;
            }
            Cmd::Close => {
                if !cur.is_empty() && cur[0] != last {
                    cur.push(start);
                }
                finish(&mut cur, &mut out);
                last = start;
            }
        }
    }
    finish(&mut cur, &mut out);
    out
}

/// The integer pixel box covering `polys`: (x0, y0, x1, y1), `None` when empty.
pub fn bounds(polys: &[Vec<Pt>]) -> Option<(i32, i32, i32, i32)> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for p in polys.iter().flatten() {
        b = Some(match b {
            None => (p.x, p.y, p.x, p.y),
            Some((x0, y0, x1, y1)) => (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        });
    }
    let (x0, y0, x1, y1) = b?;
    let (x0, y0, x1, y1) = (x0.floor() as i32, y0.floor() as i32, x1.ceil() as i32, y1.ceil() as i32);
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}

/// Offsets every contour point outward by `strength / 2` pixels along the bisector of its two
/// edges (FreeType's `FT_Outline_EmboldenXY` approach with equal x and y strength), so a stem
/// grows by `strength`. The mitre is limited to twice the offset.
pub fn embolden(polys: &mut [Vec<Pt>], strength: f32) {
    if strength <= 0.0 {
        return;
    }
    // Outer contours wind one way for TrueType and the other for CFF: orient by total area.
    let area: f32 = polys.iter().map(|p| signed_area(p)).sum();
    let outward = if area >= 0.0 { 1.0 } else { -1.0 };
    let d = strength * 0.5;
    for poly in polys.iter_mut() {
        let n = poly.len();
        if n < 3 {
            continue;
        }
        let src = poly.clone();
        for i in 0..n {
            let prev = src[(i + n - 1) % n];
            let cur = src[i];
            let next = src[(i + 1) % n];
            let n0 = normal(prev, cur);
            let n1 = normal(cur, next);
            let (mut bx, mut by) = (n0.x + n1.x, n0.y + n1.y);
            let len = bx.hypot(by);
            if len < 1e-6 {
                continue;
            }
            bx /= len;
            by /= len;
            // Scale so the offset along each edge normal is `d`, limited at sharp corners.
            let cos = (bx * n0.x + by * n0.y).max(0.5);
            let k = outward * d / cos;
            poly[i] = Pt::new(cur.x + bx * k, cur.y + by * k);
        }
    }
}

fn signed_area(p: &[Pt]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (p0, p1) = (p[i], p[(i + 1) % p.len()]);
        a += p0.x * p1.y - p1.x * p0.y;
    }
    a * 0.5
}

/// The unit normal of the edge a→b (rotated -90° in y-down space).
fn normal(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx.hypot(dy);
    if len < 1e-9 {
        return Pt::default();
    }
    Pt::new(dy / len, -dx / len)
}

/// Shears points horizontally: `x += tan(angle) * (baseline_y - y)`, the faux italic.
pub fn skew(polys: &mut [Vec<Pt>], degrees: f32) {
    if degrees == 0.0 {
        return;
    }
    let t = degrees.to_radians().tan();
    for p in polys.iter_mut().flatten() {
        p.x -= t * p.y;
    }
}

/// A coverage accumulator for a `w × h` mask whose top-left pixel is at (`ox`, `oy`).
pub struct Accumulator {
    w: usize,
    h: usize,
    stride: usize,
    ox: f32,
    oy: f32,
    a: Vec<f32>,
}

impl Accumulator {
    pub fn new(w: u32, h: u32, ox: i32, oy: i32) -> Self {
        let stride = w as usize + 2;
        Accumulator { w: w as usize, h: h as usize, stride, ox: ox as f32, oy: oy as f32, a: vec![0.0; stride * h as usize] }
    }

    /// Adds every edge of `polys` (closed polygons) with symmetric or horizontal-only coverage.
    pub fn fill(&mut self, polys: &[Vec<Pt>], horizontal_only: bool) {
        for poly in polys {
            for i in 0..poly.len() {
                let p0 = poly[i];
                let p1 = poly[(i + 1) % poly.len()];
                let p0 = Pt::new(p0.x - self.ox, p0.y - self.oy);
                let p1 = Pt::new(p1.x - self.ox, p1.y - self.oy);
                if horizontal_only {
                    self.line_centre_sampled(p0, p1);
                } else {
                    self.line(p0, p1);
                }
            }
        }
    }

    /// Adds a vertical edge of signed height `d` at `x` in row `y`.
    fn deposit_vertical(&mut self, y: usize, x: f32, d: f32) {
        let x = x.clamp(0.0, self.w as f32);
        let xi = x.floor();
        let xf = x - xi;
        let i = y * self.stride + xi as usize;
        self.a[i] += d * (1.0 - xf);
        self.a[i + 1] += d * xf;
    }

    /// Horizontal-only antialiasing: the segment counts in row `y` iff it crosses the row's centre
    /// line `y + 0.5` (half-open, so a vertex on the line counts once).
    fn line_centre_sampled(&mut self, p0: Pt, p1: Pt) {
        if p0.y == p1.y {
            return;
        }
        let (dir, a, b) = if p0.y < p1.y { (1.0, p0, p1) } else { (-1.0, p1, p0) };
        let first = (a.y - 0.5).ceil().max(0.0) as usize;
        let last = ((b.y - 0.5).ceil().max(0.0) as usize).min(self.h);
        let dxdy = (b.x - a.x) / (b.y - a.y);
        for y in first..last {
            let yc = y as f32 + 0.5;
            let x = a.x + (yc - a.y) * dxdy;
            self.deposit_vertical(y, x, dir);
        }
    }

    /// Exact signed area coverage of one segment (font-rs `draw_line`).
    fn line(&mut self, p0: Pt, p1: Pt) {
        if (p0.y - p1.y).abs() <= f32::EPSILON {
            return;
        }
        let (dir, p0, p1) = if p0.y < p1.y { (1.0, p0, p1) } else { (-1.0, p1, p0) };
        let dxdy = (p1.x - p0.x) / (p1.y - p0.y);
        let mut x = p0.x;
        let y0 = p0.y.max(0.0) as usize;
        if p0.y < 0.0 {
            x -= p0.y * dxdy;
        }
        let y_end = (p1.y.ceil().max(0.0) as usize).min(self.h);
        let wmax = self.w as f32;
        for y in y0..y_end {
            let linestart = y * self.stride;
            let dy = ((y + 1) as f32).min(p1.y) - (y as f32).max(p0.y);
            let xnext = x + dxdy * dy;
            let d = dy * dir;
            let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let (x0, x1) = (x0.clamp(0.0, wmax), x1.clamp(0.0, wmax));
            let x0floor = x0.floor();
            let x0i = x0floor as usize;
            let x1ceil = x1.ceil();
            let x1i = x1ceil as usize;
            if x1i <= x0i + 1 {
                let xmf = 0.5 * (x0 + x1) - x0floor;
                self.a[linestart + x0i] += d - d * xmf;
                self.a[linestart + x0i + 1] += d * xmf;
            } else {
                let s = (x1 - x0).recip();
                let x0f = x0 - x0floor;
                let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
                let x1f = x1 - x1ceil + 1.0;
                let am = 0.5 * s * x1f * x1f;
                self.a[linestart + x0i] += d * a0;
                if x1i == x0i + 2 {
                    self.a[linestart + x0i + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - x0f);
                    self.a[linestart + x0i + 1] += d * (a1 - a0);
                    for xi in x0i + 2..x1i - 1 {
                        self.a[linestart + xi] += d * s;
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    self.a[linestart + x1i - 1] += d * (1.0 - a2 - am);
                }
                self.a[linestart + x1i] += d * am;
            }
            x = xnext;
        }
    }

    /// Coverage per pixel in 0..=1 (non-zero winding, clamped), row-major `w × h`.
    pub fn coverage(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.w * self.h);
        for y in 0..self.h {
            let mut acc = 0.0f32;
            let row = &self.a[y * self.stride..(y + 1) * self.stride];
            for v in &row[..self.w] {
                acc += *v;
                out.push(acc.abs().min(1.0));
            }
        }
        out
    }

    /// Coverage as 8-bit linear alpha.
    pub fn to_mask(&self) -> Vec<u8> {
        self.coverage().into_iter().map(|c| (c * 255.0 + 0.5) as u8).collect()
    }
}
