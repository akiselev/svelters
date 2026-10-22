//! Dumps and counters (ARCH-SCENE §11): [`Scene::dump`] (a stable text tree for tests and failure
//! reports) and [`SceneStats`] (live nodes, memory estimate, layout work). Both are unstable
//! (§16): the dump format and the counter set may change in any sha; regenerate goldens.
//!
//! SG7 adds `SceneSnapshot`. Writer lane: SCENE (TESTKIT reads).

use super::Scene;
use super::id::NodeId;
use super::node::{LayoutBox, NodeCold, NodeCore, NodeFlags, Owner, Payload};

/// Work counters, cumulative since the scene was created.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counters {
    pub created: u64,
    pub destroyed: u64,
    /// Full layouts of a root.
    pub layouts: u64,
    /// Relayouts of a single boundary (content changes inside it).
    pub boundaries_laid_out: u64,
    /// Leaves measured with egui's fonts (intrinsic size).
    pub leaves_measured: u64,
    /// Taffy leaf computations (cache misses of leaves).
    pub leaf_layouts: u64,
    /// Rects placed after a layout.
    pub nodes_placed: u64,
    /// `SceneHandle::write` batches (and deferred ones).
    pub writes: u64,
    /// `write` calls refused with `SceneBusy`.
    pub writes_busy: u64,
    pub deferred_run: u64,
    /// `prop_changed` announcements of Rust writes.
    pub announced: u64,
}

/// What the scene holds and what it did (unstable fields, §16).
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneStats {
    pub live: usize,
    pub roots: usize,
    /// Live nodes per element tag, sorted by tag.
    pub per_tag: Vec<(&'static str, usize)>,
    /// Estimated scene memory: columns' capacity times their element size, plus boxed cold data
    /// and children lists.
    pub bytes: usize,
    pub created: u64,
    pub destroyed: u64,
    pub layouts: u64,
    pub boundaries_laid_out: u64,
    pub leaves_measured: u64,
    pub leaf_layouts: u64,
    pub nodes_placed: u64,
    pub writes: u64,
    pub writes_busy: u64,
    pub deferred_run: u64,
    pub announced: u64,
    /// Live change observers (SG2).
    pub observers: usize,
    /// Entries in the per-viewport hit lists (SG3).
    pub hit_entries: usize,
}

/// What [`Scene::dump`] prints besides tag, id and children.
#[derive(Clone, Copy, Debug, Default)]
pub struct DumpOpts {
    /// Root-relative rects of the last layout.
    pub rects: bool,
    /// Dirty bits and scene flags.
    pub dirty: bool,
    /// State bits by name (built-ins and provider states).
    pub state: bool,
    /// The extension keys present, by name (SG2).
    pub ext: bool,
}

impl Scene {
    pub(crate) fn stats(&self) -> SceneStats {
        let c = self.counters;
        let mut per_tag: Vec<(&'static str, usize)> = Vec::new();
        for id in self.ids() {
            let t = self.tag_at(id.index() as usize);
            match per_tag.iter_mut().find(|(x, _)| *x == t) {
                Some(e) => e.1 += 1,
                None => per_tag.push((t, 1)),
            }
        }
        per_tag.sort();
        use std::mem::size_of;
        let mut bytes = self.core.capacity() * size_of::<NodeCore>()
            + self.children.capacity() * size_of::<Vec<NodeId>>()
            + self.layout.capacity() * size_of::<LayoutBox>()
            + self.payload.capacity() * size_of::<Payload>()
            + self.cold.capacity() * size_of::<Option<Box<NodeCold>>>()
            + self.ext.bytes();
        for (i, kids) in self.children.iter().enumerate() {
            bytes += kids.capacity() * size_of::<NodeId>();
            if let Some(cold) = &self.cold[i] {
                bytes += size_of::<NodeCold>() + cold.common.tooltip.capacity() + cold.common.accessible_name.capacity();
            }
        }
        SceneStats {
            live: self.len(),
            roots: self.roots.len(),
            per_tag,
            bytes,
            created: c.created,
            destroyed: c.destroyed,
            layouts: c.layouts,
            boundaries_laid_out: c.boundaries_laid_out,
            leaves_measured: c.leaves_measured,
            leaf_layouts: c.leaf_layouts,
            nodes_placed: c.nodes_placed,
            writes: c.writes,
            writes_busy: c.writes_busy,
            deferred_run: c.deferred_run,
            announced: c.announced,
            observers: self.journal.len(),
            hit_entries: self.hits.len(),
        }
    }

    /// `top`'s subtree, one node per line, indented by depth:
    /// `<tag> #id [owner] "text" {x y w h} dirty(…) flags(…) state(…) ext(…)`.
    pub(crate) fn dump(&self, top: NodeId, opts: DumpOpts) -> String {
        let mut out = String::new();
        if self.idx(top).is_none() {
            return format!("{top} (stale)\n");
        }
        let mut stack = vec![(top, 0usize)];
        while let Some((id, depth)) = stack.pop() {
            let i = self.ix(id);
            let c = &self.core[i];
            for _ in 0..depth {
                out.push_str("  ");
            }
            out.push_str(&format!("<{}> {id}", self.tag_at(i)));
            match c.owner {
                Owner::Js => {}
                Owner::Rust(p) => out.push_str(&format!(" [rust:{}]", p.name())),
                Owner::Host => out.push_str(" [host]"),
            }
            let text = super::node::text_of(self.w(i));
            if !text.is_empty() {
                out.push_str(&format!(" {text:?}"));
            }
            if opts.rects && c.rect != egui::Rect::NOTHING {
                let r = c.rect;
                out.push_str(&format!(" {{{} {} {} {}}}", r.min.x, r.min.y, r.width(), r.height()));
            }
            if opts.dirty {
                if !c.dirty.is_empty() {
                    out.push_str(&format!(" {:?}", c.dirty));
                }
                let f = c.flags & !(NodeFlags::HAD_FOCUS);
                if !f.is_empty() {
                    out.push_str(&format!(" {f:?}"));
                }
                if self.is_boundary(i) {
                    out.push_str(" boundary");
                }
            }
            if opts.state && c.state != crate::provider::StateBits::NONE {
                out.push_str(&format!(" state({})", c.state.names().join("|")));
            }
            if opts.ext && self.ext.has_any(i as u32) {
                let names: Vec<&str> = self.ext.ids_at(i as u32).map(|k| k.name()).collect();
                out.push_str(&format!(" ext({})", names.join(",")));
            }
            out.push('\n');
            for &k in self.children[i].iter().rev() {
                stack.push((k, depth + 1));
            }
        }
        out
    }
}
