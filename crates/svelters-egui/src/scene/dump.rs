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
    /// SG6: nodes the walk visited and subtrees it culled (`walk::culled`), all passes.
    pub nodes_walked: u64,
    pub nodes_culled: u64,
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
    /// SG4: provider code that panicked with its payload taken out (`ui`, `layout`, `prepare`).
    pub provider_panics: u64,
    /// SG4: `StateProvider::update` calls, and custom layouts run (`Caps::CUSTOM_LAYOUT`).
    pub provider_updates: u64,
    pub custom_layouts: u64,
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
    /// SG6: `bytes` by column (`core`, `children`, `layout`, …), for the memory budget's report.
    pub bytes_by: Vec<(&'static str, usize)>,
    /// SG6: distinct taffy styles and common-prop sets the nodes share, and the nodes with a
    /// (boxed) taffy layout cache (`scene/compact.rs`).
    pub styles_interned: usize,
    pub commons_interned: usize,
    pub layout_caches: usize,
    /// SG6: nodes the walk visited, and subtrees (or leaves) it culled off screen, all passes.
    pub nodes_walked: u64,
    pub nodes_culled: u64,
    /// SG6: nodes whose subtree paints outside their own bounds (`ink`, `NodeFlags::INK_OVERFLOW`).
    pub ink_overflows: usize,
    /// SG6: `WidgetCx::cached_shapes` entries, hits and misses.
    pub paint_cached: usize,
    pub paint_hits: u64,
    pub paint_misses: u64,
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
    /// SG4: see `Counters`.
    pub provider_panics: u64,
    pub provider_updates: u64,
    pub custom_layouts: u64,
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
        let bytes_by = self.bytes_by();
        let bytes = bytes_by.iter().map(|(_, b)| b).sum();
        SceneStats {
            live: self.len(),
            roots: self.roots.len(),
            per_tag,
            bytes,
            bytes_by,
            styles_interned: self.styles.len(),
            commons_interned: self.commons.len(),
            layout_caches: self.layout.iter().filter(|l| l.cache.is_boxed()).count(),
            created: c.created,
            destroyed: c.destroyed,
            layouts: c.layouts,
            boundaries_laid_out: c.boundaries_laid_out,
            leaves_measured: c.leaves_measured,
            provider_panics: c.provider_panics,
            provider_updates: c.provider_updates,
            custom_layouts: c.custom_layouts,
            leaf_layouts: c.leaf_layouts,
            nodes_placed: c.nodes_placed,
            nodes_walked: c.nodes_walked,
            nodes_culled: c.nodes_culled,
            ink_overflows: self.ink.len(),
            paint_cached: self.paint_cache.len(),
            paint_hits: self.paint_cache.hits,
            paint_misses: self.paint_cache.misses,
            writes: c.writes,
            writes_busy: c.writes_busy,
            deferred_run: c.deferred_run,
            announced: c.announced,
            observers: self.journal.len(),
            hit_entries: self.hits.len(),
        }
    }

    /// SG6: the scene's memory estimate by column (ARCH-SCENE §8.5): columns' capacity times their
    /// element size, plus the heap each element owns where it is cheap to count (children lists,
    /// boxed cold data and its strings, the CSS per-node data).
    pub(crate) fn bytes_by(&self) -> Vec<(&'static str, usize)> {
        use std::mem::size_of;
        let mut children = self.children.capacity() * size_of::<Vec<NodeId>>();
        let mut cold = self.cold.capacity() * size_of::<Option<Box<NodeCold>>>();
        for (i, kids) in self.children.iter().enumerate() {
            children += kids.capacity() * size_of::<NodeId>();
            if let Some(c) = &self.cold[i] {
                cold += size_of::<NodeCold>() + c.observe.capacity() * 8 + (c.listen_overflow.capacity() + c.attached.capacity() * 2 + c.hoisted.capacity() * 2) * 4;
            }
        }
        // SG6: the interned values count once (with their strings), the column per node
        let strings: usize = self.commons.values().map(|c| c.tooltip.capacity() + c.accessible_name.capacity() + c.anchor.capacity()).sum();
        vec![
            ("core", self.core.capacity() * size_of::<NodeCore>()),
            ("children", children),
            ("layout", self.layout.capacity() * size_of::<LayoutBox>() + self.layout.iter().filter(|l| l.cache.is_boxed()).count() * size_of::<taffy::Cache>()),
            ("payload", self.payload.capacity() * size_of::<Payload>()),
            ("cold", cold),
            ("common", self.common.capacity() * size_of::<Option<std::sync::Arc<super::Common>>>() + self.commons.bytes() + strings),
            ("styles", self.styles.bytes()),
            ("ext", self.ext.bytes()),
            ("slots", self.slots.bytes()),
            ("style", self.style.bytes()),
        ]
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
