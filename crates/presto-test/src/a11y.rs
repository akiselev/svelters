//! The AccessKit snapshot the harness queries (architecture §16.1): the tree folded from every applied
//! commit's `TreeUpdate`, with window-space bounds (each node's bounds through its ancestors'
//! transforms). Author ids (EM1) are the stable locators.

use presto_base::accesskit::{self, NodeId, Role, Toggled, TreeId, TreeUpdate};
use presto_base::geom::{Point, Rect};
use std::collections::HashMap;

/// One window's accessibility tree.
#[derive(Clone, Debug, Default)]
pub struct A11yTree {
    nodes: HashMap<NodeId, accesskit::Node>,
    root: Option<NodeId>,
    focus: Option<NodeId>,
    tree_id: Option<TreeId>,
}

impl A11yTree {
    /// Fold an update in: nodes replace their previous version; nodes no longer reachable from the
    /// root are gone (AccessKit removes a subtree by dropping it from its parent's children).
    pub fn apply(&mut self, u: &TreeUpdate) {
        if u.tree_id != TreeId::ROOT {
            // Subtrees (egui islands) graft under a node of this tree; the harness reads the root tree.
            return;
        }
        for (id, n) in &u.nodes {
            self.nodes.insert(*id, n.clone());
        }
        if let Some(t) = &u.tree {
            self.root = Some(t.root);
        }
        self.tree_id = Some(u.tree_id);
        self.focus = Some(u.focus);
        self.collect();
    }

    fn collect(&mut self) {
        let Some(root) = self.root else { return };
        let mut live = std::collections::HashSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !live.insert(id) {
                continue;
            }
            if let Some(n) = self.nodes.get(&id) {
                stack.extend(n.children().iter().copied());
            }
        }
        self.nodes.retain(|id, _| live.contains(id));
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }
    pub fn focus(&self) -> Option<NodeId> {
        self.focus
    }
    pub fn node(&self, id: NodeId) -> Option<&accesskit::Node> {
        self.nodes.get(&id)
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Every node in tree order (depth first, children in order), with its window-space bounds.
    pub fn walk(&self) -> Vec<NodeInfo<'_>> {
        let mut out = Vec::new();
        let Some(root) = self.root else { return out };
        let mut stack = vec![(root, accesskit::Affine::IDENTITY, None::<NodeId>)];
        while let Some((id, parent_xf, parent)) = stack.pop() {
            let Some(n) = self.nodes.get(&id) else {
                continue;
            };
            let xf = match n.transform() {
                Some(t) => parent_xf * *t,
                None => parent_xf,
            };
            let bounds = n.bounds().map(|b| {
                let r = xf.transform_rect_bbox(b);
                Rect::new(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32)
            });
            out.push(NodeInfo {
                id,
                node: n,
                bounds,
                parent,
            });
            for c in n.children().iter().rev() {
                stack.push((*c, xf, Some(id)));
            }
        }
        out
    }
}

/// A node with what the harness computed for it.
#[derive(Clone, Copy, Debug)]
pub struct NodeInfo<'a> {
    pub id: NodeId,
    pub node: &'a accesskit::Node,
    /// Window points.
    pub bounds: Option<Rect>,
    pub parent: Option<NodeId>,
}

impl NodeInfo<'_> {
    pub fn label(&self) -> Option<&str> {
        self.node.label()
    }
    pub fn value(&self) -> Option<&str> {
        self.node.value()
    }
    pub fn role(&self) -> Role {
        self.node.role()
    }
    pub fn author_id(&self) -> Option<&str> {
        self.node.author_id()
    }
    pub fn toggled(&self) -> Option<Toggled> {
        self.node.toggled()
    }
    pub fn is_disabled(&self) -> bool {
        self.node.is_disabled()
    }
    pub fn center(&self) -> Option<Point> {
        self.bounds
            .map(|b| Point::new((b.min.x + b.max.x) * 0.5, (b.min.y + b.max.y) * 0.5))
    }
}

/// A query over the snapshot (kittest's `By`).
#[derive(Clone, Debug, Default)]
pub struct By<'q> {
    label: Option<&'q str>,
    label_contains: Option<&'q str>,
    role: Option<Role>,
    value: Option<&'q str>,
    author_id: Option<&'q str>,
    include_hidden: bool,
}

impl<'q> By<'q> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn label(mut self, l: &'q str) -> Self {
        self.label = Some(l);
        self
    }
    pub fn label_contains(mut self, l: &'q str) -> Self {
        self.label_contains = Some(l);
        self
    }
    pub fn role(mut self, r: Role) -> Self {
        self.role = Some(r);
        self
    }
    pub fn value(mut self, v: &'q str) -> Self {
        self.value = Some(v);
        self
    }
    pub fn author_id(mut self, id: &'q str) -> Self {
        self.author_id = Some(id);
        self
    }
    /// Also match nodes marked hidden (excluded by default, as kittest does).
    pub fn include_hidden(mut self) -> Self {
        self.include_hidden = true;
        self
    }

    pub fn matches(&self, n: &NodeInfo<'_>) -> bool {
        (self.include_hidden || !n.node.is_hidden())
            && self.label.is_none_or(|l| n.label() == Some(l))
            && self
                .label_contains
                .is_none_or(|l| n.label().is_some_and(|x| x.contains(l)))
            && self.role.is_none_or(|r| n.role() == r)
            && self.value.is_none_or(|v| n.value() == Some(v))
            && self.author_id.is_none_or(|a| n.author_id() == Some(a))
    }
}

impl std::fmt::Display for By<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let Some(l) = self.label {
            parts.push(format!("label = {l:?}"));
        }
        if let Some(l) = self.label_contains {
            parts.push(format!("label contains {l:?}"));
        }
        if let Some(r) = self.role {
            parts.push(format!("role = {r:?}"));
        }
        if let Some(v) = self.value {
            parts.push(format!("value = {v:?}"));
        }
        if let Some(a) = self.author_id {
            parts.push(format!("author id = {a:?}"));
        }
        write!(f, "{}", parts.join(", "))
    }
}
