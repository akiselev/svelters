//! Path tessellation (architecture §8.1 `Path`): lyon fill and stroke meshes in the path's local
//! space, cached per (path, quantised device scale, paint). The renderer draws the meshes into a 4×
//! MSAA intermediate sized to each path batch's bounds and composites the resolved result.

use std::collections::HashMap;

use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexBuffers,
};
use skelter_base::commit::{FillRule, PathCmd, PathData};
use skelter_base::geom::Rect;
use skelter_base::ids::PathId;

use crate::gpu::PathVertex;

/// Tessellation tolerance in device pixels.
const TOLERANCE_PX: f32 = 0.2;
/// Scale steps per octave in the cache key: a mesh is reused within 2^(1/8) ≈ 9% of its scale.
const SCALE_STEPS: f32 = 8.0;
/// Meshes unused for this many frames are dropped.
pub const IDLE_FRAMES: u64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshKey {
    pub path: PathId,
    pub scale_step: i32,
    /// `None` = fill; `Some(width bits)` = stroke of that local width.
    pub stroke: Option<u32>,
}

impl MeshKey {
    /// The key for drawing `path` at `ppu` device pixels per local unit.
    pub fn new(path: PathId, ppu: f32, stroke_width: Option<f32>) -> Self {
        let scale_step = (ppu.max(1e-6).log2() * SCALE_STEPS).round() as i32;
        MeshKey { path, scale_step, stroke: stroke_width.map(f32::to_bits) }
    }
    fn ppu(&self) -> f32 {
        (self.scale_step as f32 / SCALE_STEPS).exp2()
    }
}

pub struct Mesh {
    pub vertices: Vec<PathVertex>,
    pub indices: Vec<u32>,
    /// Local bounds of the vertices.
    pub bounds: Rect,
    pub gpu: Option<GpuMesh>,
    last_used: u64,
}

pub struct GpuMesh {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
}

#[derive(Default)]
pub struct PathCache {
    meshes: HashMap<MeshKey, Mesh>,
    fill: FillTessellator,
    stroke: StrokeTessellator,
    frame: u64,
}

fn build_path(data: &PathData) -> Path {
    let mut b = Path::builder();
    let mut open = false;
    let mut last = point(0.0, 0.0);
    let start = |b: &mut lyon::path::builder::NoAttributes<lyon::path::BuilderImpl>, open: &mut bool, at| {
        if !*open {
            b.begin(at);
            *open = true;
        }
    };
    for cmd in &data.cmds {
        match *cmd {
            PathCmd::MoveTo(x, y) => {
                if open {
                    b.end(false);
                    open = false;
                }
                last = point(x, y);
                start(&mut b, &mut open, last);
            }
            PathCmd::LineTo(x, y) => {
                start(&mut b, &mut open, last);
                last = point(x, y);
                b.line_to(last);
            }
            PathCmd::QuadTo(cx, cy, x, y) => {
                start(&mut b, &mut open, last);
                last = point(x, y);
                b.quadratic_bezier_to(point(cx, cy), last);
            }
            PathCmd::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                start(&mut b, &mut open, last);
                last = point(x, y);
                b.cubic_bezier_to(point(c1x, c1y), point(c2x, c2y), last);
            }
            PathCmd::Close => {
                if open {
                    b.end(true);
                    open = false;
                }
            }
        }
    }
    if open {
        b.end(false);
    }
    b.build()
}

impl PathCache {
    /// The mesh for `key`, tessellating `data` on a miss. `None` when the path is empty or lyon fails.
    pub fn get(&mut self, key: MeshKey, data: &PathData) -> Option<&mut Mesh> {
        if !self.meshes.contains_key(&key) {
            let mesh = self.tessellate(&key, data)?;
            self.meshes.insert(key, mesh);
        }
        let frame = self.frame;
        let mesh = self.meshes.get_mut(&key)?;
        mesh.last_used = frame;
        Some(mesh)
    }

    fn tessellate(&mut self, key: &MeshKey, data: &PathData) -> Option<Mesh> {
        let path = build_path(data);
        let tolerance = TOLERANCE_PX / key.ppu();
        let mut buffers: VertexBuffers<PathVertex, u32> = VertexBuffers::new();
        let result = match key.stroke {
            None => {
                let rule = match data.fill_rule {
                    FillRule::NonZero => lyon::tessellation::FillRule::NonZero,
                    FillRule::EvenOdd => lyon::tessellation::FillRule::EvenOdd,
                };
                let opts = FillOptions::tolerance(tolerance).with_fill_rule(rule);
                self.fill.tessellate_path(
                    &path,
                    &opts,
                    &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| PathVertex { pos: v.position().to_array() }),
                )
            }
            Some(bits) => {
                let opts = StrokeOptions::tolerance(tolerance).with_line_width(f32::from_bits(bits));
                self.stroke.tessellate_path(
                    &path,
                    &opts,
                    &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex| PathVertex {
                        pos: v.position().to_array(),
                    }),
                )
            }
        };
        if result.is_err() || buffers.indices.is_empty() {
            return None;
        }
        let mut bounds = Rect::new(f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for v in &buffers.vertices {
            bounds.min.x = bounds.min.x.min(v.pos[0]);
            bounds.min.y = bounds.min.y.min(v.pos[1]);
            bounds.max.x = bounds.max.x.max(v.pos[0]);
            bounds.max.y = bounds.max.y.max(v.pos[1]);
        }
        Some(Mesh { vertices: buffers.vertices, indices: buffers.indices, bounds, gpu: None, last_used: self.frame })
    }

    pub fn mesh(&self, key: &MeshKey) -> Option<&Mesh> {
        self.meshes.get(key)
    }

    pub fn mesh_mut(&mut self, key: &MeshKey) -> Option<&mut Mesh> {
        self.meshes.get_mut(key)
    }

    pub fn forget(&mut self, path: PathId) {
        self.meshes.retain(|k, _| k.path != path);
    }

    pub fn clear_gpu(&mut self) {
        for m in self.meshes.values_mut() {
            m.gpu = None;
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.meshes.len()
    }

    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.meshes.retain(|_, m| frame.saturating_sub(m.last_used) < IDLE_FRAMES);
        self.frame += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> PathData {
        PathData {
            cmds: vec![PathCmd::MoveTo(0.0, 0.0), PathCmd::LineTo(10.0, 0.0), PathCmd::LineTo(0.0, 10.0), PathCmd::Close],
            fill_rule: FillRule::NonZero,
        }
    }

    #[test]
    fn fill_and_stroke_meshes_are_cached_per_key() {
        let mut cache = PathCache::default();
        let k = MeshKey::new(PathId(1), 2.0, None);
        let m = cache.get(k, &triangle()).unwrap();
        assert_eq!(m.indices.len(), 3);
        assert_eq!(m.bounds, Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(MeshKey::new(PathId(1), 2.05, None), k);
        assert_ne!(MeshKey::new(PathId(1), 3.0, None), k);
        let s = MeshKey::new(PathId(1), 2.0, Some(2.0));
        let sm = cache.get(s, &triangle()).unwrap();
        assert!(sm.bounds.min.x < 0.0);
        assert_eq!(cache.len(), 2);
        cache.forget(PathId(1));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn empty_path_has_no_mesh() {
        let mut cache = PathCache::default();
        assert!(cache.get(MeshKey::new(PathId(2), 1.0, None), &PathData::default()).is_none());
    }
}
