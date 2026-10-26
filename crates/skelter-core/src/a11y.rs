//! Accessibility (architecture §13): the AccessKit projection of a window's tree, sent in the commit.
//! Node ids are skelter's [`NodeId`]s. A kind fills its node through [`Kind::a11y`](crate::Kind::a11y);
//! the document adds bounds (window space), children and focus.
//!
//! A full tree goes out on a window's first frame; afterwards only the nodes whose accessibility data
//! changed, the ones that moved relative to their parent (each node carries its local box and its
//! transform to its parent), and subtrees that joined the tree. Every update passes [`validate`] before it leaves: AccessKit
//! panics on an invalid update (a focus or child id not in the tree, a node reachable from nowhere),
//! so the builder repairs what it can and drops the rest, with a debug assertion, never a panic in
//! release.
//!
//! A kind can add **virtual children** in [`Kind::a11y`](crate::Kind::a11y) through
//! [`A11yCx::add_virtual_child`]: AccessKit nodes for parts it draws without nodes of their own (a
//! select's options). They go out with their owner every time it is sent, after its real children,
//! under ids in a range node ids never reach ([`virtual_id`]); an action on one reaches the owner's
//! kind as [`Event::A11yChild`](crate::Event::A11yChild) with the child's index.
//!
//! **Focus** (QA11Y F3): `TreeUpdate.focus` is the window's [`FocusTarget`] resolved to an id the
//! tree has: the focused node, or its virtual child `sub`; a graft node (an egui island's, a node
//! carrying a `tree_id`) only once its subtree has gone out ([`Document::a11y_subtree_sent`]), since
//! AccessKit's consumer panics on a focused graft without its subtree; a node the tree doesn't have
//! (concealed, a graft waiting for its subtree) names its nearest ancestor that it has. The root only
//! when nothing is focused.
//!
//! **Bounds** are in physical window pixels, as AccessKit wants them: the root's transform is
//! `scale(window.scale)` (OS scale × UI zoom) over its world transform, and every other node is
//! relative to its parent, so a scale change re-sends the root only.

use crate::cx::A11yCx;
use crate::document::Document;
use accesskit::{Node, NodeId as AxId, TreeId, TreeInfo, TreeUpdate};
use skelter_base::ids::{NodeId, WindowId};
use std::collections::{HashMap, HashSet};

fn ax(id: NodeId) -> AxId {
    AxId(id.0)
}

/// The bit that marks a virtual child's id. Node ids stay below 2^52 (`generation << 24 | index`
/// with generations below 2^28) and fixed-part list keys use bit 63, so ids with bit 62 set are
/// never a node's.
pub const VIRTUAL_BIT: u64 = 1 << 62;
/// Bits of the child index in a virtual id: a kind has at most this many virtual children
/// (`2^10`); further ones are dropped.
pub const VIRTUAL_INDEX_BITS: u32 = 10;
/// The most virtual children one node has.
pub const MAX_VIRTUAL_CHILDREN: usize = 1 << VIRTUAL_INDEX_BITS;

/// The AccessKit id of `owner`'s virtual child `index`: `1 << 62 | owner << 10 | index`.
pub const fn virtual_id(owner: NodeId, index: u32) -> AxId {
    AxId(VIRTUAL_BIT | owner.0 << VIRTUAL_INDEX_BITS | index as u64)
}

/// The owner and index of a virtual child's id; `None` for any other id.
pub const fn virtual_child(id: AxId) -> Option<(NodeId, u32)> {
    if id.0 & VIRTUAL_BIT == 0 || id.0 & (1 << 63) != 0 {
        return None;
    }
    let rest = id.0 & !VIRTUAL_BIT;
    Some((NodeId(rest >> VIRTUAL_INDEX_BITS), (rest & (MAX_VIRTUAL_CHILDREN as u64 - 1)) as u32))
}

impl Document {
    /// The AccessKit node of node `i`: the kind's, with its box in its own space and its transform
    /// relative to its parent (so a move re-sends only the node that moved), and its children; then
    /// the virtual children the kind added, with their ids.
    fn a11y_node(&mut self, i: usize) -> (Node, Vec<(AxId, Node)>) {
        let role = self.view_at(i).kind().spec.role;
        let mut out = Node::new(role);
        let mut virtual_children = Vec::new();
        if let Some(k) = self.kinds[i].take() {
            {
                let mut cx = A11yCx { node: self.view_at(i), out: &mut out, virtual_children: &mut virtual_children };
                k.a11y(&mut cx);
            }
            self.kinds[i] = Some(k);
        }
        // a realised row of a virtualising parent: what it is as the parent's member (a list box's
        // option), under its own props
        if self.core[i].flags.contains(crate::tree::NodeFlags::ROW)
            && let Some(p) = self.core[i].parent
        {
            let id = self.slots.id_at(i).expect("live");
            let row = self.virt.get(&p).and_then(|st| st.rows.iter().find(|&&(_, n)| n == id)).map(|&(r, _)| r);
            if let (Some(row), Some(pi)) = (row, self.idx(p))
                && let Some(mut k) = self.kinds[pi].take()
            {
                if let Some(vc) = k.virtual_children() {
                    vc.row_a11y(row, &mut out);
                }
                self.kinds[pi] = Some(k);
            }
        }
        // common props a behaviour may set win over the kind's own node
        {
            let v = self.view_at(i);
            match v.prop(crate::common::ARIA_LIVE).and_then(|v| v.as_enum()) {
                Some(1) => out.set_live(accesskit::Live::Polite),
                Some(2) => out.set_live(accesskit::Live::Assertive),
                _ => {}
            }
            if v.prop_bool(crate::common::ARIA_BUSY) {
                out.set_busy();
            }
            if let Some(role) = v.prop_str(crate::common::ROLE).and_then(crate::common::role_named) {
                out.set_role(role);
            }
            if let Some(level) = v.prop(crate::common::ARIA_LEVEL).and_then(|v| v.as_i64()).filter(|l| *l > 0) {
                out.set_level(level as usize);
            }
            match v.prop(crate::common::ARIA_EXPANDED).and_then(|v| v.as_enum()) {
                Some(1) => out.set_expanded(true),
                Some(2) => out.set_expanded(false),
                _ => {}
            }
            match v.prop(crate::common::ARIA_SELECTED).and_then(|v| v.as_enum()) {
                Some(1) => out.set_selected(true),
                Some(2) => out.set_selected(false),
                _ => {}
            }
            if let Some(d) = v.prop_str(crate::common::ARIA_DESCRIPTION).filter(|d| !d.is_empty()) {
                out.set_description(d);
            }
            match v.prop(crate::common::ARIA_CHECKED).and_then(|v| v.as_enum()) {
                Some(1) => out.set_toggled(accesskit::Toggled::True),
                Some(2) => out.set_toggled(accesskit::Toggled::False),
                Some(3) => out.set_toggled(accesskit::Toggled::Mixed),
                _ => {}
            }
            match v.prop(crate::common::ARIA_HASPOPUP).and_then(|v| v.as_enum()) {
                Some(1 | 2) => out.set_has_popup(accesskit::HasPopup::Menu),
                Some(3) => out.set_has_popup(accesskit::HasPopup::Listbox),
                Some(4) => out.set_has_popup(accesskit::HasPopup::Tree),
                Some(5) => out.set_has_popup(accesskit::HasPopup::Grid),
                Some(6) => out.set_has_popup(accesskit::HasPopup::Dialog),
                _ => {}
            }
            if v.prop_bool(crate::common::ARIA_MODAL) {
                out.set_modal();
            }
            // ARIA's precedence: `aria-label` names the node over its text content, whatever the
            // kind set from its content (a button labelled "Toggle problems" that shows "3", A30)
            if let Some(l) = v.prop_str(crate::common::ARIA_LABEL).filter(|l| !l.trim().is_empty()) {
                out.set_label(l);
            }
        }
        let node = self.slots.id_at(i).expect("live");
        let window = self.core[i].window;
        let graft = out.tree_id().is_some();
        if let Some(w) = window.and_then(|w| self.window_mut(w)) {
            if graft {
                w.a11y_grafts.insert(node);
            } else if w.a11y_grafts.remove(&node) {
                w.a11y_subtrees_sent.remove(&node);
            }
        }
        let l = &self.layout[i].layout;
        out.set_bounds(accesskit::Rect { x0: 0.0, y0: 0.0, x1: l.size.width as f64, y1: l.size.height as f64 });
        let world = self.record[i].world_tf;
        let rel = match self.core[i].parent {
            Some(p) => self.record[self.ix(p)].world_tf.inverse().map_or(skelter_base::geom::Affine::IDENTITY, |pi| pi.then(&world)),
            // the root scales document points to physical window pixels
            None => {
                let s = window.and_then(|w| self.window(w)).map_or(1.0, |w| w.scale.0);
                skelter_base::geom::Affine::scale(s, s).then(&world)
            }
        };
        if rel != skelter_base::geom::Affine::IDENTITY {
            out.set_transform(accesskit::Affine::new([rel.a as f64, rel.b as f64, rel.c as f64, rel.d as f64, rel.e as f64, rel.f as f64]));
        }
        self.record[i].a11y_tf = Some(rel);
        let hidden = self.layout[i].style.display == taffy::Display::None || self.style.computed(i).visibility_hidden;
        if hidden {
            out.set_hidden();
        }
        let kids: Vec<NodeId> = self.a11y_children(i).collect();
        // nested virtual children (a row's cells) are their parent's children, not the node's
        let mut top = Vec::new();
        let mut nested: Vec<Vec<AxId>> = vec![Vec::new(); virtual_children.len()];
        for (k, (_, parent)) in virtual_children.iter().enumerate() {
            match parent {
                Some(p) => nested[*p as usize].push(virtual_id(node, k as u32)),
                None => top.push(virtual_id(node, k as u32)),
            }
        }
        let virtual_children: Vec<(AxId, Node)> = virtual_children
            .into_iter()
            .zip(nested)
            .enumerate()
            .map(|(k, ((mut n, _), kids))| {
                if !kids.is_empty() {
                    n.set_children(kids);
                }
                (virtual_id(node, k as u32), n)
            })
            .collect();
        if let Some(w) = window.and_then(|w| self.window_mut(w)) {
            if virtual_children.is_empty() {
                w.a11y_virtual.remove(&node);
            } else {
                w.a11y_virtual.insert(node, virtual_children.len() as u32);
            }
        }
        out.set_children(kids.iter().map(|&c| ax(c)).chain(top).collect::<Vec<_>>());
        if self.state[i].contains(svelters_style::StateMask::INVALID) {
            out.set_invalid(accesskit::Invalid::True);
        }
        // the anchor is the author id; the labels whose `for` names it label the node
        if let Some(a) = self.anchors.get(&node) {
            out.set_author_id(a.to_string());
            let w = self.core[i].window;
            let mut labels: Vec<NodeId> = self.label_for.iter().filter(|(l, f)| *f == a && self.window_of(**l) == w).map(|(l, _)| *l).collect();
            if !labels.is_empty() {
                labels.sort_by_key(|l| l.0);
                out.set_labelled_by(labels.into_iter().map(ax).collect::<Vec<_>>());
            }
        }
        // references by anchor (validated against the tree before the update leaves)
        if let Some(w) = window {
            let v = self.view_at(i);
            let ad = v.prop_str(crate::common::ARIA_ACTIVEDESCENDANT).filter(|a| !a.is_empty()).map(str::to_owned);
            let controls = v.prop_str(crate::common::ARIA_CONTROLS).filter(|a| !a.trim().is_empty()).map(str::to_owned);
            if let Some(t) = ad.and_then(|a| self.find_anchor(w, &a)) {
                out.set_active_descendant(ax(t));
            }
            if let Some(c) = controls {
                let ids: Vec<AxId> = c.split_whitespace().filter_map(|a| self.find_anchor(w, a)).map(ax).collect();
                if !ids.is_empty() {
                    out.set_controls(ids);
                }
            }
        }
        self.record[i].a11y_kids = kids;
        (out, virtual_children)
    }

    /// The children in the accessibility tree: all but those their container conceals.
    fn a11y_children(&self, i: usize) -> impl Iterator<Item = NodeId> + '_ {
        let kids: &[NodeId] = if self.view_at(i).kind().spec.conceals_children { &[] } else { &self.children[i] };
        kids.iter().copied().filter(|&c| !self.core[self.ix(c)].flags.contains(crate::tree::NodeFlags::CONCEALED))
    }

    /// The update for this frame (`None`: nothing changed). The whole tree goes out on the window's
    /// first frame; afterwards the nodes in `dirty` that the tree has, plus every node that joins
    /// it under them (a new or shown subtree). Nodes that leave it (removed, concealed) are dropped
    /// from the known set with their subtrees.
    pub(crate) fn a11y_update(&mut self, window: WindowId, root: NodeId, dirty: &[NodeId]) -> Option<TreeUpdate> {
        let first = self.window(window).is_some_and(|w| w.a11y_sent.is_empty());
        if !first && dirty.is_empty() {
            return None;
        }
        let mut known = self.window_mut(window).map(|w| std::mem::take(&mut w.a11y_sent)).unwrap_or_default();
        let before = known.clone();
        let mut nodes = Vec::new();
        let mut sent: HashSet<NodeId> = HashSet::new();
        let mut queue: Vec<NodeId> = if first { vec![root] } else { dirty.iter().copied().filter(|n| known.contains(n)).collect() };
        while let Some(n) = queue.pop() {
            let Some(i) = self.idx(n) else { continue };
            if self.core[i].window != Some(window) || !sent.insert(n) {
                continue;
            }
            let old_kids = std::mem::take(&mut self.record[i].a11y_kids);
            let (node, virtual_children) = self.a11y_node(i);
            nodes.push((ax(n), node));
            // virtual children go out with their owner (and leave with it): never in the known set
            nodes.extend(virtual_children);
            known.insert(n);
            // children that joined the tree come with this node
            for &c in &self.record[i].a11y_kids {
                if !known.contains(&c) {
                    queue.push(c);
                }
            }
            // a virtualising node's realised rows carry its state (`selected`, positions): with it
            if let Some(st) = self.virt.get(&n) {
                queue.extend(st.rows.iter().map(|&(_, r)| r).filter(|r| known.contains(r)));
            }
            // children that left it take their subtrees out of the known set
            for c in old_kids.into_iter().filter(|c| !self.record[i].a11y_kids.contains(c)) {
                let mut stack = vec![c];
                while let Some(g) = stack.pop() {
                    // a node moved under a parent sent earlier in this update stays
                    if !sent.contains(&g) && known.remove(&g) {
                        stack.extend(self.children(g).iter().copied());
                    }
                }
            }
        }
        if nodes.is_empty() {
            if let Some(w) = self.window_mut(window) {
                w.a11y_sent = known;
            }
            return None;
        }
        let focus = self.a11y_focus(window, root, &known);
        if let Some(w) = self.window_mut(window) {
            // nodes that left the tree take their graft and virtual-child state with them
            w.a11y_grafts.retain(|n| known.contains(n));
            w.a11y_subtrees_sent.retain(|n| known.contains(n));
            w.a11y_virtual.retain(|n, _| known.contains(n));
            w.a11y_sent = known;
        }
        let update = TreeUpdate { nodes, tree: first.then(|| TreeInfo::new(ax(root))), tree_id: TreeId::ROOT, focus };
        let prev: HashSet<AxId> = if first {
            HashSet::new()
        } else {
            // the known nodes and the virtual children they last went out with
            let virt = self.window(window).map(|w| &w.a11y_virtual);
            before
                .iter()
                .flat_map(|&n| {
                    let k = virt.and_then(|v| v.get(&n)).copied().unwrap_or(0);
                    std::iter::once(ax(n)).chain((0..k).map(move |c| virtual_id(n, c)))
                })
                .collect()
        };
        Some(validate(update, ax(root), &prev))
    }

    /// `TreeUpdate.focus` (module docs): the focus target resolved to an id the tree has.
    fn a11y_focus(&self, window: WindowId, root: NodeId, known: &HashSet<NodeId>) -> AxId {
        let Some(w) = self.window(window) else { return ax(root) };
        let Some(t) = self.focus_target(window) else { return ax(root) };
        let shown = |n: NodeId| known.contains(&n) && (!w.a11y_grafts.contains(&n) || w.a11y_subtrees_sent.contains(&n));
        if shown(t.node) {
            if !w.a11y_grafts.contains(&t.node)
                && let Some(sub) = t.sub
                && w.a11y_virtual.get(&t.node).is_some_and(|&k| sub < k)
            {
                return virtual_id(t.node, sub);
            }
            return ax(t.node);
        }
        // the nearest ancestor the tree has (a graft's subtree isn't out yet; the node is concealed)
        let mut a = self.parent(t.node);
        while let Some(p) = a {
            if shown(p) {
                return ax(p);
            }
            a = self.parent(p);
        }
        ax(root)
    }

    /// The subtree grafted at `node` (an egui island's: its AccessKit node carries a `tree_id`) went
    /// out after the host update that carries `node`: from the next frame, focus on `node` reaches
    /// AccessKit as the graft (followed into the subtree's own focus) instead of its parent. The
    /// document that sends the subtrees (`Commit::a11y_subtrees`) calls it once the commit carries one.
    pub fn a11y_subtree_sent(&mut self, node: NodeId) {
        let Some(window) = self.window_of(node) else { return };
        let focused = self.focused(window) == Some(node);
        let Some(w) = self.window_mut(window) else { return };
        if !w.a11y_sent.contains(&node) || !w.a11y_subtrees_sent.insert(node) {
            return;
        }
        if focused {
            self.mark(node, crate::tree::Dirty::A11Y);
        }
    }
}

/// Makes `update` valid for a tree whose previous nodes are `known` (empty when the update carries the
/// whole tree): children that exist nowhere are dropped, nodes not reachable from the root through
/// the update or the known tree are dropped, duplicates keep the last, and a focus outside the tree
/// moves to the root.
pub fn validate(mut update: TreeUpdate, root: AxId, known: &HashSet<AxId>) -> TreeUpdate {
    let mut by_id: HashMap<AxId, usize> = HashMap::new();
    let mut dup = false;
    for (k, (id, _)) in update.nodes.iter().enumerate() {
        dup |= by_id.insert(*id, k).is_some();
    }
    if dup {
        debug_assert!(false, "a11y: duplicate node ids in one update");
        let keep: HashSet<usize> = by_id.values().copied().collect();
        let mut k = 0;
        update.nodes.retain(|_| {
            k += 1;
            keep.contains(&(k - 1))
        });
        by_id = update.nodes.iter().enumerate().map(|(k, (id, _))| (*id, k)).collect();
    }
    let exists = |id: &AxId| by_id.contains_key(id) || known.contains(id);
    // children that exist nowhere
    let mut repaired = false;
    for (_, node) in update.nodes.iter_mut() {
        let kids = node.children().to_vec();
        if kids.iter().any(|c| !exists(c)) {
            repaired = true;
            node.set_children(kids.into_iter().filter(|c| exists(c)).collect::<Vec<_>>());
        }
    }
    debug_assert!(!repaired, "a11y: an update named children that are in no tree");
    // ARIA references (by anchor) may name a node the tree doesn't have (hidden, concealed):
    // accesskit_consumer unwraps `controls`, so they go
    for (_, node) in update.nodes.iter_mut() {
        if node.active_descendant().is_some_and(|a| !exists(&a)) {
            node.clear_active_descendant();
        }
        let controls = node.controls().to_vec();
        if controls.iter().any(|c| !exists(c)) {
            let keep: Vec<AxId> = controls.into_iter().filter(|c| exists(c)).collect();
            if keep.is_empty() {
                node.clear_controls();
            } else {
                node.set_controls(keep);
            }
        }
    }
    // reachability: from the root through updated children, or a known node whose parent is unchanged
    if update.tree.is_some() || !known.contains(&root) {
        let mut reach = HashSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !reach.insert(id) {
                continue;
            }
            if let Some(&k) = by_id.get(&id) {
                stack.extend(update.nodes[k].1.children().iter().copied());
            }
        }
        let before = update.nodes.len();
        update.nodes.retain(|(id, _)| reach.contains(id));
        debug_assert_eq!(before, update.nodes.len(), "a11y: unreachable nodes in a full update");
        if update.tree.is_none() {
            update.tree = Some(TreeInfo::new(root));
        }
    } else {
        // a new node must hang under a known node the update re-sends
        let mut reach = HashSet::new();
        let mut stack: Vec<AxId> = update.nodes.iter().map(|(id, _)| *id).filter(|id| known.contains(id)).collect();
        while let Some(id) = stack.pop() {
            if !reach.insert(id) {
                continue;
            }
            if let Some(&k) = by_id.get(&id) {
                stack.extend(update.nodes[k].1.children().iter().copied());
            }
        }
        let before = update.nodes.len();
        update.nodes.retain(|(id, _)| reach.contains(id) || *id == root);
        debug_assert_eq!(before, update.nodes.len(), "a11y: a new node in an incremental update without its parent");
    }
    let present = |id: &AxId| update.nodes.iter().any(|(n, _)| n == id) || known.contains(id);
    if !present(&update.focus) {
        debug_assert!(false, "a11y: focus outside the tree");
        update.focus = root;
    }
    update
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::Role;

    fn node(children: &[u64]) -> Node {
        let mut n = Node::new(Role::GenericContainer);
        n.set_children(children.iter().map(|&c| AxId(c)).collect::<Vec<_>>());
        n
    }

    #[test]
    fn a_valid_full_update_passes_unchanged() {
        let u = TreeUpdate { nodes: vec![(AxId(1), node(&[2])), (AxId(2), node(&[]))], tree: Some(TreeInfo::new(AxId(1))), tree_id: TreeId::ROOT, focus: AxId(2) };
        let v = validate(u.clone(), AxId(1), &HashSet::new());
        assert_eq!(v, u);
    }

    #[test]
    fn incremental_updates_keep_known_nodes() {
        let known: HashSet<AxId> = [AxId(1), AxId(2)].into_iter().collect();
        let u = TreeUpdate { nodes: vec![(AxId(2), node(&[]))], tree: None, tree_id: TreeId::ROOT, focus: AxId(1) };
        let v = validate(u.clone(), AxId(1), &known);
        assert_eq!(v, u);
    }
}

/// Repairs happen in release builds without panicking (the debug assertions above are off there).
#[cfg(all(test, not(debug_assertions)))]
mod release_tests {
    use super::*;
    use accesskit::Role;

    #[test]
    fn invalid_updates_are_repaired() {
        let mut parent = Node::new(Role::GenericContainer);
        parent.set_children(vec![AxId(2), AxId(9)]);
        let u = TreeUpdate { nodes: vec![(AxId(1), parent), (AxId(2), Node::new(Role::Button)), (AxId(5), Node::new(Role::Label))], tree: Some(TreeInfo::new(AxId(1))), tree_id: TreeId::ROOT, focus: AxId(7) };
        let v = validate(u, AxId(1), &HashSet::new());
        assert_eq!(v.nodes.len(), 2);
        assert_eq!(v.nodes[0].1.children(), &[AxId(2)]);
        assert_eq!(v.focus, AxId(1));
    }
}
