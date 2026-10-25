//! Colour glyphs into premultiplied RGBA8 (sRGB-encoded, the atlas format of architecture §9.3).
//!
//! - **COLRv0/v1**: skrifa walks the paint graph and calls our [`ColorPainter`]. Paint coordinates
//!   are font units, y up; the painter keeps a transform stack, a clip-mask stack (glyph outlines
//!   and clip boxes rasterised with [`super::coverage`]) and a layer stack. Fills evaluate solid
//!   colours and linear, two-point-conical and sweep gradients (pad, repeat, reflect) per pixel
//!   centre, interpolating premultiplied sRGB as browsers do. Layers composite with the Porter-Duff
//!   operators and the separable blend modes of W3C Compositing Level 1; the four non-separable
//!   HSL modes composite as source-over. Palette 0 is used; the foreground colour (palette index
//!   0xFFFF) is opaque black, since a glyph key carries no text colour.
//! - **Bitmaps** (sbix PNG, CBDT PNG/BGRA, EBDT masks): the strike nearest the ppem, decoded and
//!   resampled bilinearly to the ppem, placed from the strike's bearings.
//!
//! Emboldening, skew, hinting and the AA mode do not apply to colour glyphs.

use super::coverage::{self, Accumulator, Cmd, Pt};
use super::FontRasterizer;
use skelter_base::text::{GlyphFormat, GlyphImage, GlyphKey};
use skrifa::bitmap::{BitmapData, BitmapStrikes, Origin};
use skrifa::color::{Brush, ColorGlyph, ColorPainter, ColorStop, CompositeMode, Extend, Transform};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::types::BoundingBox;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// Rasterises `key` as a colour glyph, if the font has COLR data or a bitmap strike for it.
pub(crate) fn rasterize_color(
    r: &FontRasterizer,
    key: &GlyphKey,
    font: &FontRef<'_>,
    coords: &[NormalizedCoord],
) -> Option<GlyphImage> {
    let _ = r;
    let gid = GlyphId::new(key.glyph);
    let ppem = key.ppem_26_6 as f32 / 64.0;
    if let Some(glyph) = font.color_glyphs().get(gid) {
        return paint_colr(key, font, coords, &glyph, ppem);
    }
    bitmap(key, font, ppem)
}

/// The device transform: font units (y up) → pixels (y down), plus the subpixel offset.
#[derive(Clone, Copy)]
struct Device {
    s: f32,
    dx: f32,
}

impl Device {
    fn apply(&self, t: &Transform, x: f32, y: f32) -> Pt {
        let (fx, fy) = (t.xx * x + t.xy * y + t.dx, t.yx * x + t.yy * y + t.dy);
        Pt::new(fx * self.s + self.dx, -fy * self.s)
    }
}

fn mul(a: &Transform, b: &Transform) -> Transform {
    // a ∘ b: apply b first.
    Transform {
        xx: a.xx * b.xx + a.xy * b.yx,
        yx: a.yx * b.xx + a.yy * b.yx,
        xy: a.xx * b.xy + a.xy * b.yy,
        yy: a.yx * b.xy + a.yy * b.yy,
        dx: a.xx * b.dx + a.xy * b.dy + a.dx,
        dy: a.yx * b.dx + a.yy * b.dy + a.dy,
    }
}

fn invert(t: &Transform) -> Option<Transform> {
    let det = t.xx * t.yy - t.xy * t.yx;
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    Some(Transform {
        xx: t.yy * inv,
        yx: -t.yx * inv,
        xy: -t.xy * inv,
        yy: t.xx * inv,
        dx: (t.xy * t.dy - t.yy * t.dx) * inv,
        dy: (t.yx * t.dx - t.xx * t.dy) * inv,
    })
}

const IDENTITY: Transform = Transform { xx: 1.0, yx: 0.0, xy: 0.0, yy: 1.0, dx: 0.0, dy: 0.0 };

/// Outline commands of `gid` in font units, transformed to device pixels.
fn glyph_cmds(font: &FontRef<'_>, coords: &[NormalizedCoord], gid: GlyphId, t: &Transform, d: Device) -> Vec<Cmd> {
    struct Pen<'a> {
        cmds: Vec<Cmd>,
        t: &'a Transform,
        d: Device,
    }
    impl OutlinePen for Pen<'_> {
        fn move_to(&mut self, x: f32, y: f32) {
            let p = self.d.apply(self.t, x, y);
            self.cmds.push(Cmd::Move(p));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            let p = self.d.apply(self.t, x, y);
            self.cmds.push(Cmd::Line(p));
        }
        fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
            let c = self.d.apply(self.t, cx0, cy0);
            let p = self.d.apply(self.t, x, y);
            self.cmds.push(Cmd::Quad(c, p));
        }
        fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
            let c0 = self.d.apply(self.t, cx0, cy0);
            let c1 = self.d.apply(self.t, cx1, cy1);
            let p = self.d.apply(self.t, x, y);
            self.cmds.push(Cmd::Cubic(c0, c1, p));
        }
        fn close(&mut self) {
            self.cmds.push(Cmd::Close);
        }
    }
    let mut pen = Pen { cmds: Vec::new(), t, d };
    if let Some(g) = font.outline_glyphs().get(gid) {
        let _ = g.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::new(coords)), &mut pen);
    }
    pen.cmds
}

/// First pass: the device-pixel bounds of everything the glyph paints (union of its clip glyphs,
/// limited by its clip boxes).
struct BoundsPainter<'a> {
    font: &'a FontRef<'a>,
    coords: &'a [NormalizedCoord],
    d: Device,
    transforms: Vec<Transform>,
    bounds: Option<(i32, i32, i32, i32)>,
}

impl BoundsPainter<'_> {
    fn top(&self) -> Transform {
        *self.transforms.last().unwrap_or(&IDENTITY)
    }
}

impl ColorPainter for BoundsPainter<'_> {
    fn push_transform(&mut self, t: Transform) {
        let m = mul(&self.top(), &t);
        self.transforms.push(m);
    }
    fn pop_transform(&mut self) {
        self.transforms.pop();
    }
    fn push_clip_glyph(&mut self, gid: GlyphId) {
        let polys = coverage::flatten(&glyph_cmds(self.font, self.coords, gid, &self.top(), self.d));
        if let Some(b) = coverage::bounds(&polys) {
            self.bounds = Some(match self.bounds {
                None => b,
                Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
            });
        }
    }
    fn push_clip_box(&mut self, _clip_box: BoundingBox<f32>) {}
    fn pop_clip(&mut self) {}
    fn fill(&mut self, _brush: Brush<'_>) {}
    fn push_layer(&mut self, _mode: CompositeMode) {}
}

type Px = [f32; 4];

struct Painter<'a> {
    font: &'a FontRef<'a>,
    coords: &'a [NormalizedCoord],
    palette: Vec<Px>,
    d: Device,
    x0: i32,
    y0: i32,
    w: u32,
    h: u32,
    transforms: Vec<Transform>,
    clips: Vec<Vec<f32>>,
    layers: Vec<Vec<Px>>,
}

impl Painter<'_> {
    fn top(&self) -> Transform {
        *self.transforms.last().unwrap_or(&IDENTITY)
    }

    fn mask_of(&self, polys: &[Vec<Pt>]) -> Vec<f32> {
        let mut acc = Accumulator::new(self.w, self.h, self.x0, self.y0);
        acc.fill(polys, false);
        acc.coverage()
    }

    fn push_mask(&mut self, mut m: Vec<f32>) {
        if let Some(prev) = self.clips.last() {
            for (a, b) in m.iter_mut().zip(prev) {
                *a *= b;
            }
        }
        self.clips.push(m);
    }

    fn color(&self, palette_index: u16, alpha: f32) -> Px {
        let c = if palette_index == 0xFFFF {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            self.palette.get(palette_index as usize).copied().unwrap_or([0.0, 0.0, 0.0, 1.0])
        };
        let a = c[3] * alpha;
        [c[0] * a, c[1] * a, c[2] * a, a]
    }

    fn stop_color(&self, stops: &[ColorStop], t: f32, extend: Extend) -> Px {
        let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return [0.0; 4] };
        let (lo, hi) = (first.offset, last.offset);
        let span = hi - lo;
        let t = if span <= 0.0 {
            lo
        } else {
            match extend {
                Extend::Repeat => lo + (t - lo).rem_euclid(span),
                Extend::Reflect => {
                    let u = (t - lo).rem_euclid(2.0 * span);
                    lo + if u > span { 2.0 * span - u } else { u }
                }
                _ => t.clamp(lo, hi),
            }
        };
        if t <= lo {
            return self.color(first.palette_index, first.alpha);
        }
        for w in stops.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t <= b.offset {
                let f = if b.offset > a.offset { (t - a.offset) / (b.offset - a.offset) } else { 1.0 };
                let (ca, cb) = (self.color(a.palette_index, a.alpha), self.color(b.palette_index, b.alpha));
                return [
                    ca[0] + (cb[0] - ca[0]) * f,
                    ca[1] + (cb[1] - ca[1]) * f,
                    ca[2] + (cb[2] - ca[2]) * f,
                    ca[3] + (cb[3] - ca[3]) * f,
                ];
            }
        }
        self.color(last.palette_index, last.alpha)
    }
}

/// The parameter of the two-point conical gradient at `p`: the largest `t` with
/// `|p − c(t)| = r(t)` and `r(t) ≥ 0`, if any.
fn conical_t(p: (f32, f32), c0: (f32, f32), r0: f32, c1: (f32, f32), r1: f32) -> Option<f32> {
    let (cdx, cdy) = (c1.0 - c0.0, c1.1 - c0.1);
    let (pdx, pdy) = (p.0 - c0.0, p.1 - c0.1);
    let dr = r1 - r0;
    let a = cdx * cdx + cdy * cdy - dr * dr;
    let b = pdx * cdx + pdy * cdy + r0 * dr;
    let c = pdx * pdx + pdy * pdy - r0 * r0;
    if a.abs() < 1e-6 {
        if b.abs() < 1e-9 {
            return None;
        }
        let t = c / (2.0 * b);
        return (r0 + t * dr >= 0.0).then_some(t);
    }
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let (t1, t2) = ((b + sq) / a, (b - sq) / a);
    let (hi, lo) = if t1 > t2 { (t1, t2) } else { (t2, t1) };
    if r0 + hi * dr >= 0.0 {
        Some(hi)
    } else if r0 + lo * dr >= 0.0 {
        Some(lo)
    } else {
        None
    }
}

fn blend_channel(mode: CompositeMode, cs: f32, cb: f32) -> f32 {
    match mode {
        CompositeMode::Multiply => cs * cb,
        CompositeMode::Screen => cb + cs - cb * cs,
        CompositeMode::Overlay => blend_channel(CompositeMode::HardLight, cb, cs),
        CompositeMode::Darken => cs.min(cb),
        CompositeMode::Lighten => cs.max(cb),
        CompositeMode::ColorDodge => {
            if cb == 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        CompositeMode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        CompositeMode::HardLight => {
            if cs <= 0.5 {
                cb * 2.0 * cs
            } else {
                let s = 2.0 * cs - 1.0;
                cb + s - cb * s
            }
        }
        CompositeMode::SoftLight => {
            if cs <= 0.5 {
                cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
            } else {
                let d = if cb <= 0.25 { ((16.0 * cb - 12.0) * cb + 4.0) * cb } else { cb.sqrt() };
                cb + (2.0 * cs - 1.0) * (d - cb)
            }
        }
        CompositeMode::Difference => (cs - cb).abs(),
        CompositeMode::Exclusion => cs + cb - 2.0 * cs * cb,
        _ => cs,
    }
}

/// Composites premultiplied `s` onto `d` with `mode`.
fn composite(mode: CompositeMode, s: Px, d: Px) -> Px {
    let (sa, da) = (s[3], d[3]);
    let pd = |fs: f32, fd: f32| [s[0] * fs + d[0] * fd, s[1] * fs + d[1] * fd, s[2] * fs + d[2] * fd, sa * fs + da * fd];
    match mode {
        CompositeMode::Clear => [0.0; 4],
        CompositeMode::Src => s,
        CompositeMode::Dest => d,
        CompositeMode::DestOver => pd(1.0 - da, 1.0),
        CompositeMode::SrcIn => pd(da, 0.0),
        CompositeMode::DestIn => pd(0.0, sa),
        CompositeMode::SrcOut => pd(1.0 - da, 0.0),
        CompositeMode::DestOut => pd(0.0, 1.0 - sa),
        CompositeMode::SrcAtop => pd(da, 1.0 - sa),
        CompositeMode::DestAtop => pd(1.0 - da, sa),
        CompositeMode::Xor => pd(1.0 - da, 1.0 - sa),
        CompositeMode::Plus => {
            [(s[0] + d[0]).min(1.0), (s[1] + d[1]).min(1.0), (s[2] + d[2]).min(1.0), (sa + da).min(1.0)]
        }
        CompositeMode::Screen
        | CompositeMode::Overlay
        | CompositeMode::Darken
        | CompositeMode::Lighten
        | CompositeMode::ColorDodge
        | CompositeMode::ColorBurn
        | CompositeMode::HardLight
        | CompositeMode::SoftLight
        | CompositeMode::Difference
        | CompositeMode::Exclusion
        | CompositeMode::Multiply => {
            let mut out = [0.0; 4];
            for i in 0..3 {
                let cs = if sa > 0.0 { s[i] / sa } else { 0.0 };
                let cb = if da > 0.0 { d[i] / da } else { 0.0 };
                out[i] = (1.0 - da) * s[i] + (1.0 - sa) * d[i] + sa * da * blend_channel(mode, cs, cb);
            }
            out[3] = sa + da - sa * da;
            out
        }
        _ => pd(1.0, 1.0 - sa),
    }
}

impl ColorPainter for Painter<'_> {
    fn push_transform(&mut self, t: Transform) {
        let m = mul(&self.top(), &t);
        self.transforms.push(m);
    }
    fn pop_transform(&mut self) {
        self.transforms.pop();
    }
    fn push_clip_glyph(&mut self, gid: GlyphId) {
        let polys = coverage::flatten(&glyph_cmds(self.font, self.coords, gid, &self.top(), self.d));
        let m = self.mask_of(&polys);
        self.push_mask(m);
    }
    fn push_clip_box(&mut self, b: BoundingBox<f32>) {
        let t = self.top();
        let pts = [(b.x_min, b.y_min), (b.x_max, b.y_min), (b.x_max, b.y_max), (b.x_min, b.y_max)];
        let poly: Vec<Pt> = pts.iter().map(|(x, y)| self.d.apply(&t, *x, *y)).collect();
        let m = self.mask_of(&[poly]);
        self.push_mask(m);
    }
    fn pop_clip(&mut self) {
        self.clips.pop();
    }
    fn fill(&mut self, brush: Brush<'_>) {
        let Some(inv) = invert(&self.top()) else { return };
        let (w, h) = (self.w as usize, self.h as usize);
        let mut buf = std::mem::take(self.layers.last_mut().expect("base layer"));
        for j in 0..h {
            for i in 0..w {
                let ix = j * w + i;
                let cov = self.clips.last().map(|c| c[ix]).unwrap_or(1.0);
                if cov <= 0.0 {
                    continue;
                }
                // Pixel centre → font units of the current paint space.
                let px = self.x0 as f32 + i as f32 + 0.5;
                let py = self.y0 as f32 + j as f32 + 0.5;
                let (ux, uy) = ((px - self.d.dx) / self.d.s, -py / self.d.s);
                let (x, y) = (inv.xx * ux + inv.xy * uy + inv.dx, inv.yx * ux + inv.yy * uy + inv.dy);
                let c = match &brush {
                    Brush::Solid { palette_index, alpha } => self.color(*palette_index, *alpha),
                    Brush::LinearGradient { p0, p1, color_stops, extend } => {
                        let (vx, vy) = (p1.x - p0.x, p1.y - p0.y);
                        let len2 = vx * vx + vy * vy;
                        let t = if len2 > 0.0 { ((x - p0.x) * vx + (y - p0.y) * vy) / len2 } else { 0.0 };
                        self.stop_color(color_stops, t, *extend)
                    }
                    Brush::RadialGradient { c0, r0, c1, r1, color_stops, extend } => {
                        match conical_t((x, y), (c0.x, c0.y), *r0, (c1.x, c1.y), *r1) {
                            Some(t) => self.stop_color(color_stops, t, *extend),
                            None => [0.0; 4],
                        }
                    }
                    Brush::SweepGradient { c0, start_angle, end_angle, color_stops, extend } => {
                        let mut a = (y - c0.y).atan2(x - c0.x).to_degrees();
                        if a < 0.0 {
                            a += 360.0;
                        }
                        let span = end_angle - start_angle;
                        let t = if span.abs() > 1e-6 { (a - start_angle) / span } else { 0.0 };
                        self.stop_color(color_stops, t, *extend)
                    }
                };
                let s = [c[0] * cov, c[1] * cov, c[2] * cov, c[3] * cov];
                buf[ix] = composite(CompositeMode::SrcOver, s, buf[ix]);
            }
        }
        *self.layers.last_mut().expect("base layer") = buf;
    }
    fn push_layer(&mut self, _mode: CompositeMode) {
        self.layers.push(vec![[0.0; 4]; (self.w * self.h) as usize]);
    }
    fn pop_layer(&mut self) {
        self.pop_layer_with_mode(CompositeMode::SrcOver);
    }
    fn pop_layer_with_mode(&mut self, mode: CompositeMode) {
        if self.layers.len() < 2 {
            return;
        }
        let src = self.layers.pop().expect("checked");
        let dst = self.layers.last_mut().expect("checked");
        for (d, s) in dst.iter_mut().zip(src) {
            *d = composite(mode, s, *d);
        }
    }
}

fn to_rgba8(px: &[Px]) -> Vec<u8> {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    px.iter().flat_map(|p| [q(p[0]), q(p[1]), q(p[2]), q(p[3])]).collect()
}

fn paint_colr(
    key: &GlyphKey,
    font: &FontRef<'_>,
    coords: &[NormalizedCoord],
    glyph: &ColorGlyph<'_>,
    ppem: f32,
) -> Option<GlyphImage> {
    let upem = font.head().ok()?.units_per_em() as f32;
    let d = Device { s: ppem / upem.max(1.0), dx: key.x_bin.min(3) as f32 * 0.25 };
    let location = LocationRef::new(coords);
    let mut bp = BoundsPainter { font, coords, d, transforms: Vec::new(), bounds: None };
    glyph.paint(location, &mut bp).ok()?;
    let (x0, y0, x1, y1) = bp.bounds?;
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let palette = font
        .color_palettes()
        .get(0)
        .map(|p| {
            p.colors()
                .iter()
                .map(|c| [c.red as f32 / 255.0, c.green as f32 / 255.0, c.blue as f32 / 255.0, c.alpha as f32 / 255.0])
                .collect()
        })
        .unwrap_or_default();
    let mut painter = Painter {
        font,
        coords,
        palette,
        d,
        x0,
        y0,
        w,
        h,
        transforms: Vec::new(),
        clips: Vec::new(),
        layers: vec![vec![[0.0; 4]; (w * h) as usize]],
    };
    glyph.paint(location, &mut painter).ok()?;
    let base = painter.layers.into_iter().next()?;
    Some(GlyphImage { left: x0, top: y0, width: w, height: h, format: GlyphFormat::Color, data: to_rgba8(&base) })
}

/// Decodes a PNG to premultiplied RGBA8.
fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let n = (w * h) as usize;
    let mut out = Vec::with_capacity(n * 4);
    let px = &buf[..info.buffer_size()];
    for i in 0..n {
        let (r, g, b, a) = match info.color_type {
            png::ColorType::Rgba => (px[i * 4], px[i * 4 + 1], px[i * 4 + 2], px[i * 4 + 3]),
            png::ColorType::Rgb => (px[i * 3], px[i * 3 + 1], px[i * 3 + 2], 255),
            png::ColorType::GrayscaleAlpha => (px[i * 2], px[i * 2], px[i * 2], px[i * 2 + 1]),
            png::ColorType::Grayscale => (px[i], px[i], px[i], 255),
            png::ColorType::Indexed => return None,
        };
        let pm = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
        out.extend_from_slice(&[pm(r), pm(g), pm(b), a]);
    }
    Some((w, h, out))
}

/// Bilinear resampling of a `channels`-per-pixel image to `nw × nh` (pixel-centre aligned).
fn resample(src: &[u8], w: u32, h: u32, channels: usize, nw: u32, nh: u32) -> Vec<u8> {
    if (w, h) == (nw, nh) {
        return src.to_vec();
    }
    let mut out = vec![0u8; (nw * nh) as usize * channels];
    let (sx, sy) = (w as f32 / nw as f32, h as f32 / nh as f32);
    for y in 0..nh {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (h - 1) as f32);
        let (y0, ty) = (fy.floor() as u32, fy - fy.floor());
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..nw {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (w - 1) as f32);
            let (x0, tx) = (fx.floor() as u32, fx - fx.floor());
            let x1 = (x0 + 1).min(w - 1);
            for c in 0..channels {
                let at = |xx: u32, yy: u32| src[((yy * w + xx) as usize) * channels + c] as f32;
                let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * tx;
                let bot = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * tx;
                out[((y * nw + x) as usize) * channels + c] = (top + (bot - top) * ty + 0.5) as u8;
            }
        }
    }
    out
}

fn bitmap(key: &GlyphKey, font: &FontRef<'_>, ppem: f32) -> Option<GlyphImage> {
    let strikes = BitmapStrikes::new(font);
    if strikes.is_empty() {
        return None;
    }
    let g = strikes.glyph_for_size(Size::new(ppem), GlyphId::new(key.glyph))?;
    let (format, channels, w, h, data) = match &g.data {
        BitmapData::Png(bytes) => {
            let (w, h, d) = decode_png(bytes)?;
            (GlyphFormat::Color, 4, w, h, d)
        }
        BitmapData::Bgra(bytes) => {
            let n = (g.width * g.height) as usize;
            let src = bytes.get(..n * 4)?;
            let d = src.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            (GlyphFormat::Color, 4, g.width, g.height, d)
        }
        BitmapData::Mask(mask) => {
            let d = mask.decode(g.width, g.height).ok()?;
            (GlyphFormat::Mask, 1, g.width, g.height, d)
        }
    };
    if w == 0 || h == 0 {
        return None;
    }
    let s = ppem / g.ppem_x.max(1.0);
    let upem = font.head().ok()?.units_per_em() as f32;
    let outer = ppem / upem.max(1.0);
    let (nw, nh) = (((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1));
    let left = g.bearing_x * outer + g.inner_bearing_x * s + key.x_bin.min(3) as f32 * 0.25;
    let edge = g.bearing_y * outer + g.inner_bearing_y * s;
    let top = match g.placement_origin {
        Origin::TopLeft => -edge,
        Origin::BottomLeft => -(edge + nh as f32),
    };
    Some(GlyphImage {
        left: left.round() as i32,
        top: top.round() as i32,
        width: nw,
        height: nh,
        format,
        data: resample(&data, w, h, channels, nw, nh),
    })
}
