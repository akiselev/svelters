//! Finding nodes (ported from svelters-egui's `scene/query.rs`, ARCH-SCENE §6.2–§6.3): the kind
//! sets, anchors, and [`Query`], which chains predicates (all must hold) and drives from the
//! most selective index it has: the kind set, else a scope's subtree, else the anchors, else every
//! live node. Results come in the driver's order: the kind set's packed order (deterministic for a
//! given history, not tree order), pre-order for a scope. CSS selectors go through svelters-style's matcher
//! ([`Document::select`]).
//!
//! The kind sets are kept by `create` and `free`: per kind, its live nodes in a packed vector with a
//! position per slot, swap-removed when a node is freed.

use crate::document::Document;
use crate::ids::{KindId, PropId};
use crate::value::Value;
use skelter_base::ids::{NodeId, Revision, WindowId};
use std::collections::HashMap;
use svelters_style::StateMask;

const NO_POS: u32 = u32::MAX;

/// Per kind, the live nodes of that kind.
#[derive(Default)]
pub(crate) struct KindSets {
    by_kind: HashMap<KindId, Vec<NodeId>>,
    /// Slot → position in its kind's vector (`NO_POS`: none).
    pos: Vec<u32>,
}

impl KindSets {
    pub fn add(&mut self, kind: KindId, id: NodeId) {
        let i = id.index() as usize;
        if self.pos.len() <= i {
            self.pos.resize(i + 1, NO_POS);
        }
        let v = self.by_kind.entry(kind).or_default();
        self.pos[i] = v.len() as u32;
        v.push(id);
    }

    pub fn remove(&mut self, kind: KindId, id: NodeId) {
        let i = id.index() as usize;
        let p = match self.pos.get(i) {
            Some(&p) if p != NO_POS => p as usize,
            _ => return,
        };
        let Some(v) = self.by_kind.get_mut(&kind) else { return };
        v.swap_remove(p);
        if let Some(&moved) = v.get(p) {
            self.pos[moved.index() as usize] = p as u32;
        }
        self.pos[i] = NO_POS;
    }

    pub fn of(&self, kind: KindId) -> &[NodeId] {
        self.by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }
}

/// A chain of predicates over the document's nodes ([`Document::query`]).
#[derive(Clone)]
pub struct Query<'d> {
    doc: &'d Document,
    kind: Option<KindId>,
    window: Option<WindowId>,
    within: Option<NodeId>,
    anchor: Option<&'d str>,
    state: StateMask,
    props: Vec<(PropId, Value)>,
    since: Option<Revision>,
}

impl<'d> Query<'d> {
    /// Nodes of `kind`.
    pub fn kind(mut self, kind: KindId) -> Self {
        self.kind = Some(kind);
        self
    }
    /// Nodes in `window`'s tree.
    pub fn in_window(mut self, window: WindowId) -> Self {
        self.window = Some(window);
        self
    }
    /// `scope` and the nodes under it.
    pub fn within(mut self, scope: NodeId) -> Self {
        self.within = Some(scope);
        self
    }
    /// Nodes whose `anchor` is `anchor`.
    pub fn anchor(mut self, anchor: &'d str) -> Self {
        self.anchor = Some(anchor);
        self
    }
    /// Nodes with every bit of `state` (`:checked`, `:focus`).
    pub fn state(mut self, state: StateMask) -> Self {
        self.state = StateMask(self.state.0 | state.0);
        self
    }
    /// Nodes whose prop `prop` reads `v` (written or default).
    pub fn prop_eq(mut self, prop: PropId, v: Value) -> Self {
        self.props.push((prop, v));
        self
    }
    /// Nodes changed (any class) after `rev` (R14).
    pub fn changed_since(mut self, rev: Revision) -> Self {
        self.since = Some(rev);
        self
    }

    fn holds(&self, n: NodeId) -> bool {
        let d = self.doc;
        let Some(i) = d.idx(n) else { return false };
        if self.kind.is_some_and(|k| d.core[i].kind != k) || self.window.is_some_and(|w| d.core[i].window != Some(w)) {
            return false;
        }
        if self.since.is_some_and(|r| d.core[i].changed <= r) {
            return false;
        }
        if !d.state[i].contains(self.state) {
            return false;
        }
        if self.anchor.is_some_and(|a| d.anchors.get(&n).is_none_or(|x| &**x != a)) {
            return false;
        }
        if self.within.is_some_and(|s| !d.is_ancestor_or_self(s, n)) {
            return false;
        }
        let v = d.view_at(i);
        self.props.iter().all(|(p, want)| v.prop(*p) == Some(want))
    }

    /// The matching nodes.
    pub fn collect(&self) -> Vec<NodeId> {
        let d = self.doc;
        let driver: Vec<NodeId> = if let Some(s) = self.within.filter(|_| self.kind.is_none()) {
            let mut out = Vec::new();
            let mut stack = vec![s];
            while let Some(n) = stack.pop() {
                out.push(n);
                stack.extend(d.children(n).iter().rev());
            }
            out
        } else if let Some(k) = self.kind {
            d.kinds_of(k).to_vec()
        } else if let Some(a) = self.anchor {
            d.anchors.iter().filter(|(_, x)| &***x == a).map(|(n, _)| *n).collect()
        } else {
            (0..d.core.len()).filter_map(|i| d.slots.id_at(i)).collect()
        };
        driver.into_iter().filter(|&n| self.holds(n)).collect()
    }

    /// The first matching node in the driver's order.
    pub fn first(&self) -> Option<NodeId> {
        self.collect().into_iter().next()
    }

    pub fn count(&self) -> usize {
        self.collect().len()
    }
}

impl Document {
    /// A query over every live node; chain predicates on it.
    pub fn query(&self) -> Query<'_> {
        Query { doc: self, kind: None, window: None, within: None, anchor: None, state: StateMask::NONE, props: Vec::new(), since: None }
    }

    /// The live nodes of `kind` (packed order).
    pub fn kinds_of(&self, kind: KindId) -> &[NodeId] {
        self.kind_sets.of(kind)
    }

    /// The nodes of `root`'s subtree (`root` included) that match the CSS selector list `css`, in
    /// tree order (the selector engine of the cascade; MCP and test queries).
    pub fn select(&self, root: NodeId, css: &str) -> Result<Vec<NodeId>, String> {
        let list = svelters_style::parse_selector_list(css, None)?;
        let Some(ri) = self.idx(root) else { return Ok(Vec::new()) };
        let mut caches = svelters_style::MatchCaches::new();
        let found = svelters_style::select(&list, crate::style::cascade::El { doc: self, i: ri }, &mut caches);
        Ok(found.into_iter().filter_map(|e| self.slots.id_at(e.i)).collect())
    }

    /// Whether `node` matches the CSS selector list `css`.
    pub fn matches(&self, node: NodeId, css: &str) -> Result<bool, String> {
        let list = svelters_style::parse_selector_list(css, None)?;
        let Some(i) = self.idx(node) else { return Ok(false) };
        let mut caches = svelters_style::MatchCaches::new();
        Ok(svelters_style::matches(&list, crate::style::cascade::El { doc: self, i }, &mut caches))
    }
}
