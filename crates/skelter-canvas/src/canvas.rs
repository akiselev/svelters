//! A canvas's content, shared between the thread that edits it and the renderer that draws it.
//!
//! Two planes: [`Plane::Static`], the bulk of the scene (a board's copper, a schematic's symbols),
//! which the renderer caches to a texture while neither it nor the view changes, and
//! [`Plane::Overlay`] (hover, selection, the router's rubber band), drawn over the cache every frame
//! the canvas is drawn, so changing it costs only its own primitives.
//!
//! Edits build a new [`Snapshot`] (chunk lists are `Arc`s, so an edit copies pointers, not records)
//! and publish it; the renderer takes the current snapshot once per frame. Nothing waits on the GPU.

use crate::chunk::{Chunk, ChunkId};
use crate::text::CanvasFonts;
use skelter_base::ids::CanvasId;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plane {
    Static,
    Overlay,
}

/// One plane's chunks in paint order (`z`, then insertion).
#[derive(Clone, Debug, Default)]
pub struct PlaneSnapshot {
    pub chunks: Vec<(ChunkId, Arc<Chunk>)>,
    /// Bumped by every edit of this plane.
    pub revision: u64,
}

impl PlaneSnapshot {
    fn sort(&mut self) {
        self.chunks.sort_by_key(|(id, c)| (c.z, *id));
    }

    pub fn primitive_count(&self) -> usize {
        self.chunks.iter().map(|(_, c)| c.primitive_count()).sum()
    }
}

/// A canvas's content at one moment.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub statics: PlaneSnapshot,
    pub overlay: PlaneSnapshot,
}

impl Snapshot {
    pub fn plane(&self, p: Plane) -> &PlaneSnapshot {
        match p {
            Plane::Static => &self.statics,
            Plane::Overlay => &self.overlay,
        }
    }
    fn plane_mut(&mut self, p: Plane) -> &mut PlaneSnapshot {
        match p {
            Plane::Static => &mut self.statics,
            Plane::Overlay => &mut self.overlay,
        }
    }
}

/// A canvas: register it with the compositor (`CompositorHandle::register_canvas`) and place it with
/// a `Prim::Canvas` or a `LayerKind::Canvas` layer.
#[derive(Debug)]
pub struct Canvas {
    id: CanvasId,
    fonts: Arc<CanvasFonts>,
    current: Mutex<Arc<Snapshot>>,
    next_chunk: Mutex<u64>,
}

impl Canvas {
    pub fn new(id: CanvasId, fonts: Arc<CanvasFonts>) -> Arc<Self> {
        Arc::new(Canvas { id, fonts, current: Mutex::default(), next_chunk: Mutex::new(1) })
    }

    pub fn id(&self) -> CanvasId {
        self.id
    }

    pub fn fonts(&self) -> &Arc<CanvasFonts> {
        &self.fonts
    }

    /// The current content (cheap: one `Arc` clone).
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Applies edits atomically: the renderer sees all of them or none. Tell the compositor afterwards
    /// (`CompositorHandle::canvas_changed`) so it damages the canvas.
    pub fn edit<R>(&self, f: impl FnOnce(&mut Edit<'_>) -> R) -> R {
        let mut cur = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = (**cur).clone();
        let mut ids = self.next_chunk.lock().unwrap_or_else(|e| e.into_inner());
        let mut e = Edit { snap: &mut next, next_id: &mut ids, touched: [false; 2] };
        let r = f(&mut e);
        let touched = e.touched;
        for (i, p) in [Plane::Static, Plane::Overlay].into_iter().enumerate() {
            if touched[i] {
                let plane = next.plane_mut(p);
                plane.revision += 1;
                plane.sort();
            }
        }
        *cur = Arc::new(next);
        r
    }
}

/// Edits of one [`Canvas::edit`] call.
pub struct Edit<'a> {
    snap: &'a mut Snapshot,
    next_id: &'a mut u64,
    touched: [bool; 2],
}

impl Edit<'_> {
    fn touch(&mut self, p: Plane) -> &mut PlaneSnapshot {
        self.touched[match p {
            Plane::Static => 0,
            Plane::Overlay => 1,
        }] = true;
        self.snap.plane_mut(p)
    }

    pub fn insert(&mut self, plane: Plane, chunk: impl Into<Arc<Chunk>>) -> ChunkId {
        let id = ChunkId(*self.next_id);
        *self.next_id += 1;
        self.touch(plane).chunks.push((id, chunk.into()));
        id
    }

    /// Replaces a chunk's content; `false` when the plane has no such chunk.
    pub fn replace(&mut self, plane: Plane, id: ChunkId, chunk: impl Into<Arc<Chunk>>) -> bool {
        let chunk = chunk.into();
        match self.touch(plane).chunks.iter_mut().find(|(c, _)| *c == id) {
            Some(slot) => {
                slot.1 = chunk;
                true
            }
            None => false,
        }
    }

    pub fn remove(&mut self, plane: Plane, id: ChunkId) -> bool {
        let p = self.touch(plane);
        let n = p.chunks.len();
        p.chunks.retain(|(c, _)| *c != id);
        p.chunks.len() != n
    }

    pub fn clear(&mut self, plane: Plane) {
        self.touch(plane).chunks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkBuilder;
    use crate::world::{DVec2, WorldRect};
    use skelter_base::color::Color;

    fn chunk(z: i32) -> Chunk {
        let mut b = ChunkBuilder::new(DVec2::ZERO).z(z);
        b.rect(WorldRect::new(0.0, 0.0, 1.0, 1.0), 0.0, 0.0, Color::WHITE);
        b.build()
    }

    #[test]
    fn edits_publish_whole_snapshots_and_bump_only_their_plane() {
        let c = Canvas::new(CanvasId(1), CanvasFonts::new());
        let before = c.snapshot();
        let (a, b) = c.edit(|e| (e.insert(Plane::Static, chunk(2)), e.insert(Plane::Static, chunk(1))));
        let s = c.snapshot();
        assert_eq!(before.statics.chunks.len(), 0, "old snapshots are untouched");
        assert_eq!(s.statics.revision, 1);
        assert_eq!(s.overlay.revision, 0);
        assert_eq!(s.statics.chunks.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![b, a], "sorted by z");
        c.edit(|e| assert!(e.replace(Plane::Static, a, chunk(0))));
        assert_eq!(c.snapshot().statics.chunks[0].0, a);
        c.edit(|e| e.insert(Plane::Overlay, chunk(0)));
        assert_eq!(c.snapshot().statics.revision, 2);
        assert_eq!(c.snapshot().overlay.revision, 1);
    }
}
