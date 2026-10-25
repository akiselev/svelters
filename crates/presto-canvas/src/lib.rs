//! presto's native canvas layers (`plans/research/presto/architecture.md` §9.5, plan W-V2).
//!
//! A canvas holds the CAD scenes (PCB, schematic, 2D sketches) as typed instance chunks instead of
//! display lists: millions of primitives in `f64` world coordinates, culled and cut by level of detail
//! per chunk, drawn by analytic shaders. Pan and zoom are one camera change on the compositor, with no
//! UI commit and no re-upload; the static plane is cached to a texture between edits, and the overlay
//! plane (hover, selection, rubber bands) redraws alone.
//!
//! | Module | What |
//! |---|---|
//! | [`world`] | `f64` world points and rects, the [`Camera`], camera-relative chunk transforms |
//! | [`chunk`] | the instance records and [`ChunkBuilder`]: rects, segments, circles, arcs, polygons, labels |
//! | [`canvas`] | the shared [`Canvas`]: static and overlay planes, atomic edits, snapshots |
//! | [`cull`] | per-frame culling and LOD ([`plan`]) |
//! | [`text`] | canvas fonts: registration, line layout, the glyph cache |
//! | [`slug`] | Slug GPU text: outlines to quadratic bands, and the coverage the shader computes |
//! | [`gpu`] | [`CanvasRenderer`]: chunk buffers, the glyph table, pipelines, the static cache |
//!
//! presto-render draws `Prim::Canvas` through [`CanvasRenderer`]; the compositor owns each canvas's
//! [`CanvasView`] (camera plus its view transform) as a property.

pub mod canvas;
pub mod chunk;
pub mod cull;
pub mod gpu;
pub mod slug;
pub mod text;
pub mod world;

pub use canvas::{Canvas, Edit, Plane, Snapshot};
pub use chunk::{Cap, Chunk, ChunkBuilder, ChunkId, FillRule, Label};
pub use cull::{plan, ChunkDraw, LodParams, PlanStats};
pub use gpu::{CanvasRenderer, CanvasStats, Placement};
pub use text::{CanvasFonts, FontKey, GlyphKey};
pub use world::{Camera, DVec2, Projection, WorldRect, YAxis};

use presto_base::geom::Affine;

/// A canvas's compositor-side view state: the camera, and the view transform a `FastCommit::CanvasView`
/// sets (applied in canvas points after the camera, e.g. a pinch in progress), with the LOD settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasView {
    pub camera: Camera,
    pub view: Affine,
    pub lod: LodParams,
}

impl Default for CanvasView {
    fn default() -> Self {
        CanvasView { camera: Camera::default(), view: Affine::IDENTITY, lod: LodParams::default() }
    }
}
