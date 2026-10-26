//! Per-frame planning, on the CPU: which chunks of a plane are visible through the canvas's device
//! rectangle, how much of each to draw at this zoom, and the transform each draws with.
//!
//! Culling tests each chunk's world bounds against the world rectangle the device clip sees (a
//! linear scan: a plane holds hundreds to a few thousand chunks). Level of detail cuts each chunk to
//! the prefix of its records that are at least [`LodParams::lod_px`] device pixels big
//! ([`crate::chunk`]), and its labels into glyphs, greeked bars and hidden ones by their size on screen.

use crate::canvas::PlaneSnapshot;
use crate::chunk::{Chunk, ChunkId, ChunkLod, LabelLod};
use crate::world::{ChunkXform, Projection};
use skelter_base::geom::Rect;
use std::sync::Arc;

/// Level-of-detail and stroke settings of one canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodParams {
    /// Records smaller than this on screen (device pixels) are not drawn.
    pub lod_px: f32,
    /// Strokes and segments are drawn at least this wide (device pixels), so hairlines stay visible.
    pub min_stroke_px: f32,
    /// Labels with an em size of at least this many device pixels draw as glyphs (Slug)...
    pub glyph_min_px: f32,
    /// ...down to this size as greeked bars; smaller ones not at all.
    pub greek_min_px: f32,
    /// Chunks this close outside the clip (device pixels) still count as visible (antialiasing fringe).
    pub cull_margin_px: f32,
}

impl Default for LodParams {
    fn default() -> Self {
        // Architecture §9.5: below about 6–8 px on screen labels become greeked bars.
        LodParams { lod_px: 0.5, min_stroke_px: 1.0, glyph_min_px: 6.0, greek_min_px: 1.5, cull_margin_px: 2.0 }
    }
}

/// One visible chunk's draw.
#[derive(Clone, Debug)]
pub struct ChunkDraw {
    pub id: ChunkId,
    pub chunk: Arc<Chunk>,
    pub xform: ChunkXform,
    pub lod: ChunkLod,
    pub labels: LabelLod,
}

impl ChunkDraw {
    /// Primitives this draw puts on screen (polygons count by triangles' polygons, labels once).
    pub fn primitives(&self) -> u64 {
        let polys = self.chunk.poly_ranges.iter().filter(|(_, end)| *end <= self.lod.poly_indices).count();
        self.lod.shapes as u64 + polys as u64 + self.labels.greek_end as u64
    }
}

/// What planning one plane found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanStats {
    pub chunks: u32,
    /// Chunks inside the view.
    pub visible: u32,
    /// Visible chunks whose every record was below the LOD threshold.
    pub lod_empty: u32,
    /// Shape records drawn.
    pub shapes: u64,
    pub glyphs: u64,
}

/// Appends the visible chunks of `plane` (paint order) to `out`. `clip` is the device rectangle the
/// canvas is visible in.
pub fn plan(plane: &PlaneSnapshot, proj: &Projection, clip: &Rect, lod: &LodParams, out: &mut Vec<ChunkDraw>) -> PlanStats {
    let mut stats = PlanStats { chunks: plane.chunks.len() as u32, ..Default::default() };
    if clip.is_empty() {
        return stats;
    }
    let view = proj.visible_world(&clip.inflate(lod.cull_margin_px));
    if view.is_empty() {
        return stats;
    }
    let ppu = proj.px_per_unit();
    if !(ppu > 0.0) {
        return stats;
    }
    let min_extent = (lod.lod_px as f64 / ppu) as f32;
    let glyph_min = (lod.glyph_min_px as f64 / ppu) as f32;
    let greek_min = (lod.greek_min_px as f64 / ppu) as f32;
    for (id, chunk) in &plane.chunks {
        if !chunk.bounds.intersects(&view) {
            continue;
        }
        stats.visible += 1;
        let l = chunk.lod(min_extent);
        let labels = chunk.label_lod(glyph_min, greek_min);
        if l.shapes == 0 && l.poly_indices == 0 && labels.glyphs == 0 && labels.greek_end == labels.greek_start {
            stats.lod_empty += 1;
            continue;
        }
        stats.shapes += l.shapes as u64;
        stats.glyphs += labels.glyphs as u64;
        out.push(ChunkDraw { id: *id, chunk: chunk.clone(), xform: proj.chunk(chunk.origin), lod: l, labels });
    }
    stats
}
