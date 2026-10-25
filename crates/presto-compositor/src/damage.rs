//! Damage (architecture §7.4): which device pixels of a surface must be redrawn this frame.
//!
//! Every frame that draws flattens the scene anyway, so damage is computed by diffing the flattened
//! items against the previous frame's: an item whose list, resolved transform, clip or opacity
//! changed damages its old and new bounds; an item that appeared or went away damages its bounds; an
//! item that survived but moved in paint order relative to the other survivors damages its bounds (a
//! removal or insertion alone reorders no one). That covers the journal's dirty nodes, changed
//! property nodes and running animations without separate bookkeeping, and it is the full-scene check
//! the architecture asks for as a debug mode, run always. Explicit rects (`LayerDelta::Damage`, the
//! caret) are added on top. Damage limits what is rendered; presenting is the GPU side's
//! (`crate::gpu`).

use crate::tree::{FlatItem, SurfaceKey};
use presto_base::display::DisplayList;
use presto_base::geom::{Affine, DeviceRect, DeviceSize, Rect, Scale};
use presto_base::ids::{LayerId, NodeId};
use presto_base::props::ClipShape;
use std::collections::HashMap;
use std::sync::Arc;

/// More rects than this are merged into their bounding box (the renderer scissors per rect).
pub const MAX_RECTS: usize = 8;

type ItemKey = (LayerId, NodeId);

#[derive(Clone, Debug)]
struct ItemSnap {
    bounds: Rect,
    transform: Affine,
    clip: Option<ClipShape>,
    opacity: f32,
    list: Arc<DisplayList>,
    /// Position in the frame's paint order.
    order: usize,
}

/// The previous frame's flattened items of one surface.
#[derive(Default, Debug)]
pub struct Snapshot {
    items: HashMap<ItemKey, ItemSnap>,
}

impl Snapshot {
    /// Diffs `items` against the snapshot, pushes the damaged rects (surface points) and takes `items`
    /// as the new snapshot.
    pub fn diff(&mut self, items: &[FlatItem], out: &mut Vec<Rect>) {
        let mut next = HashMap::with_capacity(items.len());
        // The highest old position among survivors so far: a survivor below it was reordered.
        let mut max_old: Option<usize> = None;
        for (order, it) in items.iter().enumerate() {
            let key = (it.layer, it.node);
            let snap = ItemSnap {
                bounds: if it.opacity > 0.0 { it.bounds } else { Rect::ZERO },
                transform: it.transform,
                clip: it.clip,
                opacity: it.opacity,
                list: it.list.clone(),
                order,
            };
            match self.items.remove(&key) {
                Some(old) => {
                    let reordered = max_old.is_some_and(|m| old.order < m);
                    max_old = Some(max_old.map_or(old.order, |m| m.max(old.order)));
                    if reordered || !same(&old, &snap) {
                        push_nonempty(out, old.bounds);
                        push_nonempty(out, snap.bounds);
                    }
                }
                None => push_nonempty(out, snap.bounds),
            }
            next.insert(key, snap);
        }
        for (_, old) in self.items.drain() {
            push_nonempty(out, old.bounds);
        }
        self.items = next;
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

fn same(a: &ItemSnap, b: &ItemSnap) -> bool {
    a.bounds == b.bounds
        && a.transform == b.transform
        && a.clip == b.clip
        && a.opacity == b.opacity
        && (Arc::ptr_eq(&a.list, &b.list) || *a.list == *b.list)
}

fn push_nonempty(out: &mut Vec<Rect>, r: Rect) {
    if !r.is_empty() {
        out.push(r);
    }
}

/// Converts point rects to device rects at `scale` (one pixel of antialiasing margin), clipped to the
/// surface and merged: overlapping rects are joined, and more than [`MAX_RECTS`] become their bounding
/// box. Empty input gives empty output (no damage).
pub fn to_device(rects: &[Rect], scale: Scale, size: DeviceSize) -> Vec<DeviceRect> {
    let mut out: Vec<DeviceRect> = Vec::new();
    for r in rects {
        let d = r.to_device(scale);
        let d = DeviceRect { x: d.x - 1, y: d.y - 1, w: d.w + 2, h: d.h + 2 }.clip_to(size);
        if !d.is_empty() {
            out.push(d);
        }
    }
    merge(out)
}

/// Joins overlapping or touching rects until none overlap; then caps the count.
pub fn merge(mut rects: Vec<DeviceRect>) -> Vec<DeviceRect> {
    let mut changed = true;
    while changed {
        changed = false;
        'outer: for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                if touches(&rects[i], &rects[j]) {
                    let u = rects[i].union(&rects[j]);
                    rects[i] = u;
                    rects.swap_remove(j);
                    changed = true;
                    break 'outer;
                }
            }
        }
    }
    if rects.len() > MAX_RECTS {
        let all = rects.iter().fold(DeviceRect::default(), |a, r| a.union(r));
        return vec![all];
    }
    rects
}

fn touches(a: &DeviceRect, b: &DeviceRect) -> bool {
    a.x <= b.x + b.w as i32 && b.x <= a.x + a.w as i32 && a.y <= b.y + b.h as i32 && b.y <= a.y + a.h as i32
}

/// Whether `rects` cover the whole surface (a full redraw).
pub fn covers(rects: &[DeviceRect], size: DeviceSize) -> bool {
    rects.iter().any(|r| r.x <= 0 && r.y <= 0 && r.x + r.w as i32 >= size.w as i32 && r.y + r.h as i32 >= size.h as i32)
}

/// The damage a surface has accumulated since it last drew, in device pixels, plus the flag for a
/// full redraw.
#[derive(Default, Debug)]
pub struct Pending {
    pub rects: Vec<DeviceRect>,
    pub full: bool,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        !self.full && self.rects.is_empty()
    }
    pub fn add(&mut self, rects: impl IntoIterator<Item = DeviceRect>) {
        self.rects.extend(rects);
    }
    pub fn take(&mut self, size: DeviceSize) -> Option<Vec<DeviceRect>> {
        if self.full {
            self.full = false;
            self.rects.clear();
            return Some(vec![DeviceRect { x: 0, y: 0, w: size.w, h: size.h }]);
        }
        if self.rects.is_empty() {
            return None;
        }
        let r = merge(std::mem::take(&mut self.rects));
        Some(r)
    }
}

/// Per-surface snapshots of one window.
pub type Snapshots = HashMap<SurfaceKey, Snapshot>;
