//! Typed instance chunks (architecture §9.5): a chunk is a group of primitives near one `f64` origin,
//! stored as `f32` offsets in the records the GPU reads, with what culling and level of detail need.
//!
//! Primitives: rectangles (filled, outlined, rounded), segments (butt, round or square caps), circles
//! and rings, arcs, filled polygons (tessellated when the chunk is built) and text labels. Widths and
//! sizes are world units; [`crate::LodParams::min_stroke_px`] keeps hairlines visible when zoomed out.
//!
//! **Level of detail.** Each kind's records are sorted by their extent (the larger side of their
//! bounds), largest first, so the primitives at least `lod_px` device pixels big are a prefix: a
//! frame draws `0..n` with `n` from a binary search, without touching the records. Paint order
//! inside a chunk is therefore by size (stable among equal sizes); order between chunks is their `z`.
//! Labels are sorted by their size the same way: big enough ones draw as Slug glyphs, smaller ones
//! as greeked bars, the smallest not at all.

use crate::text::{CanvasFonts, FontKey, GlyphKey};
use crate::world::{DVec2, WorldRect, YAxis};
use presto_base::color::Color;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Shape kinds in [`ShapeInstance::kind`].
pub mod kind {
    pub const RECT: u32 = 0;
    pub const SEGMENT: u32 = 1;
    pub const CIRCLE: u32 = 2;
    pub const ARC: u32 = 3;
}

/// One analytic primitive, as the shape pipeline reads it (an instance-rate vertex buffer).
///
/// | kind | `p0` | `p1` | `params` |
/// |---|---|---|---|
/// | rect | min corner | max corner | corner radius, stroke width (0 = filled) |
/// | segment | from | to | width, cap (0 butt, 1 round, 2 square) |
/// | circle | centre | — | radius, ring width (0 = disc) |
/// | arc | centre | — | radius, width, start angle, sweep (radians in world space: 0 along `+x`, π/2 along `+y`) |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeInstance {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub params: [f32; 4],
    /// Straight-alpha sRGB-encoded RGBA8 (`r` in the low byte).
    pub color: u32,
    pub kind: u32,
}

/// A polygon vertex (triangle lists, tessellated at build time).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PolyVertex {
    pub pos: [f32; 2],
    pub color: u32,
}

/// One glyph of a label, before the renderer gives its outline a GPU index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPlacement {
    pub glyph: GlyphKey,
    /// The label's anchor (its baseline origin), chunk-local.
    pub anchor: [f32; 2],
    /// The glyph's pen position from the anchor, in ems along the baseline (`x`) and up (`y`).
    pub pen: [f32; 2],
    /// The em size in world units.
    pub size: f32,
    /// Rotation of the baseline, counter-clockwise on screen: `(cos, sin)`.
    pub rot: [f32; 2],
    pub color: u32,
}

/// Packs a colour into [`ShapeInstance::color`]'s layout.
pub fn pack(c: Color) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    q(c.r) | q(c.g) << 8 | q(c.b) << 16 | q(c.a) << 24
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cap {
    Butt,
    #[default]
    Round,
    Square,
}

impl Cap {
    fn code(self) -> f32 {
        match self {
            Cap::Butt => 0.0,
            Cap::Round => 1.0,
            Cap::Square => 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

/// A chunk's id within its canvas plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkId(pub u64);

/// Every built chunk gets a process-unique version, so GPU buffers are keyed by content.
static VERSIONS: AtomicU64 = AtomicU64::new(1);

/// One label's records: `glyphs[glyph_end of the previous label .. glyph_end]`, and its greeked bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelRange {
    /// Em size in world units (the LOD key).
    pub size: f32,
    pub glyph_end: u32,
}

/// A built chunk: immutable, shared by the canvas snapshots and the renderer.
#[derive(Debug)]
pub struct Chunk {
    /// Content version (unique per build).
    pub version: u64,
    pub origin: DVec2,
    /// World bounds of everything the chunk draws (strokes and labels included).
    pub bounds: WorldRect,
    /// Paint order among the plane's chunks (ties: insertion order).
    pub z: i32,
    /// Sorted by extent, largest first; `shape_extents[i]` is record `i`'s extent in world units.
    pub shapes: Vec<ShapeInstance>,
    pub shape_extents: Vec<f32>,
    pub poly_vertices: Vec<PolyVertex>,
    /// Indices of whole polygons, largest polygon first.
    pub poly_indices: Vec<u32>,
    /// `(extent, index_end)` per polygon, in index order.
    pub poly_ranges: Vec<(f32, u32)>,
    /// Glyphs in label order, labels largest first.
    pub glyphs: Vec<GlyphPlacement>,
    pub labels: Vec<LabelRange>,
    /// One bar per label (a butt-capped segment along its baseline), in label order.
    pub greek: Vec<ShapeInstance>,
}

impl Chunk {
    /// Records with an extent of at least `min_extent` world units: a prefix of each kind.
    pub fn lod(&self, min_extent: f32) -> ChunkLod {
        let shapes = self.shape_extents.partition_point(|e| *e >= min_extent) as u32;
        let polys = self.poly_ranges.partition_point(|(e, _)| *e >= min_extent);
        let poly_indices = if polys == 0 { 0 } else { self.poly_ranges[polys - 1].1 };
        ChunkLod { shapes, poly_indices }
    }

    /// Labels split by em size: `0..glyph` as glyphs (size ≥ `glyph_min`), the next ones up to
    /// `greek_end` as bars (size ≥ `greek_min`).
    pub fn label_lod(&self, glyph_min: f32, greek_min: f32) -> LabelLod {
        let slug = self.labels.partition_point(|l| l.size >= glyph_min);
        let greek = self.labels.partition_point(|l| l.size >= greek_min).max(slug);
        let glyphs = if slug == 0 { 0 } else { self.labels[slug - 1].glyph_end };
        LabelLod { glyphs, greek_start: slug as u32, greek_end: greek as u32 }
    }

    pub fn primitive_count(&self) -> usize {
        self.shapes.len() + self.poly_ranges.len() + self.labels.len()
    }

    /// The largest extent in the chunk (nothing is drawn below it).
    pub fn max_extent(&self) -> f32 {
        let s = self.shape_extents.first().copied().unwrap_or(0.0);
        let p = self.poly_ranges.first().map_or(0.0, |r| r.0);
        let l = self.labels.first().map_or(0.0, |l| l.size);
        s.max(p).max(l)
    }
}

/// How much of a chunk's shapes and polygons one frame draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkLod {
    pub shapes: u32,
    pub poly_indices: u32,
}

/// How a chunk's labels draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LabelLod {
    /// Glyph records `0..glyphs`.
    pub glyphs: u32,
    /// Greeked bars `greek_start..greek_end`.
    pub greek_start: u32,
    pub greek_end: u32,
}

/// A text label: a string laid out along a baseline with the canvas fonts.
#[derive(Clone, Debug, PartialEq)]
pub struct Label<'a> {
    pub text: &'a str,
    pub font: FontKey,
    /// Baseline origin in world units.
    pub pos: DVec2,
    /// Em size in world units.
    pub size: f64,
    /// Counter-clockwise on screen, radians.
    pub rotation: f64,
    pub color: Color,
    /// Horizontal alignment of the text against `pos`: 0 = start, 0.5 = centred, 1 = end.
    pub align: f32,
}

/// Builds one [`Chunk`]. Coordinates are world units; the builder stores them relative to `origin`.
pub struct ChunkBuilder {
    origin: DVec2,
    z: i32,
    y_axis: YAxis,
    bounds: WorldRect,
    shapes: Vec<(f32, ShapeInstance)>,
    polys: Vec<(f32, Vec<PolyVertex>, Vec<u32>)>,
    labels: Vec<(f32, Vec<GlyphPlacement>, ShapeInstance)>,
}

impl ChunkBuilder {
    pub fn new(origin: DVec2) -> Self {
        ChunkBuilder {
            origin,
            z: 0,
            y_axis: YAxis::Down,
            bounds: WorldRect::EMPTY,
            shapes: Vec::new(),
            polys: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub fn z(mut self, z: i32) -> Self {
        self.z = z;
        self
    }

    /// The world's `y` direction the canvas is viewed with, for placing labels' greeked bars (glyphs
    /// follow the camera by themselves). Default: [`YAxis::Down`].
    pub fn y_axis(mut self, y: YAxis) -> Self {
        self.y_axis = y;
        self
    }

    fn rel(&self, p: DVec2) -> [f32; 2] {
        [(p.x - self.origin.x) as f32, (p.y - self.origin.y) as f32]
    }

    fn push(&mut self, extent: f64, world: WorldRect, s: ShapeInstance) {
        self.bounds = self.bounds.union(&world);
        self.shapes.push((extent as f32, s));
    }

    /// A rectangle: filled when `stroke` is 0, else its outline of that width centred on the edge.
    pub fn rect(&mut self, r: WorldRect, radius: f64, stroke: f64, color: Color) -> &mut Self {
        let s = ShapeInstance {
            p0: self.rel(r.min),
            p1: self.rel(r.max),
            params: [radius as f32, stroke as f32, 0.0, 0.0],
            color: pack(color),
            kind: kind::RECT,
        };
        self.push(r.width().max(r.height()) + stroke, r.inflate(stroke * 0.5), s);
        self
    }

    /// A segment of `width` (a track, a wire, an oval pad with round caps).
    pub fn segment(&mut self, a: DVec2, b: DVec2, width: f64, cap: Cap, color: Color) -> &mut Self {
        let s = ShapeInstance {
            p0: self.rel(a),
            p1: self.rel(b),
            params: [width as f32, cap.code(), 0.0, 0.0],
            color: pack(color),
            kind: kind::SEGMENT,
        };
        let mut w = WorldRect::EMPTY;
        w.include(a);
        w.include(b);
        let pad = if cap == Cap::Butt { width * 0.5 } else { width * std::f64::consts::FRAC_1_SQRT_2 };
        self.push((b - a).length() + width, w.inflate(pad), s);
        self
    }

    /// A disc (`ring` 0) or a ring of width `ring` centred on the circle of `radius`.
    pub fn circle(&mut self, c: DVec2, radius: f64, ring: f64, color: Color) -> &mut Self {
        let s = ShapeInstance {
            p0: self.rel(c),
            p1: [0.0; 2],
            params: [radius as f32, ring as f32, 0.0, 0.0],
            color: pack(color),
            kind: kind::CIRCLE,
        };
        let r = radius + ring * 0.5;
        self.push(2.0 * r, WorldRect::new(c.x - r, c.y - r, c.x + r, c.y + r), s);
        self
    }

    /// An arc of `width` with round caps, from `start` sweeping `sweep` radians, both in world space
    /// (0 along `+x`, π/2 along `+y`; a negative sweep goes the other way).
    pub fn arc(&mut self, c: DVec2, radius: f64, width: f64, start: f64, sweep: f64, color: Color) -> &mut Self {
        let s = ShapeInstance {
            p0: self.rel(c),
            p1: [0.0; 2],
            params: [radius as f32, width as f32, start as f32, sweep as f32],
            color: pack(color),
            kind: kind::ARC,
        };
        let r = radius + width * 0.5;
        self.push(2.0 * r, WorldRect::new(c.x - r, c.y - r, c.x + r, c.y + r), s);
        self
    }

    /// A filled polygon: `contours` are closed rings (holes by the fill rule). Self-intersecting or
    /// degenerate input that the tessellator rejects is dropped.
    pub fn polygon(&mut self, contours: &[&[DVec2]], rule: FillRule, color: Color) -> &mut Self {
        use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
        let mut path = lyon::path::Path::builder();
        let mut world = WorldRect::EMPTY;
        for ring in contours {
            let Some((first, rest)) = ring.split_first() else { continue };
            let p = self.rel(*first);
            path.begin(lyon::math::point(p[0], p[1]));
            world.include(*first);
            for q in rest {
                let p = self.rel(*q);
                path.line_to(lyon::math::point(p[0], p[1]));
                world.include(*q);
            }
            path.end(true);
        }
        let path = path.build();
        let mut out: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
        let opts = FillOptions::tolerance(0.01).with_fill_rule(match rule {
            FillRule::NonZero => lyon::tessellation::FillRule::NonZero,
            FillRule::EvenOdd => lyon::tessellation::FillRule::EvenOdd,
        });
        let ok = FillTessellator::new()
            .tessellate_path(&path, &opts, &mut BuffersBuilder::new(&mut out, |v: FillVertex| v.position().to_array()))
            .is_ok();
        if !ok || out.indices.is_empty() {
            return self;
        }
        let color = pack(color);
        let vertices = out.vertices.iter().map(|p| PolyVertex { pos: *p, color }).collect();
        self.bounds = self.bounds.union(&world);
        self.polys.push((world.width().max(world.height()) as f32, vertices, out.indices));
        self
    }

    /// A text label laid out with `fonts` (no shaping: a character map and advances, which covers
    /// reference designators, net names and values). Characters the font lacks draw nothing.
    pub fn label(&mut self, fonts: &CanvasFonts, l: &Label<'_>) -> &mut Self {
        let Some(line) = fonts.layout(l.font, l.text) else { return self };
        let (s, c) = l.rotation.sin_cos();
        let anchor = self.rel(l.pos);
        let color = pack(l.color);
        let shift = -line.advance * l.align as f64;
        let mut glyphs = Vec::with_capacity(line.glyphs.len());
        for g in &line.glyphs {
            glyphs.push(GlyphPlacement {
                glyph: g.key,
                anchor,
                pen: [(g.x + shift) as f32, 0.0],
                size: l.size as f32,
                rot: [c as f32, s as f32],
                color,
            });
        }
        // The extent of the text in world space: the baseline span, up by the ascent. Rotation is
        // screen-relative, so the bounds take the rotated box's bounding square.
        let w = line.advance * l.size;
        let reach = (w.abs() + l.size * (line.ascent + line.descent)).max(l.size);
        let world = WorldRect::new(l.pos.x - reach, l.pos.y - reach, l.pos.x + reach, l.pos.y + reach);
        // The greeked bar: from the text's start to its end along the baseline, at half the x-height.
        let lift = l.size * line.ascent * 0.35;
        let start = shift * l.size;
        // Text space is `y` up; world `y` is down for `YAxis::Down`.
        let ys = if self.y_axis == YAxis::Down { -1.0 } else { 1.0 };
        let along = |d: f64, up: f64| [anchor[0] + (c * d - s * up) as f32, anchor[1] + (ys * (s * d + c * up)) as f32];
        let bar = ShapeInstance {
            p0: along(start, lift),
            p1: along(start + w, lift),
            params: [(l.size * line.ascent * 0.6) as f32, Cap::Butt.code(), 0.0, 0.0],
            color,
            kind: kind::SEGMENT,
        };
        self.bounds = self.bounds.union(&world);
        self.labels.push((l.size as f32, glyphs, bar));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.polys.is_empty() && self.labels.is_empty()
    }

    pub fn build(mut self) -> Chunk {
        let by_extent = |a: &f32, b: &f32| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal);
        self.shapes.sort_by(|a, b| by_extent(&a.0, &b.0));
        self.polys.sort_by(|a, b| by_extent(&a.0, &b.0));
        self.labels.sort_by(|a, b| by_extent(&a.0, &b.0));
        let (shape_extents, shapes) = self.shapes.into_iter().unzip();
        let mut poly_vertices = Vec::new();
        let mut poly_indices = Vec::new();
        let mut poly_ranges = Vec::new();
        for (extent, vs, is) in self.polys {
            let base = poly_vertices.len() as u32;
            poly_vertices.extend(vs);
            poly_indices.extend(is.into_iter().map(|i| i + base));
            poly_ranges.push((extent, poly_indices.len() as u32));
        }
        let mut glyphs = Vec::new();
        let mut labels = Vec::new();
        let mut greek = Vec::new();
        for (size, gs, bar) in self.labels {
            glyphs.extend(gs);
            labels.push(LabelRange { size, glyph_end: glyphs.len() as u32 });
            greek.push(bar);
        }
        Chunk {
            version: VERSIONS.fetch_add(1, Ordering::Relaxed),
            origin: self.origin,
            bounds: self.bounds,
            z: self.z,
            shapes,
            shape_extents,
            poly_vertices,
            poly_indices,
            poly_ranges,
            glyphs,
            labels,
            greek,
        }
    }

    pub fn build_arc(self) -> Arc<Chunk> {
        Arc::new(self.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_relative_and_sorted_for_lod() {
        let o = DVec2::new(1.0e9, 2.0e9);
        let mut b = ChunkBuilder::new(o);
        b.circle(o + DVec2::new(10.0, 0.0), 1.0, 0.0, Color::WHITE);
        b.rect(WorldRect::new(o.x, o.y, o.x + 100.0, o.y + 50.0), 0.0, 0.0, Color::BLACK);
        b.segment(o, o + DVec2::new(30.0, 40.0), 2.0, Cap::Round, Color::WHITE);
        let c = b.build();
        assert_eq!(c.shape_extents, vec![100.0, 52.0, 2.0]);
        assert_eq!(c.shapes[0].p1, [100.0, 50.0]);
        assert_eq!(c.shapes[2].p0, [10.0, 0.0]);
        assert_eq!(c.lod(50.0).shapes, 2);
        assert_eq!(c.lod(1000.0).shapes, 0);
        assert_eq!(c.lod(0.0).shapes, 3);
        assert!(c.bounds.min.x <= o.x - 1.0 && c.bounds.max.x >= o.x + 100.0);
    }

    #[test]
    fn polygons_tessellate_and_cut_by_extent() {
        let mut b = ChunkBuilder::new(DVec2::ZERO);
        let small = [DVec2::new(0.0, 0.0), DVec2::new(1.0, 0.0), DVec2::new(1.0, 1.0)];
        let big = [DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)];
        b.polygon(&[&small], FillRule::NonZero, Color::WHITE);
        b.polygon(&[&big], FillRule::NonZero, Color::WHITE);
        let c = b.build();
        assert_eq!(c.poly_ranges.len(), 2);
        assert_eq!(c.poly_ranges[0].0, 10.0, "largest first");
        assert_eq!(c.lod(5.0).poly_indices, 6, "the square's two triangles only");
        assert_eq!(c.lod(0.5).poly_indices as usize, c.poly_indices.len());
    }
}
