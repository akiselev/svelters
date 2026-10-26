//! Vector icons and SVG images on the GPU (`plans/milestones/SKELTER-svg.md`).
//!
//! An SVG is parsed once (usvg) into a [`VectorArt`]: its layers in paint order, each a path in its
//! own coordinates with a transform into the drawing, an opacity and a paint source (a literal
//! colour, `currentColor`, or one of the icon kit's slots). Strokes are outlined on the CPU when the
//! SVG is parsed (tiny-skia's stroker: caps, joins, miter limits and dashes as resvg draws them), so
//! every layer is a fill. A layer's path enters the commit through
//! [`Resources::path`](crate::shim::core::Resources::path), keyed by the SVG's content, so every
//! node that draws the same icon draws the same `PathId` (skelter-core shares it, and the renderer
//! tessellates it once per 9% scale step); it is drawn as a [`Prim::Path`] through the lyon/MSAA path
//! pipeline. Zooming or a new scale factor redraws the paths from the renderer's mesh cache: nothing
//! is rasterised and nothing enters the atlas.
//!
//! Colours are live: each layer's primitive carries the [`PaletteSlot`] its colour comes from
//! (0 = `currentColor`, 1–3 = `--icon-subject`, `--icon-accent`, `--icon-badge`), so a palette
//! animation on the compositor recolours it without a UI phase.
//!
//! Small sizes stay raster: at or below [`VECTOR_THRESHOLD_PX`] physical pixels the registry draws
//! resvg's pixel-snapped masters. With slots set, a vectorisable icon's raster is
//! [`slot_masks`]: one `A8` coverage mask per colour (a few masks instead of one RGBA raster), each
//! tinted at draw time, so the slots stay live at every size.
//!
//! What the vector path cannot draw yet falls back to the raster path, per icon:
//! filters, masks, clip paths, blend modes, patterns, embedded images, text, a group opacity over
//! several shapes, and gradients (linear and radial: a gradient `PathPaint` needs per-pixel colour in
//! the path shader, more than this lane's small change; see the record). [`VectorArt::parse`] says
//! why, and the registry reports it once per icon (`IconError::Fallback`).

use super::super::PaintCx;
use crate::paint::icon::registry::slot_svg;
use resvg::tiny_skia;
use resvg::usvg;
use skelter_base::color::Color;
use skelter_base::commit::{FillRule, ImageData, ImageFormat, PathCmd, PathData};
use skelter_base::display::{PaletteSlot, PathPaint, PathPrim, Prim};
use skelter_base::geom::{Affine, DeviceSize, Rect, Size};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock, Mutex};

/// The physical size (pixels) at or below which an icon is drawn from its raster masters; above it,
/// as vectors. Chosen from side-by-side screenshots at 1×, 1.25×, 1.5× and 2× (the record): up to
/// 24 pixels resvg's analytic coverage of a pixel-snapped master is visibly crisper than 4× MSAA.
pub const VECTOR_THRESHOLD_PX: u32 = 24;

/// One of the icon kit's colour slots (ICON-STYLE §6), by its class in the SVG.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconSlot {
    Subject,
    Accent,
    Badge,
}

impl IconSlot {
    pub const ALL: [IconSlot; 3] = [IconSlot::Subject, IconSlot::Accent, IconSlot::Badge];

    /// Its palette index (0 is `currentColor`).
    pub fn index(self) -> u8 {
        match self {
            IconSlot::Subject => 1,
            IconSlot::Accent => 2,
            IconSlot::Badge => 3,
        }
    }

    /// Its class in the SVG.
    pub fn class(self) -> &'static str {
        match self {
            IconSlot::Subject => "subject",
            IconSlot::Accent => "accent",
            IconSlot::Badge => "badge",
        }
    }
}

/// Where a layer's colour comes from.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaintSource {
    /// A colour written in the SVG.
    Color(Color),
    /// `currentColor` (palette slot 0).
    CurrentColor,
    /// `currentColor` on an element of a slot's class: the slot's colour when it is set, else
    /// `currentColor`.
    Slot(IconSlot),
}

/// Whether a layer was a fill or a stroke in the SVG (a stroke is drawn as its outline's fill).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Fill,
    Stroke,
}

/// One layer of a [`VectorArt`].
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct VectorLayer {
    /// The outline, in the layer's own coordinates (a stroke already outlined).
    pub path: Arc<PathData>,
    /// The layer's coordinates → the drawing's (the SVG's size, viewBox applied).
    pub transform: Affine,
    pub kind: LayerKind,
    /// The fill or stroke opacity times its groups' opacities.
    pub opacity: f32,
    pub source: PaintSource,
    /// The layer's bounds in the drawing's coordinates.
    pub bounds: Rect,
    ts: tiny_skia::Path,
    ts_rule: tiny_skia::FillRule,
    ts_transform: tiny_skia::Transform,
}

/// An SVG as GPU-drawable geometry: its size in its own units (points) and its layers in paint
/// order.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct VectorArt {
    pub size: Size,
    pub layers: Vec<VectorLayer>,
    /// A hash of the SVG's bytes: the commit keys of its paths.
    pub content: u64,
}

/// The sentinel `color`s the parse style sheet gives the root and each slot class, so a resolved
/// paint says where it came from.
const SENTINELS: [(u8, u8, u8); 4] = [(1, 2, 3), (1, 2, 4), (1, 2, 5), (1, 2, 6)];

fn sentinel_sheet() -> String {
    let rgb = |(r, g, b): (u8, u8, u8)| format!("rgb({r},{g},{b})");
    let mut s = format!("svg{{color:{}}}", rgb(SENTINELS[0]));
    for slot in IconSlot::ALL {
        s.push_str(&format!(".{}{{color:{}}}", slot.class(), rgb(SENTINELS[slot.index() as usize])));
    }
    s
}

fn source_of(paint: &usvg::Paint) -> Result<PaintSource, String> {
    match paint {
        usvg::Paint::Color(c) => {
            let rgb = (c.red, c.green, c.blue);
            Ok(match SENTINELS.iter().position(|s| *s == rgb) {
                Some(0) => PaintSource::CurrentColor,
                Some(i) => PaintSource::Slot(IconSlot::ALL[i - 1]),
                None => PaintSource::Color(Color::from_rgba8(c.red, c.green, c.blue, 255)),
            })
        }
        usvg::Paint::LinearGradient(_) => Err("linear gradient".into()),
        usvg::Paint::RadialGradient(_) => Err("radial gradient".into()),
        usvg::Paint::Pattern(_) => Err("pattern".into()),
    }
}

fn affine(t: tiny_skia::Transform) -> Affine {
    Affine { a: t.sx, b: t.ky, c: t.kx, d: t.sy, e: t.tx, f: t.ty }
}

fn path_data(p: &tiny_skia::Path, rule: FillRule) -> PathData {
    let cmds = p
        .segments()
        .map(|s| match s {
            tiny_skia::PathSegment::MoveTo(a) => PathCmd::MoveTo(a.x, a.y),
            tiny_skia::PathSegment::LineTo(a) => PathCmd::LineTo(a.x, a.y),
            tiny_skia::PathSegment::QuadTo(c, a) => PathCmd::QuadTo(c.x, c.y, a.x, a.y),
            tiny_skia::PathSegment::CubicTo(c1, c2, a) => PathCmd::CubicTo(c1.x, c1.y, c2.x, c2.y, a.x, a.y),
            tiny_skia::PathSegment::Close => PathCmd::Close,
        })
        .collect();
    PathData { cmds, fill_rule: rule }
}

/// Shapes under a group (paths, images, text), to tell whether its opacity may go to its one shape.
fn shapes(g: &usvg::Group) -> usize {
    g.children()
        .iter()
        .map(|n| match n {
            usvg::Node::Group(g) => shapes(g),
            usvg::Node::Path(p) => usize::from(p.is_visible()) * (usize::from(p.fill().is_some()) + usize::from(p.stroke().is_some())),
            _ => 1,
        })
        .sum()
}

fn walk(g: &usvg::Group, opacity: f32, out: &mut Vec<VectorLayer>) -> Result<(), String> {
    for n in g.children() {
        match n {
            usvg::Node::Group(c) => {
                if !c.filters().is_empty() {
                    return Err("filter".into());
                }
                if c.mask().is_some() {
                    return Err("mask".into());
                }
                if c.clip_path().is_some() {
                    return Err("clip path".into());
                }
                if c.blend_mode() != usvg::BlendMode::Normal {
                    return Err("blend mode".into());
                }
                let o = c.opacity().get();
                if o < 1.0 && shapes(c) > 1 {
                    return Err("group opacity over several shapes".into());
                }
                walk(c, opacity * o, out)?;
            }
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let fill_first = p.paint_order() == usvg::PaintOrder::FillAndStroke;
                let mut fill = None;
                let mut stroke = None;
                if let Some(f) = p.fill() {
                    let rule = match f.rule() {
                        usvg::FillRule::NonZero => (FillRule::NonZero, tiny_skia::FillRule::Winding),
                        usvg::FillRule::EvenOdd => (FillRule::EvenOdd, tiny_skia::FillRule::EvenOdd),
                    };
                    fill = Some(layer(p.data().clone(), rule, p.abs_transform(), LayerKind::Fill, opacity * f.opacity().get(), source_of(f.paint())?));
                }
                if let Some(s) = p.stroke() {
                    let t = p.abs_transform();
                    // outline precision: the drawing's units at up to 16 device pixels each
                    let res = (t.sx * t.sy - t.kx * t.ky).abs().sqrt().max(1.0) * 16.0;
                    let mut ts = s.to_tiny_skia();
                    let dashed = match ts.dash.take() {
                        Some(d) => p.data().dash(&d, res),
                        None => Some(p.data().clone()),
                    };
                    if let Some(outline) = dashed.and_then(|d| d.stroke(&ts, res)) {
                        stroke = Some(layer(outline, (FillRule::NonZero, tiny_skia::FillRule::Winding), t, LayerKind::Stroke, opacity * s.opacity().get(), source_of(s.paint())?));
                    }
                }
                let (a, b) = if fill_first { (fill, stroke) } else { (stroke, fill) };
                out.extend(a.into_iter().chain(b).flatten());
            }
            usvg::Node::Image(_) => return Err("embedded image".into()),
            usvg::Node::Text(_) => return Err("text".into()),
        }
    }
    Ok(())
}

fn layer(p: tiny_skia::Path, rule: (FillRule, tiny_skia::FillRule), t: tiny_skia::Transform, kind: LayerKind, opacity: f32, source: PaintSource) -> Option<VectorLayer> {
    if opacity <= 0.0 {
        return None;
    }
    let transform = affine(t);
    let b = p.bounds();
    let bounds = transform.apply_rect(&Rect::new(b.left(), b.top(), b.right(), b.bottom()));
    Some(VectorLayer { path: Arc::new(path_data(&p, rule.0)), transform, kind, opacity, source, bounds, ts: p, ts_rule: rule.1, ts_transform: t })
}

fn content_hash(svg: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    svg.hash(&mut h);
    h.finish()
}

/// Why an SVG is not drawn as vectors.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotVector {
    /// It does not parse (the raster path reports it as a load error).
    Invalid(String),
    /// It uses what the vector path cannot draw yet (a filter, a mask, a gradient…).
    Unsupported(String),
}

type Parsed = Result<Arc<VectorArt>, NotVector>;

static PARSED: LazyLock<Mutex<HashMap<(u64, usize), Parsed>>> = LazyLock::new(Mutex::default);

impl VectorArt {
    /// `svg` as vector art, parsed once per content (cached for the process); `Err` says why it
    /// draws through the raster path instead (it does not parse, or uses what the vector path cannot
    /// draw yet: a filter, a mask, a gradient…).
    pub fn parse(svg: &[u8]) -> Parsed {
        let key = (content_hash(svg), svg.len());
        if let Some(p) = PARSED.lock().unwrap_or_else(|p| p.into_inner()).get(&key) {
            return p.clone();
        }
        let parsed = Self::parse_uncached(svg, key.0).map(Arc::new);
        PARSED.lock().unwrap_or_else(|p| p.into_inner()).insert(key, parsed.clone());
        parsed
    }

    fn parse_uncached(svg: &[u8], content: u64) -> Result<VectorArt, NotVector> {
        // classed elements resolve `currentColor` against their own (slot) colour
        let svg = slot_svg(svg).map_err(NotVector::Invalid)?;
        let mut o = usvg::Options { style_sheet: Some(sentinel_sheet()), ..Default::default() };
        o.image_href_resolver.resolve_string = Box::new(|_, _| None);
        let tree = usvg::Tree::from_data(&svg, &o).map_err(|e| NotVector::Invalid(e.to_string()))?;
        let s = tree.size();
        if s.width() <= 0.0 || s.height() <= 0.0 {
            return Err(NotVector::Invalid("empty svg".into()));
        }
        let mut layers = Vec::new();
        walk(tree.root(), 1.0, &mut layers).map_err(NotVector::Unsupported)?;
        Ok(VectorArt { size: Size::new(s.width(), s.height()), layers, content })
    }

    /// Whether every layer takes its colour from `currentColor` or a slot (a monochrome icon).
    pub fn is_mono(&self) -> bool {
        self.layers.iter().all(|l| !matches!(l.source, PaintSource::Color(_)))
    }

    /// The commit key of layer `i`'s path.
    pub fn path_key(&self, i: usize) -> String {
        format!("vpath:{:016x}/{i}", self.content)
    }

    /// The transform that fits the drawing into `rect` keeping its aspect ratio, centred (as the
    /// icon rasters are).
    pub fn fit(&self, rect: Rect) -> Affine {
        let k = (rect.width() / self.size.w).min(rect.height() / self.size.h);
        let (w, h) = (self.size.w * k, self.size.h * k);
        Affine::translate(rect.min.x + (rect.width() - w) / 2.0, rect.min.y + (rect.height() - h) / 2.0).then(&Affine::scale(k, k))
    }

    /// Records the layers as [`Prim::Path`]s through `to_rect` (the drawing's coordinates → the
    /// node's), each in the colour and palette slot `paint` gives its source and opacity. `false`
    /// (nothing recorded) when the resources take no paths (the raster path draws it instead).
    pub fn record(&self, cx: &mut PaintCx, to_rect: Affine, paint: &dyn Fn(PaintSource, f32) -> (Color, Option<PaletteSlot>)) -> bool {
        let mut ids = Vec::with_capacity(self.layers.len());
        for (i, l) in self.layers.iter().enumerate() {
            let data = l.path.clone();
            let Some(id) = cx.res.path(&self.path_key(i), &mut || Some(data.clone())) else { return false };
            ids.push(id);
        }
        for (l, id) in self.layers.iter().zip(ids) {
            let (color, slot) = paint(l.source, l.opacity);
            let ink = to_rect.apply_rect(&l.bounds);
            cx.push(Prim::Path(PathPrim { path: id, transform: to_rect.then(&l.transform), paint: PathPaint::Fill(color), slot }), ink);
        }
        true
    }
}

/// Where a layer goes in [`slot_masks`]: the RGBA raster of literal colours, or a slot's mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaskChannel {
    Literal,
    Slot(u8),
}

/// The channels [`slot_masks`] makes for `art`, in its order.
pub fn mask_channels(art: &VectorArt, channel: &dyn Fn(PaintSource) -> MaskChannel) -> Vec<MaskChannel> {
    let mut lit = false;
    let mut slots: Vec<u8> = Vec::new();
    for l in &art.layers {
        match channel(l.source) {
            MaskChannel::Literal => lit = true,
            MaskChannel::Slot(s) if !slots.contains(&s) => slots.push(s),
            MaskChannel::Slot(_) => {}
        }
    }
    slots.sort();
    lit.then_some(MaskChannel::Literal).into_iter().chain(slots.into_iter().map(MaskChannel::Slot)).collect()
}

/// The raster of `art` in a `px`-pixel square (fitted and centred) as one coverage mask per palette
/// slot its layers use (`A8`, to be tinted at draw time) plus, when it has literal colours, one
/// premultiplied RGBA raster of those. `channel` says where each layer's source goes. A later layer
/// knocks out what it covers of the earlier ones, so drawing the masks tinted, in order, composes as
/// the SVG does (up to antialiased edges shared by two channels). One entry per channel a layer
/// uses, literal first, then by slot.
pub fn slot_masks(art: &VectorArt, px: u32, channel: &dyn Fn(PaintSource) -> MaskChannel) -> Vec<(MaskChannel, ImageData)> {
    let px = px.max(1);
    let n = (px * px) as usize;
    let fit = art.fit(Rect::new(0.0, 0.0, px as f32, px as f32));
    let fit = tiny_skia::Transform::from_row(fit.a, fit.b, fit.c, fit.d, fit.e, fit.f);
    let mut literal: Option<Vec<[f32; 4]>> = None;
    let mut masks: Vec<(u8, Vec<f32>)> = Vec::new();
    let Some(mut cov) = tiny_skia::Mask::new(px, px) else { return Vec::new() };
    for l in &art.layers {
        cov.clear();
        cov.fill_path(&l.ts, l.ts_rule, true, fit.pre_concat(l.ts_transform));
        let ch = channel(l.source);
        let lit = match (ch, l.source) {
            (MaskChannel::Literal, PaintSource::Color(c)) => Some(c),
            (MaskChannel::Literal, _) => Some(Color::BLACK),
            _ => None,
        };
        if let MaskChannel::Slot(s) = ch
            && !masks.iter().any(|(k, _)| *k == s)
        {
            masks.push((s, vec![0.0; n]));
        }
        if lit.is_some() && literal.is_none() {
            literal = Some(vec![[0.0; 4]; n]);
        }
        for (i, &c) in cov.data().iter().enumerate() {
            if c == 0 {
                continue;
            }
            let a = l.opacity * c as f32 / 255.0;
            let keep = 1.0 - a;
            if let Some(lt) = &mut literal {
                for v in &mut lt[i] {
                    *v *= keep;
                }
            }
            for (_, m) in &mut masks {
                m[i] *= keep;
            }
            match (ch, lit) {
                (MaskChannel::Literal, Some(col)) => {
                    let p = &mut literal.as_mut().expect("made above")[i];
                    let ca = col.a * a;
                    p[0] += col.r * ca;
                    p[1] += col.g * ca;
                    p[2] += col.b * ca;
                    p[3] += ca;
                }
                (MaskChannel::Slot(s), _) => {
                    if let Some((_, m)) = masks.iter_mut().find(|(k, _)| *k == s) {
                        m[i] += a;
                    }
                }
                _ => {}
            }
        }
    }
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let size = DeviceSize { w: px, h: px };
    let mut out = Vec::new();
    if let Some(lt) = literal {
        let pixels = lt.iter().flat_map(|p| p.map(q)).collect();
        out.push((MaskChannel::Literal, ImageData { size, format: ImageFormat::Rgba8Premul, pixels }));
    }
    masks.sort_by_key(|(k, _)| *k);
    for (s, m) in masks {
        out.push((MaskChannel::Slot(s), ImageData { size, format: ImageFormat::A8, pixels: m.iter().map(|&v| q(v)).collect() }));
    }
    out
}

/// Records SVG `svg` as vectors in `dest` (its drawing stretched to fill it, as the image rasters
/// are): in its own colours, or (`tint`) as a monochrome icon tinted by `tint` from palette slot 0.
/// `false` when it does not vectorise (the caller rasterises it) or the resources take no paths.
pub fn record_svg(cx: &mut PaintCx, svg: &[u8], dest: Rect, tint: Option<Color>) -> bool {
    let Ok(art) = VectorArt::parse(svg) else { return false };
    let to_rect = Affine::translate(dest.min.x, dest.min.y).then(&Affine::scale(dest.width() / art.size.w, dest.height() / art.size.h));
    let paint = |src: PaintSource, opacity: f32| match (tint, src) {
        (Some(t), PaintSource::Color(c)) => (t.with_alpha(t.a * opacity * c.a), Some(PaletteSlot::new(0, opacity * c.a))),
        (Some(t), _) => (t.with_alpha(t.a * opacity), Some(PaletteSlot::new(0, opacity))),
        // an image's own colours; `currentColor` in an image is black, as resvg draws it
        (None, PaintSource::Color(c)) => (c.with_alpha(c.a * opacity), None),
        (None, _) => (Color::BLACK.with_alpha(opacity), None),
    };
    art.record(cx, to_rect, &paint)
}

/// Whether a box of `p` physical pixels draws as vectors.
pub fn is_vector_size(p: u32) -> bool {
    p > VECTOR_THRESHOLD_PX
}
