//! Slug GPU text (architecture §9.5; Eric Lengyel, "GPU-Centered Font Rendering Directly from Glyph
//! Outlines", JCGT 2017; the method's patent is public domain since 2026-03-17): glyphs are drawn from
//! their quadratic Bézier outlines in the fragment shader, at any scale, with no atlas.
//!
//! This module is the CPU side: a glyph outline becomes quadratic curves in ems and two sets of
//! bands. A horizontal band lists the curves whose `y` range overlaps it, sorted by their largest `x`
//! (descending), so a ray cast to `+x` from a pixel stops at the first curve entirely to its left; a
//! vertical band does the same along `y`. [`coverage`] is the shader's per-pixel computation in Rust
//! (root classification, the two rays, their distance weights), which is what the tests check; the
//! WGSL port is `fs_glyph` in `shaders.wgsl`.

/// Bands per axis, at most.
pub const MAX_BANDS: usize = 16;

/// One glyph ready for the GPU: curves in ems (`y` up), bands over its bounds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlugGlyph {
    /// `[x0, y0, x1, y1]` in ems; empty glyphs (spaces) have `x0 > x1`.
    pub bbox: [f32; 4],
    /// Quadratic curves `[p0.x, p0.y, p1.x, p1.y, p2.x, p2.y]` (p1 the control point).
    pub curves: Vec<[f32; 6]>,
    /// Horizontal bands bottom to top: curve indices, largest `x` first.
    pub hbands: Vec<Vec<u32>>,
    /// Vertical bands left to right: curve indices, largest `y` first.
    pub vbands: Vec<Vec<u32>>,
}

impl SlugGlyph {
    pub fn is_empty(&self) -> bool {
        self.curves.is_empty()
    }
}

/// Collects an outline (in font units) as quadratic curves in ems. Lines become quadratics with the
/// control point at their middle; cubics are split into quadratics.
#[derive(Debug)]
pub struct OutlineBuilder {
    scale: f32,
    start: [f32; 2],
    cur: [f32; 2],
    curves: Vec<[f32; 6]>,
}

/// Quadratics per cubic (each the midpoint approximation of a quarter of the cubic).
const CUBIC_SPLITS: usize = 4;

impl OutlineBuilder {
    pub fn new(units_per_em: f32) -> Self {
        OutlineBuilder { scale: 1.0 / units_per_em.max(1.0), start: [0.0; 2], cur: [0.0; 2], curves: Vec::new() }
    }

    fn p(&self, x: f32, y: f32) -> [f32; 2] {
        [x * self.scale, y * self.scale]
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.start = self.p(x, y);
        self.cur = self.start;
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.line(p);
    }

    fn line(&mut self, p: [f32; 2]) {
        let a = self.cur;
        if a != p {
            let m = [(a[0] + p[0]) * 0.5, (a[1] + p[1]) * 0.5];
            self.curves.push([a[0], a[1], m[0], m[1], p[0], p[1]]);
        }
        self.cur = p;
    }

    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let c = self.p(cx, cy);
        let p = self.p(x, y);
        let a = self.cur;
        self.curves.push([a[0], a[1], c[0], c[1], p[0], p[1]]);
        self.cur = p;
    }

    pub fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        let p0 = self.cur;
        let p1 = self.p(c1x, c1y);
        let p2 = self.p(c2x, c2y);
        let p3 = self.p(x, y);
        let at = |t: f32| {
            let u = 1.0 - t;
            let f = |i: usize| u * u * u * p0[i] + 3.0 * u * u * t * p1[i] + 3.0 * u * t * t * p2[i] + t * t * t * p3[i];
            [f(0), f(1)]
        };
        let tangent = |t: f32| {
            let u = 1.0 - t;
            let f = |i: usize| 3.0 * (u * u * (p1[i] - p0[i]) + 2.0 * u * t * (p2[i] - p1[i]) + t * t * (p3[i] - p2[i]));
            [f(0), f(1)]
        };
        for k in 0..CUBIC_SPLITS {
            let (t0, t1) = (k as f32 / CUBIC_SPLITS as f32, (k + 1) as f32 / CUBIC_SPLITS as f32);
            let (a, b) = (at(t0), at(t1));
            // The control point where the end tangents meet, approximated by the midpoint rule:
            // c = (3(c1 + c2) − (a + b)) / 4 over the piece's Bézier points.
            let h = (t1 - t0) / 3.0;
            let (ta, tb) = (tangent(t0), tangent(t1));
            let c1 = [a[0] + ta[0] * h, a[1] + ta[1] * h];
            let c2 = [b[0] - tb[0] * h, b[1] - tb[1] * h];
            let c = [(3.0 * (c1[0] + c2[0]) - a[0] - b[0]) * 0.25, (3.0 * (c1[1] + c2[1]) - a[1] - b[1]) * 0.25];
            self.curves.push([a[0], a[1], c[0], c[1], b[0], b[1]]);
        }
        self.cur = p3;
    }

    /// Closes the current contour with a line back to its start.
    pub fn close(&mut self) {
        if self.cur != self.start {
            let s = self.start;
            self.line(s);
        }
    }

    pub fn finish(mut self) -> SlugGlyph {
        self.close();
        build(self.curves)
    }
}

/// Bands for `curves` (ems).
pub fn build(curves: Vec<[f32; 6]>) -> SlugGlyph {
    if curves.is_empty() {
        return SlugGlyph { bbox: [1.0, 1.0, 0.0, 0.0], ..Default::default() };
    }
    let mut bbox = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
    for c in &curves {
        for i in 0..3 {
            bbox[0] = bbox[0].min(c[2 * i]);
            bbox[1] = bbox[1].min(c[2 * i + 1]);
            bbox[2] = bbox[2].max(c[2 * i]);
            bbox[3] = bbox[3].max(c[2 * i + 1]);
        }
    }
    let n = ((curves.len() as f32).sqrt().ceil() as usize).clamp(1, MAX_BANDS);
    let range = |c: &[f32; 6], axis: usize| {
        let v = [c[axis], c[2 + axis], c[4 + axis]];
        (v[0].min(v[1]).min(v[2]), v[0].max(v[1]).max(v[2]))
    };
    let bands = |axis: usize| -> Vec<Vec<u32>> {
        let (lo, hi) = (bbox[axis], bbox[axis + 2]);
        let size = ((hi - lo) / n as f32).max(f32::MIN_POSITIVE);
        let eps = size * 1e-3;
        // A horizontal band (axis 1) sorts by the other axis (x), and vice versa.
        let other = 1 - axis;
        (0..n)
            .map(|j| {
                let b0 = lo + j as f32 * size - eps;
                let b1 = lo + (j + 1) as f32 * size + eps;
                let mut list: Vec<u32> = (0..curves.len() as u32)
                    .filter(|&i| {
                        let (a, b) = range(&curves[i as usize], axis);
                        b >= b0 && a <= b1
                    })
                    .collect();
                list.sort_by(|&a, &b| {
                    let ka = range(&curves[a as usize], other).1;
                    let kb = range(&curves[b as usize], other).1;
                    kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
                });
                list
            })
            .collect()
    };
    let hbands = bands(1);
    let vbands = bands(0);
    SlugGlyph { bbox, curves, hbands, vbands }
}

/// Which roots of a quadratic crossing the ray's line are crossings, from the signs of the three
/// control values relative to the ray: bit 0 = the first root, bit 8 = the second (Slug's table
/// `0x2E74`, indexed by the three sign bits).
pub fn root_code(y1: f32, y2: f32, y3: f32) -> u32 {
    let i1 = y1.to_bits() >> 31;
    let i2 = y2.to_bits() >> 30;
    let i3 = y3.to_bits() >> 29;
    let mut shift = (i2 & 2) | (i1 & !2);
    shift = (i3 & 4) | (shift & !4);
    (0x2E74u32 >> shift) & 0x0101
}

/// The `x` of the two roots of `y(t) = 0` for the quadratic `(p1, p2, p3)` relative to the pixel.
fn solve_x(p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> (f32, f32) {
    let a = [p1[0] - 2.0 * p2[0] + p3[0], p1[1] - 2.0 * p2[1] + p3[1]];
    let b = [p1[0] - p2[0], p1[1] - p2[1]];
    let ra = 1.0 / a[1];
    let rb = 0.5 / b[1];
    let d = (b[1] * b[1] - a[1] * p1[1]).max(0.0).sqrt();
    let (mut t1, mut t2) = ((b[1] - d) * ra, (b[1] + d) * ra);
    if a[1].abs() < 1.0 / 65536.0 {
        t1 = p1[1] * rb;
        t2 = t1;
    }
    let x = |t: f32| (a[0] * t - b[0] * 2.0) * t + p1[0];
    (x(t1), x(t2))
}

/// The band of a coordinate.
fn band(v: f32, lo: f32, hi: f32, n: usize) -> usize {
    let size = (hi - lo) / n as f32;
    if size <= 0.0 {
        return 0;
    }
    (((v - lo) / size).floor().max(0.0) as usize).min(n - 1)
}

/// Coverage (0..=1) of the pixel centred at `p` (ems) with `ppem` pixels per em along each axis: the
/// computation the glyph shader makes, for tests and for CPU fallbacks.
pub fn coverage(g: &SlugGlyph, p: [f32; 2], ppem: [f32; 2]) -> f32 {
    if g.is_empty() {
        return 0.0;
    }
    let rel = |c: &[f32; 6]| ([c[0] - p[0], c[1] - p[1]], [c[2] - p[0], c[3] - p[1]], [c[4] - p[0], c[5] - p[1]]);
    let (mut xcov, mut xwgt) = (0.0f32, 0.0f32);
    let hb = &g.hbands[band(p[1], g.bbox[1], g.bbox[3], g.hbands.len())];
    for &i in hb {
        let (a, b, c) = rel(&g.curves[i as usize]);
        if a[0].max(b[0]).max(c[0]) * ppem[0] < -0.5 {
            break;
        }
        let code = root_code(a[1], b[1], c[1]);
        if code != 0 {
            let (r1, r2) = solve_x(a, b, c);
            let (r1, r2) = (r1 * ppem[0], r2 * ppem[0]);
            if code & 1 != 0 {
                xcov += (r1 + 0.5).clamp(0.0, 1.0);
                xwgt = xwgt.max((1.0 - r1.abs() * 2.0).clamp(0.0, 1.0));
            }
            if code > 1 {
                xcov -= (r2 + 0.5).clamp(0.0, 1.0);
                xwgt = xwgt.max((1.0 - r2.abs() * 2.0).clamp(0.0, 1.0));
            }
        }
    }
    let (mut ycov, mut ywgt) = (0.0f32, 0.0f32);
    let vb = &g.vbands[band(p[0], g.bbox[0], g.bbox[2], g.vbands.len())];
    let swap = |v: [f32; 2]| [v[1], v[0]];
    for &i in vb {
        let (a, b, c) = rel(&g.curves[i as usize]);
        if a[1].max(b[1]).max(c[1]) * ppem[1] < -0.5 {
            break;
        }
        let code = root_code(a[0], b[0], c[0]);
        if code != 0 {
            let (r1, r2) = solve_x(swap(a), swap(b), swap(c));
            let (r1, r2) = (r1 * ppem[1], r2 * ppem[1]);
            // Rotating the ray a quarter turn flips the crossing direction.
            if code & 1 != 0 {
                ycov -= (r1 + 0.5).clamp(0.0, 1.0);
                ywgt = ywgt.max((1.0 - r1.abs() * 2.0).clamp(0.0, 1.0));
            }
            if code > 1 {
                ycov += (r2 + 0.5).clamp(0.0, 1.0);
                ywgt = ywgt.max((1.0 - r2.abs() * 2.0).clamp(0.0, 1.0));
            }
        }
    }
    let weighted = (xcov * xwgt + ycov * ywgt).abs() / (xwgt + ywgt).max(1.0 / 65536.0);
    weighted.max(xcov.abs().min(ycov.abs())).clamp(0.0, 1.0)
}

/// The GPU form of glyphs: one record per glyph, curves as two `vec4`s each, band headers and lists
/// as `u32` words. Appending never moves earlier glyphs.
#[derive(Clone, Debug, Default)]
pub struct GlyphTable {
    pub records: Vec<GlyphRecord>,
    pub curves: Vec<[f32; 4]>,
    pub bands: Vec<u32>,
}

/// One glyph's record (std430: two 16-byte fields).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphRecord {
    pub bbox: [f32; 4],
    /// Offset of the band headers in `bands`, horizontal band count, vertical band count, flags
    /// (bit 0: empty).
    pub info: [u32; 4],
}

impl GlyphTable {
    /// Appends a glyph; returns its index.
    pub fn push(&mut self, g: &SlugGlyph) -> u32 {
        let index = self.records.len() as u32;
        if g.is_empty() {
            self.records.push(GlyphRecord { bbox: [0.0; 4], info: [0, 0, 0, 1] });
            return index;
        }
        let base = (self.curves.len() / 2) as u32;
        for c in &g.curves {
            self.curves.push([c[0], c[1], c[2], c[3]]);
            self.curves.push([c[4], c[5], 0.0, 0.0]);
        }
        let header = self.bands.len() as u32;
        let (nh, nv) = (g.hbands.len(), g.vbands.len());
        self.bands.resize(self.bands.len() + 2 * (nh + nv), 0);
        for (k, list) in g.hbands.iter().chain(g.vbands.iter()).enumerate() {
            let start = self.bands.len() as u32;
            self.bands.extend(list.iter().map(|i| i + base));
            self.bands[header as usize + 2 * k] = start;
            self.bands[header as usize + 2 * k + 1] = list.len() as u32;
        }
        self.records.push(GlyphRecord { bbox: g.bbox, info: [header, nh as u32, nv as u32, 0] });
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square from (0.2, 0.0) to (0.8, 0.6) ems, wound counter-clockwise (y up) like TrueType holes
    /// or clockwise like outer contours, per `ccw`.
    fn square(ccw: bool) -> SlugGlyph {
        let mut b = OutlineBuilder::new(1.0);
        let pts = [(0.2, 0.0), (0.8, 0.0), (0.8, 0.6), (0.2, 0.6)];
        let order: Vec<_> = if ccw { pts.to_vec() } else { pts.iter().rev().copied().collect() };
        b.move_to(order[0].0, order[0].1);
        for p in &order[1..] {
            b.line_to(p.0, p.1);
        }
        b.finish()
    }

    #[test]
    fn root_codes_classify_crossings() {
        assert_eq!(root_code(1.0, 2.0, 3.0), 0, "all above");
        assert_eq!(root_code(-1.0, -2.0, -3.0), 0, "all below");
        assert_eq!(root_code(1.0, 0.0, -1.0), 1, "downward: first root");
        assert_eq!(root_code(-1.0, 0.0, 1.0), 0x100, "upward: second root");
        assert_eq!(root_code(1.0, -1.0, 1.0), 0x101, "dips below: both");
    }

    #[test]
    fn a_square_is_covered_inside_and_antialiased_at_its_edges() {
        for ccw in [true, false] {
            let g = square(ccw);
            let ppem = [100.0, 100.0];
            assert_eq!(coverage(&g, [0.5, 0.3], ppem), 1.0, "inside (ccw {ccw})");
            assert_eq!(coverage(&g, [0.1, 0.3], ppem), 0.0, "left (ccw {ccw})");
            assert_eq!(coverage(&g, [0.9, 0.3], ppem), 0.0, "right (ccw {ccw})");
            assert_eq!(coverage(&g, [0.5, 0.7], ppem), 0.0, "above (ccw {ccw})");
            // Half a pixel inside the left edge: the pixel centre sits on the edge → half covered.
            let half = coverage(&g, [0.2, 0.3], ppem);
            assert!((half - 0.5).abs() < 0.05, "edge {half} (ccw {ccw})");
        }
    }

    #[test]
    fn holes_cut_out_by_winding() {
        // An outer square and an inner one wound the other way.
        let mut b = OutlineBuilder::new(1.0);
        for (pts, rev) in [([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], false), ([(0.3, 0.3), (0.7, 0.3), (0.7, 0.7), (0.3, 0.7)], true)] {
            let v: Vec<_> = if rev { pts.iter().rev().copied().collect() } else { pts.to_vec() };
            b.move_to(v[0].0, v[0].1);
            for p in &v[1..] {
                b.line_to(p.0, p.1);
            }
        }
        let g = b.finish();
        let ppem = [64.0, 64.0];
        assert_eq!(coverage(&g, [0.15, 0.5], ppem), 1.0);
        assert_eq!(coverage(&g, [0.5, 0.5], ppem), 0.0, "the hole");
    }

    #[test]
    fn curves_and_cubics_cover_a_circle() {
        // A circle of radius 0.4 at (0.5, 0.5) from four cubic arcs.
        let k = 0.5523 * 0.4;
        let mut b = OutlineBuilder::new(1.0);
        b.move_to(0.9, 0.5);
        b.curve_to(0.9, 0.5 + k, 0.5 + k, 0.9, 0.5, 0.9);
        b.curve_to(0.5 - k, 0.9, 0.1, 0.5 + k, 0.1, 0.5);
        b.curve_to(0.1, 0.5 - k, 0.5 - k, 0.1, 0.5, 0.1);
        b.curve_to(0.5 + k, 0.1, 0.9, 0.5 - k, 0.9, 0.5);
        let g = b.finish();
        let ppem = [200.0, 200.0];
        assert_eq!(coverage(&g, [0.5, 0.5], ppem), 1.0);
        assert_eq!(coverage(&g, [0.5 + 0.3 * 0.7071, 0.5 + 0.3 * 0.7071], ppem), 1.0);
        assert_eq!(coverage(&g, [0.5 + 0.45 * 0.7071, 0.5 + 0.45 * 0.7071], ppem), 0.0);
        assert_eq!(coverage(&g, [0.05, 0.05], ppem), 0.0);
    }

    #[test]
    fn bands_hold_every_curve_they_cross() {
        let g = square(true);
        assert_eq!(g.curves.len(), 4);
        for (j, list) in g.hbands.iter().enumerate() {
            // Both vertical edges cross every horizontal band.
            assert!(list.len() >= 2, "band {j}");
        }
        let mut t = GlyphTable::default();
        assert_eq!(t.push(&g), 0);
        assert_eq!(t.push(&SlugGlyph::default()), 1);
        assert_eq!(t.records[1].info[3], 1, "empty");
        assert_eq!(t.curves.len(), 8);
    }
}
