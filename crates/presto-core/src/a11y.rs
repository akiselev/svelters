//! Accessibility (architecture §13): the AccessKit projection of a window's tree, sent in the commit.
//! Node ids are presto's [`NodeId`]s. A kind fills its node through [`Kind::a11y`](crate::Kind::a11y);
//! the document adds bounds (window space), children and focus.
//!
//! A full tree goes out on a window's first frame and after structure changes; otherwise only the
//! nodes whose accessibility data changed. Every update passes [`validate`] before it leaves: AccessKit
//! panics on an invalid update (a focus or child id not in the tree, a node reachable from nowhere),
//! so the builder repairs what it can and drops the rest, with a debug assertion, never a panic in
//! release.

use crate::cx::A11yCx;
use crate::document::Document;
use accesskit::{Node, NodeId as AxId, TreeId, TreeInfo, TreeUpdate};
use presto_base::ids::{NodeId, WindowId};
use std::collections::{HashMap, HashSet};

fn ax(id: NodeId) -> AxId {
    AxId(id.0)
}

impl Document {
    /// The AccessKit node of node `i`.
    fn a11y_node(&mut self, i: usize) -> Node {
        let role = self.view_at(i).kind().spec.role;
        let mut out = Node::new(role);
        if let Some(k) = self.kinds[i].take() {
            {
                let mut cx = A11yCx { node: self.view_at(i), out: &mut out };
                k.a11y(&mut cx);
            }
            self.kinds[i] = Some(k);
        }
        let w = self.record[i].world;
        out.set_bounds(accesskit::Rect { x0: w.min.x as f64, y0: w.min.y as f64, x1: w.max.x as f64, y1: w.max.y as f64 });
        let hidden = self.layout[i].style.display == taffy::Display::None || self.style.computed(i).visibility_hidden;
        if hidden {
            out.set_hidden();
        }
        out.set_children(self.a11y_children(i).map(ax).collect::<Vec<_>>());
        if self.state[i].contains(svelters_style::StateMask::INVALID) {
            out.set_invalid(accesskit::Invalid::True);
        }
        // the anchor is the author id; the labels whose `for` names it label the node
        let node = self.slots.id_at(i).expect("live");
        if let Some(a) = self.anchors.get(&node) {
            out.set_author_id(a.to_string());
            let w = self.core[i].window;
            let mut labels: Vec<NodeId> = self.label_for.iter().filter(|(l, f)| *f == a && self.window_of(**l) == w).map(|(l, _)| *l).collect();
            if !labels.is_empty() {
                labels.sort_by_key(|l| l.0);
                out.set_labelled_by(labels.into_iter().map(ax).collect::<Vec<_>>());
            }
        }
        out
    }

    /// The children in the accessibility tree: all but those their container conceals.
    fn a11y_children(&self, i: usize) -> impl Iterator<Item = NodeId> + '_ {
        self.children[i].iter().copied().filter(|&c| !self.core[self.ix(c)].flags.contains(crate::tree::NodeFlags::CONCEALED))
    }

    /// The update for this frame (`None`: nothing changed).
    pub(crate) fn a11y_update(&mut self, window: WindowId, root: NodeId, full: bool, dirty: &[NodeId]) -> Option<TreeUpdate> {
        let first = self.window(window).is_some_and(|w| w.a11y_sent.is_empty());
        let removed = self.window(window).is_some_and(|w| w.a11y_removed);
        let full = full || first || removed;
        if !full && dirty.is_empty() {
            return None;
        }
        let mut nodes = Vec::new();
        let mut sent = HashSet::new();
        if full {
            let mut stack = vec![root];
            while let Some(n) = stack.pop() {
                let Some(i) = self.idx(n) else { continue };
                nodes.push((ax(n), self.a11y_node(i)));
                sent.insert(n);
                stack.extend(self.a11y_children(i));
            }
        } else {
            let known = self.window(window).map(|w| &w.a11y_sent);
            let dirty: Vec<NodeId> = dirty.iter().copied().filter(|n| known.is_some_and(|k| k.contains(n))).collect();
            // a node the last full tree did not have (concealed) waits for the next full tree
            for &n in &dirty {
                if let Some(i) = self.idx(n)
                    && self.core[i].window == Some(window)
                {
                    nodes.push((ax(n), self.a11y_node(i)));
                }
            }
        }
        let focus = self.focused(window).unwrap_or(root);
        let ws = self.window_mut(window).expect("window");
        ws.a11y_removed = false;
        let known: &HashSet<NodeId> = if full {
            ws.a11y_sent = sent;
            &ws.a11y_sent
        } else {
            &ws.a11y_sent
        };
        let update = TreeUpdate { nodes, tree: full.then(|| TreeInfo::new(ax(root))), tree_id: TreeId::ROOT, focus: ax(focus) };
        let known: HashSet<AxId> = known.iter().map(|&n| ax(n)).collect();
        Some(validate(update, ax(root), &known))
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
        let before = update.nodes.len();
        update.nodes.retain(|(id, _)| known.contains(id) || *id == root);
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
