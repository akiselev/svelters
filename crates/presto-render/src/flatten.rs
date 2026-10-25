//! Scene flattening and batching (architecture §8.2): walks a [`FrameScene`]'s display lists, resolves
//! transforms and clips to device space, rasterises missing glyphs into the atlases, assigns each
//! primitive a draw order with the [`BoundsTree`], and groups the result into batches sorted by
//! (order, pipeline, texture), as GPUI's `BatchIterator` does.
//!
//! This module touches no GPU object, so batching is testable without an adapter.

use std::collections::HashMap;

use presto_base::color::{Color, Fill, GradientStop};
use presto_base::commit::{ImageData, ImageFormat};
use presto_base::display::{self, BorderStyle, LineCap, PathPaint, Prim, Sampling};
use presto_base::geom::{Affine, Corners, DeviceSize, Point, Rect, RoundedRect};
use presto_base::ids::{ImageId, TextureId};
use presto_base::props::ClipShape;
use presto_base::scene::{DrawItem, FrameScene, SceneResources};
use presto_base::text::{GlyphFormat, GlyphKey, GlyphRasterizer};

use crate::atlas::{Atlas, AtlasKey, AtlasKind, TileRef};
use crate::bounds_tree::BoundsTree;
use crate::gpu::{self, *};
use crate::paths::{MeshKey, PathCache};

/// Pipelines in GPUI's kind order (ties at one draw order resolve in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Shadow = 0,
    Quad = 1,
    Path = 2,
    Line = 3,
    Mask = 4,
    Image = 5,
}

/// The texture a batch binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tex {
    None,
    MaskPage(u32),
    ColorPage { page: u32, nearest: bool },
    Surface { id: TextureId, nearest: bool },
}

/// One draw call: `count` instances of `kind` starting at `first` in that kind's instance array.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Batch {
    pub kind: Kind,
    pub tex: Tex,
    pub first: u32,
    pub count: u32,
    /// Device bounds of the batch's primitives (for per-scissor culling).
    pub bounds: Rect,
}

/// A path to rasterise in a pre-pass: `mesh` drawn with `instance` (its `target` is set per batch).
#[derive(Clone, Copy, Debug)]
pub struct PathDraw {
    pub mesh: MeshKey,
    pub instance: PathInstance,
}

/// A path batch's intermediate: the paths `paths[first..first + count]` drawn into `rect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathTarget {
    pub first: u32,
    pub count: u32,
    /// Integer device rect (x, y, w, h), inside the frame's target.
    pub rect: [u32; 4],
}

#[derive(Default)]
pub struct Frame {
    pub quads: Vec<QuadInstance>,
    pub shadows: Vec<ShadowInstance>,
    pub lines: Vec<LineInstance>,
    pub masks: Vec<MaskInstance>,
    pub images: Vec<ImageInstance>,
    pub paths: Vec<PathDraw>,
    /// One per path batch, in batch order; a `Kind::Path` batch's `first` indexes this.
    pub path_targets: Vec<PathTarget>,
    pub path_sprites: Vec<PathSprite>,
    pub batches: Vec<Batch>,
    /// Primitives dropped: missing resources, unregistered textures, atlas overflow, canvas content.
    pub skipped: u32,
}

/// What the renderer remembers about a rasterised glyph (the atlas may evict its pixels).
#[derive(Clone, Copy, Debug)]
pub enum GlyphEntry {
    /// Empty or missing in the font: nothing to draw.
    Empty,
    Raster { format: GlyphFormat, left: i32, top: i32 },
}

/// Mutable state flattening needs from the renderer.
pub struct Ctx<'a> {
    pub mask: &'a mut Atlas,
    pub color: &'a mut Atlas,
    pub paths: &'a mut PathCache,
    pub glyph_index: &'a mut HashMap<GlyphKey, GlyphEntry>,
    pub glyphs: &'a dyn GlyphRasterizer,
    pub surfaces: &'a HashMap<TextureId, DeviceSize>,
    pub tree: &'a mut BoundsTree,
}

#[derive(Clone, Copy, Debug)]
struct ClipState {
    rect: Rect,
    rrect: Option<RoundedRect>,
}

impl ClipState {
    fn gpu(&self) -> Clip {
        let rect = [self.rect.min.x, self.rect.min.y, self.rect.max.x, self.rect.max.y];
        match self.rrect {
            Some(rr) => Clip { rect, rrect: xywh(&rr.rect), radii: corners(&rr.radii) },
            None => Clip { rect, rrect: [0.0, 0.0, -1.0, -1.0], radii: [0.0; 4] },
        }
    }
    /// Intersects with a device-space rounded rect (an exact rect when the radii are zero).
    fn push(&self, rr: RoundedRect) -> ClipState {
        let rect = self.rect.intersect(&rr.rect);
        // One rounded clip per instance: the innermost wins; an outer one keeps only its rect.
        let rrect = if rr.radii.is_zero() { self.rrect } else { Some(rr) };
        ClipState { rect, rrect }
    }
}

/// Resolved transform of one display list.
#[derive(Clone, Copy, Debug)]
struct Space {
    /// List-local points → device pixels.
    m: Affine,
    /// Device pixels per local unit (sqrt of |det m|).
    s: f32,
    /// Pixel-local (local × s) → device pixels.
    xform: Xform,
    /// `m` is a translation plus a uniform positive scale.
    uniform: bool,
}

impl Space {
    fn new(m: Affine) -> Space {
        let s = (m.a * m.d - m.b * m.c).abs().sqrt().max(1e-6);
        let t = m.then(&Affine::scale(1.0 / s, 1.0 / s));
        let uniform = m.b == 0.0 && m.c == 0.0 && m.a > 0.0 && (m.a - m.d).abs() <= 1e-5 * m.a;
        Space { m, s, xform: xform(&t), uniform }
    }
    fn px(&self, r: &Rect) -> [f32; 4] {
        [r.min.x * self.s, r.min.y * self.s, r.width() * self.s, r.height() * self.s]
    }
    /// A local rounded rect in device space: exact when `m` has no rotation or skew, else its bounds.
    fn device_rrect(&self, rr: &RoundedRect) -> RoundedRect {
        let rect = self.m.apply_rect(&rr.rect);
        if self.m.b == 0.0 && self.m.c == 0.0 {
            let k = (self.m.a.abs() + self.m.d.abs()) * 0.5;
            let r = rr.radii;
            RoundedRect { rect, radii: Corners { tl: r.tl * k, tr: r.tr * k, br: r.br * k, bl: r.bl * k } }
        } else {
            RoundedRect { rect, radii: Corners::ZERO }
        }
    }
}

fn xform(t: &Affine) -> Xform {
    Xform { m: [t.a, t.b, t.c, t.d], t: [t.e, t.f, 0.0, 0.0] }
}

fn xywh(r: &Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.width(), r.height()]
}

fn corners(c: &Corners) -> [f32; 4] {
    [c.tl, c.tr, c.br, c.bl]
}

fn scale_corners(c: &Corners, s: f32) -> [f32; 4] {
    [c.tl * s, c.tr * s, c.br * s, c.bl * s]
}

fn straight(c: Color, opacity: f32) -> [f32; 4] {
    [c.r, c.g, c.b, c.a * opacity]
}

/// A reference into one kind's instance array, before sorting.
#[derive(Clone, Copy, Debug)]
struct DrawRef {
    order: u32,
    kind: Kind,
    tex: Tex,
    index: u32,
    bounds: Rect,
}

#[derive(Default)]
struct Builder {
    frame: Frame,
    refs: Vec<DrawRef>,
}

impl Builder {
    fn push(&mut self, ctx: &mut Ctx<'_>, kind: Kind, tex: Tex, index: usize, bounds: Rect) {
        let order = ctx.tree.insert(bounds);
        self.refs.push(DrawRef { order, kind, tex, index: index as u32, bounds });
    }
}

/// Flattens `scene` into sorted batches. `cull` is the device area being redrawn (the damage union).
pub fn flatten(scene: &FrameScene<'_>, cull: Rect, ctx: &mut Ctx<'_>) -> Frame {
    ctx.tree.clear();
    let mut b = Builder::default();
    let target = Rect::new(0.0, 0.0, scene.target.w as f32, scene.target.h as f32);
    let cull = cull.intersect(&target);
    let scale = scene.scale.0;

    for item in scene.items {
        flatten_item(item, scale, cull, scene.resources, ctx, &mut b);
    }
    finish(b)
}

fn world_clip(clip: &ClipShape, scale: f32) -> RoundedRect {
    let to_dev = |r: &Rect| Rect::new(r.min.x * scale, r.min.y * scale, r.max.x * scale, r.max.y * scale);
    match clip {
        ClipShape::Rect(r) => RoundedRect { rect: to_dev(r), radii: Corners::ZERO },
        ClipShape::Rounded(rr) => {
            let c = rr.radii;
            RoundedRect {
                rect: to_dev(&rr.rect),
                radii: Corners { tl: c.tl * scale, tr: c.tr * scale, br: c.br * scale, bl: c.bl * scale },
            }
        }
    }
}

fn flatten_item(
    item: &DrawItem<'_>,
    scale: f32,
    cull: Rect,
    res: &dyn SceneResources,
    ctx: &mut Ctx<'_>,
    b: &mut Builder,
) {
    if item.opacity <= 0.0 || item.list.is_empty() {
        return;
    }
    let space = Space::new(Affine::scale(scale, scale).then(&item.transform));
    let mut clip = ClipState { rect: cull, rrect: None };
    if let Some(c) = &item.clip {
        clip = clip.push(world_clip(c, scale));
    }
    if !item.list.bounds.is_empty() && !space.m.apply_rect(&item.list.bounds).intersects(&clip.rect) {
        return;
    }
    let mut stack: Vec<ClipState> = Vec::new();
    for prim in &item.list.items {
        match prim {
            Prim::PushClip(rr) => {
                stack.push(clip);
                clip = clip.push(space.device_rrect(rr));
            }
            Prim::PopClip => {
                if let Some(c) = stack.pop() {
                    clip = c;
                }
            }
            _ if clip.rect.is_empty() => {}
            Prim::Quad(q) => quad(q, &space, &clip, item.opacity, ctx, b),
            Prim::Shadow(s) => shadow(s, &space, &clip, item.opacity, ctx, b),
            Prim::Line(l) => line(l, &space, &clip, item.opacity, ctx, b),
            Prim::Glyphs(g) => glyphs(g, &space, &clip, item.opacity, res, ctx, b),
            Prim::Icon(i) => icon(i, &space, &clip, item.opacity, res, ctx, b),
            Prim::Image(i) => image(i, &space, &clip, item.opacity, res, ctx, b),
            Prim::Path(p) => path(p, &space, &clip, item.opacity, res, ctx, b),
            Prim::Surface(s) => surface(s, &space, &clip, item.opacity, ctx, b),
            // Canvas pipelines (architecture §9.5) are not part of this renderer yet.
            Prim::Canvas(_) => b.frame.skipped += 1,
        }
    }
}

/// Device bounds clipped to `clip`, or `None` when nothing is visible.
fn visible(bounds: Rect, clip: &ClipState) -> Option<Rect> {
    let r = bounds.intersect(&clip.rect);
    (!r.is_empty()).then_some(r)
}

fn quad(q: &display::Quad, sp: &Space, clip: &ClipState, opacity: f32, ctx: &mut Ctx<'_>, b: &mut Builder) {
    let has_border = q.border_style_visible();
    let fill_visible = match &q.fill {
        Fill::Solid(c) => c.a > 0.0,
        Fill::Linear { stops, .. } => stops.iter().any(|s| s.color.a > 0.0),
    };
    if !fill_visible && !has_border {
        return;
    }
    let Some(dev) = visible(sp.m.apply_rect(&q.rect), clip) else { return };
    let s = sp.s;
    let mut inst = QuadInstance {
        xform: sp.xform,
        clip: clip.gpu(),
        bounds: sp.px(&q.rect),
        radii: scale_corners(&q.radii, s),
        borders: [q.border_widths.top * s, q.border_widths.right * s, q.border_widths.bottom * s, q.border_widths.left * s],
        border_color: straight(q.border_color, opacity),
        info: [
            FILL_SOLID,
            match q.border_style {
                BorderStyle::Solid => 0,
                BorderStyle::Dashed => 1,
                BorderStyle::Dotted => 2,
            },
            0,
            0,
        ],
        ..Default::default()
    };
    match &q.fill {
        Fill::Solid(c) => inst.fill = straight(*c, opacity),
        Fill::Linear { angle_deg, stops } => {
            let stops = resample_stops(stops);
            inst.info[0] = FILL_LINEAR;
            inst.info[2] = stops.len() as u32;
            inst.params[0] = angle_deg.to_radians();
            for (i, st) in stops.iter().enumerate() {
                inst.stop_colors[i] = gpu::pack_rgba8(straight(st.color, opacity));
                inst.stop_offsets[i] = st.offset;
            }
        }
    }
    let idx = b.frame.quads.len();
    b.frame.quads.push(inst);
    b.push(ctx, Kind::Quad, Tex::None, idx, dev);
}

trait QuadExt {
    fn border_style_visible(&self) -> bool;
}

impl QuadExt for display::Quad {
    fn border_style_visible(&self) -> bool {
        let w = self.border_widths;
        self.border_color.a > 0.0 && (w.top > 0.0 || w.right > 0.0 || w.bottom > 0.0 || w.left > 0.0)
    }
}

/// At most [`gpu::MAX_STOPS`] stops: more are resampled evenly between the first and last offset.
fn resample_stops(stops: &[GradientStop]) -> smallvec::SmallVec<[GradientStop; MAX_STOPS]> {
    if stops.len() <= MAX_STOPS {
        return stops.iter().copied().collect();
    }
    let (lo, hi) = (stops[0].offset, stops[stops.len() - 1].offset);
    (0..MAX_STOPS)
        .map(|i| {
            let t = lo + (hi - lo) * i as f32 / (MAX_STOPS - 1) as f32;
            GradientStop { offset: t, color: gradient_at(stops, t) }
        })
        .collect()
}

/// The colour of a CSS gradient at `t`, interpolated in premultiplied sRGB-encoded space.
fn gradient_at(stops: &[GradientStop], t: f32) -> Color {
    let first = stops[0];
    if t <= first.offset {
        return first.color;
    }
    for w in stops.windows(2) {
        let (a, c) = (w[0], w[1]);
        if t <= c.offset {
            let f = if c.offset > a.offset { (t - a.offset) / (c.offset - a.offset) } else { 1.0 };
            let (pa, pc) = (a.color.premultiplied(), c.color.premultiplied());
            let p: Vec<f32> = (0..4).map(|i| pa[i] + (pc[i] - pa[i]) * f).collect();
            let alpha = p[3];
            return if alpha > 0.0 {
                Color::rgba(p[0] / alpha, p[1] / alpha, p[2] / alpha, alpha)
            } else {
                Color::TRANSPARENT
            };
        }
    }
    stops[stops.len() - 1].color
}

fn shadow(s: &display::Shadow, sp: &Space, clip: &ClipState, opacity: f32, ctx: &mut Ctx<'_>, b: &mut Builder) {
    if s.color.a <= 0.0 {
        return;
    }
    // CSS: the blur radius is twice the Gaussian's standard deviation.
    let sigma = (s.blur * 0.5).max(0.0);
    let grow = |c: &Corners, d: f32| Corners {
        tl: (c.tl + d).max(0.0),
        tr: (c.tr + d).max(0.0),
        br: (c.br + d).max(0.0),
        bl: (c.bl + d).max(0.0),
    };
    let (shape, radii, geometry) = if s.inset {
        let hole = s.rect.translate(s.offset).inflate(-s.spread);
        (hole, grow(&s.radii, -s.spread), s.rect)
    } else {
        let r = s.rect.translate(s.offset).inflate(s.spread);
        (r, grow(&s.radii, s.spread), r.inflate(3.0 * sigma))
    };
    let Some(dev) = visible(sp.m.apply_rect(&geometry), clip) else { return };
    let k = sp.s;
    let inst = ShadowInstance {
        xform: sp.xform,
        clip: clip.gpu(),
        bounds: sp.px(&shape),
        radii: scale_corners(&radii, k),
        color: straight(s.color, opacity),
        element: sp.px(&s.rect),
        element_radii: scale_corners(&s.radii, k),
        params: [sigma * k, if s.inset { 1.0 } else { 0.0 }, 0.0, 0.0],
    };
    let idx = b.frame.shadows.len();
    b.frame.shadows.push(inst);
    b.push(ctx, Kind::Shadow, Tex::None, idx, dev);
}

/// Wavy lines: amplitude as a multiple of the line width (the shader's `WAVE_AMPLITUDE`).
pub const WAVE_AMPLITUDE: f32 = 1.0;

fn line(l: &display::Line, sp: &Space, clip: &ClipState, opacity: f32, ctx: &mut Ctx<'_>, b: &mut Builder) {
    let d = l.to - l.from;
    let len = d.length();
    if l.color.a <= 0.0 || l.width <= 0.0 {
        return;
    }
    let k = sp.s;
    let (len_px, hw) = (len * k, l.width * k * 0.5);
    let cap_ext = match l.cap {
        LineCap::Butt => 0.0,
        LineCap::Round | LineCap::Square => hw,
    };
    let amp = if l.wavy { WAVE_AMPLITUDE * 2.0 * hw } else { 0.0 };
    let half_h = hw + amp + 1.0;
    let quad_px = Rect::new(-cap_ext - 1.0, -half_h, len_px + cap_ext + 1.0, half_h);
    // Line space (x along the segment, pixel units) → local → device.
    let angle = d.y.atan2(d.x);
    let to_local = Affine::translate(l.from.x, l.from.y)
        .then(&Affine::rotate(angle))
        .then(&Affine::scale(1.0 / k, 1.0 / k));
    let t = sp.m.then(&to_local);
    let Some(dev) = visible(t.apply_rect(&quad_px), clip) else { return };
    let (on, off) = l.dash.map(|(a, b)| (a * k, b * k)).unwrap_or((0.0, 0.0));
    let inst = LineInstance {
        xform: xform(&t),
        clip: clip.gpu(),
        bounds: xywh(&quad_px),
        color: straight(l.color, opacity),
        geom: [
            len_px,
            hw,
            match l.cap {
                LineCap::Butt => 0.0,
                LineCap::Round => 1.0,
                LineCap::Square => 2.0,
            },
            if l.wavy { 1.0 } else { 0.0 },
        ],
        dash: [on, off, 0.0, 0.0],
    };
    let idx = b.frame.lines.len();
    b.frame.lines.push(inst);
    b.push(ctx, Kind::Line, Tex::None, idx, dev);
}

/// GPUI's glyph quantisation: 4 horizontal subpixel positions, whole-pixel baselines.
pub const SUBPIXEL_VARIANTS_X: f32 = 4.0;

fn glyphs(
    g: &display::Glyphs,
    sp: &Space,
    clip: &ClipState,
    opacity: f32,
    res: &dyn SceneResources,
    ctx: &mut Ctx<'_>,
    b: &mut Builder,
) {
    let Some(run) = res.run(g.run) else {
        b.frame.skipped += 1;
        return;
    };
    if g.color.a <= 0.0 {
        return;
    }
    let k = sp.s;
    let ppem_26_6 = (run.size * k * 64.0).round().max(0.0) as u32;
    for glyph in &run.glyphs {
        let pen = Point::new(g.origin.x + glyph.x, g.origin.y + glyph.y);
        // Device-space placement for axis-aligned text; pixel-local placement under rotation or skew.
        let (pen_px, x_bin, xf) = if sp.uniform {
            let p = sp.m.apply(pen);
            let q = (p.x * SUBPIXEL_VARIANTS_X).round() / SUBPIXEL_VARIANTS_X;
            let ix = q.floor();
            let bin = ((q - ix) * SUBPIXEL_VARIANTS_X).round() as u8 % SUBPIXEL_VARIANTS_X as u8;
            (Point::new(ix, p.y.round()), bin, xform(&Affine::IDENTITY))
        } else {
            (Point::new(pen.x * k, pen.y * k), 0, sp.xform)
        };
        let key = GlyphKey {
            font: run.font,
            variations: run.variations,
            glyph: glyph.id,
            ppem_26_6,
            x_bin,
            hint: run.hint,
            aa: run.aa,
            dilation: 0,
            embolden_64: (run.embolden * k * 64.0).round().clamp(0.0, u16::MAX as f32) as u16,
            skew_deg: run.skew_deg.round().clamp(-90.0, 90.0) as i8,
        };
        let Some((format, left, top, tile)) = glyph_tile(&key, ctx) else { continue };
        let px = Rect::new(
            pen_px.x + left as f32,
            pen_px.y + top as f32,
            pen_px.x + (left + tile.w as i32) as f32,
            pen_px.y + (top + tile.h as i32) as f32,
        );
        let t = Affine { a: xf.m[0], b: xf.m[1], c: xf.m[2], d: xf.m[3], e: xf.t[0], f: xf.t[1] };
        let Some(dev) = visible(t.apply_rect(&px), clip) else { continue };
        let uv = [tile.x as f32, tile.y as f32, tile.w as f32, tile.h as f32];
        match format {
            GlyphFormat::Mask => {
                let idx = b.frame.masks.len();
                b.frame.masks.push(MaskInstance {
                    xform: xf,
                    clip: clip.gpu(),
                    bounds: xywh(&px),
                    uv,
                    color: straight(g.color, opacity),
                    params: [1.0, 0.0, 0.0, 0.0],
                });
                b.push(ctx, Kind::Mask, Tex::MaskPage(tile.page), idx, dev);
            }
            GlyphFormat::Color => {
                let idx = b.frame.images.len();
                b.frame.images.push(ImageInstance {
                    xform: xf,
                    clip: clip.gpu(),
                    bounds: xywh(&px),
                    uv,
                    uv_clamp: tile_clamp(&tile),
                    radii: [0.0; 4],
                    params: [opacity, 0.0, 0.0, 0.0],
                });
                b.push(ctx, Kind::Image, Tex::ColorPage { page: tile.page, nearest: false }, idx, dev);
            }
        }
    }
}

/// The glyph's atlas tile, rasterising it on a miss.
fn glyph_tile(key: &GlyphKey, ctx: &mut Ctx<'_>) -> Option<(GlyphFormat, i32, i32, TileRef)> {
    let akey = AtlasKey::Glyph(*key);
    if let Some(GlyphEntry::Raster { format, left, top }) = ctx.glyph_index.get(key).copied() {
        let atlas = match format {
            GlyphFormat::Mask => &mut *ctx.mask,
            GlyphFormat::Color => &mut *ctx.color,
        };
        if let Some(t) = atlas.get(&akey) {
            return Some((format, left, top, t));
        }
    } else if ctx.glyph_index.contains_key(key) {
        return None;
    }
    let Some(img) = ctx.glyphs.rasterize(key).filter(|i| i.width > 0 && i.height > 0) else {
        ctx.glyph_index.insert(*key, GlyphEntry::Empty);
        return None;
    };
    let atlas = match img.format {
        GlyphFormat::Mask => &mut *ctx.mask,
        GlyphFormat::Color => &mut *ctx.color,
    };
    let tile = atlas.insert(akey, img.width, img.height, &img.data, 1)?;
    ctx.glyph_index.insert(*key, GlyphEntry::Raster { format: img.format, left: img.left, top: img.top });
    Some((img.format, img.left, img.top, tile))
}

/// The texel rect linear filtering may read: the tile shrunk by half a texel.
fn tile_clamp(t: &TileRef) -> [f32; 4] {
    [t.x as f32 + 0.5, t.y as f32 + 0.5, (t.x + t.w) as f32 - 0.5, (t.y + t.h) as f32 - 0.5]
}

/// The image's pixels as coverage (A8 as is; RGBA by its alpha) or as premultiplied RGBA (A8 as white).
fn image_pixels(img: &ImageData, kind: AtlasKind) -> Option<Vec<u8>> {
    let n = (img.size.w * img.size.h) as usize;
    match (img.format, kind) {
        (ImageFormat::A8, AtlasKind::Mask) | (ImageFormat::Rgba8Premul, AtlasKind::Color) => {
            (img.pixels.len() == n * kind.bytes_per_pixel()).then(|| img.pixels.clone())
        }
        (ImageFormat::Rgba8Premul, AtlasKind::Mask) => {
            (img.pixels.len() == n * 4).then(|| img.pixels.chunks_exact(4).map(|p| p[3]).collect())
        }
        (ImageFormat::A8, AtlasKind::Color) => {
            (img.pixels.len() == n).then(|| img.pixels.iter().flat_map(|&a| [a, a, a, a]).collect())
        }
    }
}

fn image_tile(id: ImageId, kind: AtlasKind, res: &dyn SceneResources, ctx: &mut Ctx<'_>) -> Option<TileRef> {
    let key = AtlasKey::Image { id, mask: kind == AtlasKind::Mask };
    let atlas = match kind {
        AtlasKind::Mask => &mut *ctx.mask,
        AtlasKind::Color => &mut *ctx.color,
    };
    if let Some(t) = atlas.get(&key) {
        return Some(t);
    }
    let img = res.image(id)?;
    let pixels = image_pixels(img, kind)?;
    atlas.insert(key, img.size.w, img.size.h, &pixels, 1)
}

fn icon(
    i: &display::Icon,
    sp: &Space,
    clip: &ClipState,
    opacity: f32,
    res: &dyn SceneResources,
    ctx: &mut Ctx<'_>,
    b: &mut Builder,
) {
    let Some(dev) = visible(sp.m.apply_rect(&i.rect), clip) else { return };
    let Some(img) = res.image(i.image) else {
        b.frame.skipped += 1;
        return;
    };
    // A tinted icon, or an uncoloured coverage icon (drawn black), goes through the mask pipeline.
    let tint = match (i.color, img.format) {
        (Some(c), _) => Some(c),
        (None, ImageFormat::A8) => Some(Color::BLACK),
        (None, ImageFormat::Rgba8Premul) => None,
    };
    let kind = if tint.is_some() { AtlasKind::Mask } else { AtlasKind::Color };
    let Some(tile) = image_tile(i.image, kind, res, ctx) else {
        b.frame.skipped += 1;
        return;
    };
    let uv = [tile.x as f32, tile.y as f32, tile.w as f32, tile.h as f32];
    match tint {
        Some(c) => {
            let idx = b.frame.masks.len();
            b.frame.masks.push(MaskInstance {
                xform: sp.xform,
                clip: clip.gpu(),
                bounds: sp.px(&i.rect),
                uv,
                color: straight(c, opacity),
                params: [0.0; 4],
            });
            b.push(ctx, Kind::Mask, Tex::MaskPage(tile.page), idx, dev);
        }
        None => {
            let idx = b.frame.images.len();
            b.frame.images.push(ImageInstance {
                xform: sp.xform,
                clip: clip.gpu(),
                bounds: sp.px(&i.rect),
                uv,
                uv_clamp: tile_clamp(&tile),
                radii: [0.0; 4],
                params: [opacity, 0.0, 0.0, 0.0],
            });
            b.push(ctx, Kind::Image, Tex::ColorPage { page: tile.page, nearest: false }, idx, dev);
        }
    }
}

fn image(
    i: &display::Image,
    sp: &Space,
    clip: &ClipState,
    opacity: f32,
    res: &dyn SceneResources,
    ctx: &mut Ctx<'_>,
    b: &mut Builder,
) {
    let opacity = opacity * i.opacity;
    if opacity <= 0.0 {
        return;
    }
    let Some(dev) = visible(sp.m.apply_rect(&i.rect), clip) else { return };
    let Some(tile) = image_tile(i.image, AtlasKind::Color, res, ctx) else {
        b.frame.skipped += 1;
        return;
    };
    let src = i.src.unwrap_or(Rect::new(0.0, 0.0, tile.w as f32, tile.h as f32));
    let uv = [tile.x as f32 + src.min.x, tile.y as f32 + src.min.y, src.width(), src.height()];
    let nearest = i.sampling == Sampling::Nearest;
    let idx = b.frame.images.len();
    b.frame.images.push(ImageInstance {
        xform: sp.xform,
        clip: clip.gpu(),
        bounds: sp.px(&i.rect),
        uv,
        uv_clamp: tile_clamp(&tile),
        radii: scale_corners(&i.radii, sp.s),
        params: [opacity, 0.0, 0.0, 0.0],
    });
    b.push(ctx, Kind::Image, Tex::ColorPage { page: tile.page, nearest }, idx, dev);
}

fn surface(s: &display::SurfacePrim, sp: &Space, clip: &ClipState, opacity: f32, ctx: &mut Ctx<'_>, b: &mut Builder) {
    let opacity = opacity * s.opacity;
    if opacity <= 0.0 {
        return;
    }
    let Some(size) = ctx.surfaces.get(&s.texture).copied() else {
        b.frame.skipped += 1;
        return;
    };
    let Some(dev) = visible(sp.m.apply_rect(&s.rect), clip) else { return };
    let (w, h) = (size.w as f32, size.h as f32);
    let idx = b.frame.images.len();
    b.frame.images.push(ImageInstance {
        xform: sp.xform,
        clip: clip.gpu(),
        bounds: sp.px(&s.rect),
        uv: [0.0, 0.0, w, h],
        uv_clamp: [0.5, 0.5, w - 0.5, h - 0.5],
        radii: [0.0; 4],
        params: [opacity, 0.0, 0.0, 0.0],
    });
    b.push(ctx, Kind::Image, Tex::Surface { id: s.texture, nearest: false }, idx, dev);
}

fn path(
    p: &display::PathPrim,
    sp: &Space,
    clip: &ClipState,
    opacity: f32,
    res: &dyn SceneResources,
    ctx: &mut Ctx<'_>,
    b: &mut Builder,
) {
    let Some(data) = res.path(p.path) else {
        b.frame.skipped += 1;
        return;
    };
    let (color, stroke) = match p.paint {
        PathPaint::Fill(c) => (c, None),
        PathPaint::Stroke { color, width } => (color, Some(width)),
    };
    if color.a <= 0.0 {
        return;
    }
    let pm = sp.m.then(&p.transform);
    let ppu = (pm.a * pm.d - pm.b * pm.c).abs().sqrt();
    let key = MeshKey::new(p.path, ppu, stroke);
    let Some(mesh) = ctx.paths.get(key, data) else { return };
    let Some(dev) = visible(pm.apply_rect(&mesh.bounds), clip) else { return };
    // Whole pixels: the intermediate is texel-aligned with the target.
    let dev = Rect::new(dev.min.x.floor(), dev.min.y.floor(), dev.max.x.ceil(), dev.max.y.ceil());
    let idx = b.frame.paths.len();
    b.frame.paths.push(PathDraw {
        mesh: key,
        instance: PathInstance { xform: xform(&pm), clip: clip.gpu(), color: straight(color, opacity), viewport_rect: [0.0; 4] },
    });
    b.push(ctx, Kind::Path, Tex::None, idx, dev);
}

/// Sorts by (order, kind, texture), reorders each kind's instances to match, and merges runs.
fn finish(mut b: Builder) -> Frame {
    b.refs.sort_by_key(|r| (r.order, r.kind, r.tex));
    let mut f = std::mem::take(&mut b.frame);
    let quads = std::mem::take(&mut f.quads);
    let shadows = std::mem::take(&mut f.shadows);
    let lines = std::mem::take(&mut f.lines);
    let masks = std::mem::take(&mut f.masks);
    let images = std::mem::take(&mut f.images);
    let paths = std::mem::take(&mut f.paths);

    let mut current: Option<Batch> = None;
    for r in &b.refs {
        let i = r.index as usize;
        let first = match r.kind {
            Kind::Quad => push_len(&mut f.quads, quads[i]),
            Kind::Shadow => push_len(&mut f.shadows, shadows[i]),
            Kind::Line => push_len(&mut f.lines, lines[i]),
            Kind::Mask => push_len(&mut f.masks, masks[i]),
            Kind::Image => push_len(&mut f.images, images[i]),
            Kind::Path => push_len(&mut f.paths, paths[i]),
        };
        match &mut current {
            Some(batch) if batch.kind == r.kind && batch.tex == r.tex => {
                batch.count += 1;
                batch.bounds = batch.bounds.union(&r.bounds);
            }
            _ => {
                if let Some(batch) = current.take() {
                    close(&mut f, batch);
                }
                current = Some(Batch { kind: r.kind, tex: r.tex, first, count: 1, bounds: r.bounds });
            }
        }
    }
    if let Some(batch) = current {
        close(&mut f, batch);
    }
    f
}

fn push_len<T>(v: &mut Vec<T>, x: T) -> u32 {
    v.push(x);
    (v.len() - 1) as u32
}

/// Appends `batch`; a path batch becomes one intermediate target and one composite sprite.
fn close(f: &mut Frame, mut batch: Batch) {
    if batch.kind == Kind::Path {
        let r = batch.bounds;
        let rect = [r.min.x as u32, r.min.y as u32, r.width() as u32, r.height() as u32];
        let target = [rect[0] as f32, rect[1] as f32, rect[2] as f32, rect[3] as f32];
        for p in &mut f.paths[batch.first as usize..(batch.first + batch.count) as usize] {
            p.instance.viewport_rect = target;
        }
        f.path_targets.push(PathTarget { first: batch.first, count: batch.count, rect });
        f.path_sprites.push(PathSprite { bounds: target });
        batch.first = (f.path_targets.len() - 1) as u32;
        batch.count = 1;
    }
    f.batches.push(batch);
}
