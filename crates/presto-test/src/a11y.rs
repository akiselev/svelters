//! The AccessKit snapshot the harness queries (architecture §16.1): every tree the app sent, folded
//! from its `TreeUpdate`s, walked as one tree. A node carrying a `tree_id` is a graft point (an egui
//! island, `presto_egui_island::A11yGraft`): the walk descends into that subtree's root under it, so
//! queries reach island widgets as kittest reaches egui's. Bounds are window points, through every
//! ancestor's transform, the graft node's and the subtree root's placement included. Author ids (EM1)
//! are the stable locators.
//!
//! [`By`] has kittest's semantics: a label query matches a `Label` node by its value (its text), drops
//! the label nodes that label another match (testing-library's `getByLabelText`), and hidden nodes match.

use presto_base::accesskit::{self, NodeId, Role, Toggled, TreeId, TreeUpdate};
use presto_base::geom::{Point, Rect};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// One tree's nodes (the root tree, or a grafted subtree).
#[derive(Clone, Debug, Default)]
struct Sub {
    nodes: HashMap<NodeId, accesskit::Node>,
    root: Option<NodeId>,
    focus: Option<NodeId>,
}

impl Sub {
    /// Drop nodes no longer reachable from the root (AccessKit removes a subtree by dropping it from
    /// its parent's children).
    fn collect(&mut self) {
        let Some(root) = self.root else { return };
        let mut live = HashSet::new();
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
}

/// A node's address: its tree and its id in that tree.
pub type NodeKey = (TreeId, NodeId);

/// One window's accessibility trees.
#[derive(Clone, Debug, Default)]
pub struct A11yTree {
    trees: HashMap<TreeId, Sub>,
}

impl A11yTree {
    /// Fold an update in: nodes replace their previous version; unreachable nodes are dropped. A
    /// subtree's update (non-`ROOT` `tree_id`) is kept under its id and shows where a node grafts it.
    pub fn apply(&mut self, u: &TreeUpdate) {
        let sub = self.trees.entry(u.tree_id).or_default();
        for (id, n) in &u.nodes {
            sub.nodes.insert(*id, n.clone());
        }
        if let Some(t) = &u.tree {
            sub.root = Some(t.root);
        }
        sub.focus = Some(u.focus);
        sub.collect();
    }

    /// Forget a subtree (its island closed).
    pub fn remove_tree(&mut self, tree: TreeId) {
        self.trees.remove(&tree);
    }

    pub fn root(&self) -> Option<NodeId> {
        self.trees.get(&TreeId::ROOT).and_then(|s| s.root)
    }

    pub fn root_key(&self) -> Option<NodeKey> {
        self.root().map(|r| (TreeId::ROOT, r))
    }

    /// The focused node: the root tree's focus, followed into a grafted subtree when the focus is on
    /// its graft node (AccessKit's rule for nested trees).
    pub fn focus(&self) -> Option<NodeKey> {
        let mut tree = TreeId::ROOT;
        let mut seen = HashSet::new();
        loop {
            let sub = self.trees.get(&tree)?;
            let f = sub.focus?;
            let into = sub.nodes.get(&f).and_then(|n| n.tree_id());
            match into {
                Some(t) if seen.insert(t) && self.trees.get(&t).is_some_and(|s| s.focus.is_some()) => {
                    tree = t
                }
                _ => return Some((tree, f)),
            }
        }
    }

    pub fn node(&self, key: NodeKey) -> Option<&accesskit::Node> {
        self.trees.get(&key.0).and_then(|s| s.nodes.get(&key.1))
    }

    /// The trees the app sent (the root tree and every subtree).
    pub fn tree_ids(&self) -> impl Iterator<Item = TreeId> + '_ {
        self.trees.keys().copied()
    }

    /// Nodes in every tree, grafted or not.
    pub fn len(&self) -> usize {
        self.trees.values().map(|s| s.nodes.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every node reachable from the root tree's root, in tree order (depth first, children in order,
    /// a graft node's subtree after its own children), with window-space bounds.
    pub fn walk(&self) -> Vec<NodeInfo<'_>> {
        let mut out = Vec::new();
        let Some(root) = self.root() else { return out };
        let mut stack = vec![(TreeId::ROOT, root, accesskit::Affine::IDENTITY, None::<NodeKey>, 0usize)];
        let mut visited = HashSet::new();
        while let Some((tree, id, parent_xf, parent, depth)) = stack.pop() {
            if !visited.insert((tree, id)) {
                continue;
            }
            let Some(n) = self.node((tree, id)) else { continue };
            let xf = match n.transform() {
                Some(t) => parent_xf * *t,
                None => parent_xf,
            };
            let bounds = n.bounds().map(|b| {
                let r = xf.transform_rect_bbox(b);
                Rect::new(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32)
            });
            out.push(NodeInfo {
                tree,
                id,
                node: n,
                bounds,
                parent,
                depth,
            });
            if let Some(sub_root) = n.tree_id().and_then(|t| self.trees.get(&t).and_then(|s| s.root.map(|r| (t, r)))) {
                stack.push((sub_root.0, sub_root.1, xf, Some((tree, id)), depth + 1));
            }
            for c in n.children().iter().rev() {
                stack.push((tree, *c, xf, Some((tree, id)), depth + 1));
            }
        }
        out
    }

    /// The nodes below `scope` (`None`: the root) that `by` matches, in tree order, with kittest's
    /// label rule applied. The scope node itself never matches.
    pub fn query(&self, scope: Option<NodeKey>, by: &By<'_>) -> Vec<NodeInfo<'_>> {
        let walk = self.walk();
        let Some(scope) = scope.or_else(|| self.root_key()) else {
            return Vec::new();
        };
        let Some(start) = walk.iter().position(|n| n.key() == scope) else {
            return Vec::new();
        };
        let base = walk[start].depth;
        let found: Vec<NodeInfo<'_>> = walk[start + 1..]
            .iter()
            .take_while(|n| n.depth > base)
            .filter(|n| by.recursive || n.depth == base + 1)
            .filter(|n| by.matches(n, self))
            .copied()
            .collect();
        if by.label.is_none() || by.include_labels {
            return found;
        }
        let labels: HashSet<NodeKey> = found
            .iter()
            .flat_map(|n| n.node.labelled_by().iter().map(move |l| (n.tree, *l)))
            .collect();
        found.into_iter().filter(|n| !labels.contains(&n.key())).collect()
    }
}

/// A node with what the harness computed for it.
#[derive(Clone, Copy, Debug)]
pub struct NodeInfo<'a> {
    pub tree: TreeId,
    pub id: NodeId,
    pub node: &'a accesskit::Node,
    /// Window points.
    pub bounds: Option<Rect>,
    pub parent: Option<NodeKey>,
    /// Depth below the root tree's root (a subtree's root is one below its graft node).
    pub depth: usize,
}

impl NodeInfo<'_> {
    pub fn key(&self) -> NodeKey {
        (self.tree, self.id)
    }
    pub fn label(&self) -> Option<&str> {
        self.node.label()
    }
    /// The text kittest matches labels against: a `Label` node's value (its text), else its label.
    pub fn match_label(&self) -> Option<&str> {
        if self.node.role() == Role::Label {
            self.node.value().or_else(|| self.node.label())
        } else {
            self.node.label()
        }
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
    pub fn is_hidden(&self) -> bool {
        self.node.is_hidden()
    }
    pub fn center(&self) -> Option<Point> {
        self.bounds
            .map(|b| Point::new((b.min.x + b.max.x) * 0.5, (b.min.y + b.max.y) * 0.5))
    }
}

/// A predicate over a node, as kittest's `By::predicate` takes (the node as `accesskit_node()` shows it).
pub type Predicate<'q> = Arc<dyn Fn(&AkNode<'_>) -> bool + 'q>;

/// A query over the snapshot: kittest's `By` (with `kittest::by()` as [`by`]), plus author ids.
#[derive(Clone)]
pub struct By<'q> {
    label: Option<&'q str>,
    label_contains: bool,
    include_labels: bool,
    predicate: Option<Predicate<'q>>,
    role: Option<Role>,
    value: Option<&'q str>,
    author_id: Option<&'q str>,
    exclude_hidden: bool,
    recursive: bool,
}

impl Default for By<'_> {
    fn default() -> Self {
        By {
            label: None,
            label_contains: false,
            include_labels: false,
            predicate: None,
            role: None,
            value: None,
            author_id: None,
            exclude_hidden: false,
            recursive: true,
        }
    }
}

/// kittest's `by()`.
pub fn by<'q>() -> By<'q> {
    By::new()
}

impl<'q> By<'q> {
    pub fn new() -> Self {
        Self::default()
    }
    /// The accessible name equals `l` (for a `Label` node: its text).
    pub fn label(mut self, l: &'q str) -> Self {
        self.label = Some(l);
        self.label_contains = false;
        self
    }
    pub fn label_contains(mut self, l: &'q str) -> Self {
        self.label = Some(l);
        self.label_contains = true;
        self
    }
    /// Keep the label nodes that label another match (dropped by default).
    pub fn include_labels(mut self) -> Self {
        self.include_labels = true;
        self
    }
    pub fn predicate(mut self, p: impl Fn(&AkNode<'_>) -> bool + 'q) -> Self {
        self.predicate = Some(Arc::new(p));
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
    /// Leave out nodes marked hidden (kittest matches them; so does this by default).
    pub fn exclude_hidden(mut self) -> Self {
        self.exclude_hidden = true;
        self
    }
    /// `false`: only the scope's direct children (default `true`: every descendant).
    pub fn recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Whether `n` (a node of `tree`) matches.
    pub fn matches(&self, n: &NodeInfo<'_>, tree: &A11yTree) -> bool {
        if self.exclude_hidden && n.is_hidden() {
            return false;
        }
        if let Some(l) = self.label {
            // kittest: a `Label` node by its text, any other node by its accessible name.
            let name = if n.role() == Role::Label {
                n.match_label().map(str::to_owned)
            } else {
                AkNode { info: *n, tree }.label()
            };
            match name {
                Some(x) if self.label_contains && x.contains(l) => {}
                Some(x) if !self.label_contains && x == l => {}
                _ => return false,
            }
        }
        if let Some(p) = &self.predicate
            && !p(&AkNode { info: *n, tree })
        {
            return false;
        }
        self.role.is_none_or(|r| n.role() == r)
            && self.value.is_none_or(|v| n.value() == Some(v))
            && self.author_id.is_none_or(|a| n.author_id() == Some(a))
    }
}

impl std::fmt::Display for By<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let Some(l) = self.label {
            if self.label_contains {
                parts.push(format!("label contains {l:?}"));
            } else {
                parts.push(format!("label = {l:?}"));
            }
        }
        if self.include_labels {
            parts.push("including label nodes".into());
        }
        if self.predicate.is_some() {
            parts.push("a predicate".into());
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
        if self.exclude_hidden {
            parts.push("not hidden".into());
        }
        if !self.recursive {
            parts.push("direct children only".into());
        }
        write!(f, "{}", parts.join(", "))
    }
}

impl std::fmt::Debug for By<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "By({self})")
    }
}

/// A node as kittest's `accesskit_node()` shows it (accesskit_consumer's `Node` methods tests call):
/// computed names, window-space bounds, focus.
#[derive(Clone, Copy)]
pub struct AkNode<'a> {
    pub(crate) info: NodeInfo<'a>,
    pub(crate) tree: &'a A11yTree,
}

impl<'a> AkNode<'a> {
    pub fn id(&self) -> NodeId {
        self.info.id
    }
    /// The node's id and tree (kittest's `locate`).
    pub fn locate(&self) -> (NodeId, TreeId) {
        (self.info.id, self.info.tree)
    }
    pub fn role(&self) -> Role {
        self.info.role()
    }
    /// The node's own data.
    pub fn data(&self) -> &'a accesskit::Node {
        self.info.node
    }
    /// The accessible name: the node's label, else the names of the nodes labelling it.
    pub fn label(&self) -> Option<String> {
        if let Some(l) = self.info.node.label() {
            return Some(l.to_owned());
        }
        let names: Vec<String> = self
            .info
            .node
            .labelled_by()
            .iter()
            .filter_map(|id| self.tree.node((self.info.tree, *id)))
            .filter_map(|n| n.value().or_else(|| n.label()).map(str::to_owned))
            .collect();
        (!names.is_empty()).then(|| names.join(" "))
    }
    pub fn value(&self) -> Option<String> {
        self.info.node.value().map(str::to_owned)
    }
    pub fn description(&self) -> Option<String> {
        self.info.node.description().map(str::to_owned)
    }
    pub fn author_id(&self) -> Option<&'a str> {
        self.info.node.author_id()
    }
    pub fn numeric_value(&self) -> Option<f64> {
        self.info.node.numeric_value()
    }
    pub fn toggled(&self) -> Option<Toggled> {
        self.info.node.toggled()
    }
    pub fn is_disabled(&self) -> bool {
        self.info.node.is_disabled()
    }
    pub fn is_hidden(&self) -> bool {
        self.info.node.is_hidden()
    }
    pub fn is_selected(&self) -> Option<bool> {
        self.info.node.is_selected()
    }
    pub fn is_focused(&self) -> bool {
        self.tree.focus() == Some(self.info.key())
    }
    /// Window-space bounds (points).
    pub fn bounding_box(&self) -> Option<accesskit::Rect> {
        self.info.bounds.map(|b| accesskit::Rect {
            x0: f64::from(b.min.x),
            y0: f64::from(b.min.y),
            x1: f64::from(b.max.x),
            y1: f64::from(b.max.y),
        })
    }
    /// The bounds as the node declares them (its own coordinate space).
    pub fn raw_bounds(&self) -> Option<accesskit::Rect> {
        self.info.node.bounds()
    }
}

impl std::fmt::Debug for AkNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Node");
        s.field("id", &self.info.id).field("role", &self.role());
        if self.info.tree != TreeId::ROOT {
            s.field("tree", &self.info.tree);
        }
        if let Some(l) = self.label() {
            s.field("label", &l);
        }
        if let Some(v) = self.value() {
            s.field("value", &v);
        }
        if let Some(a) = self.author_id() {
            s.field("author_id", &a);
        }
        if self.is_disabled() {
            s.field("disabled", &true);
        }
        if self.is_focused() {
            s.field("focused", &true);
        }
        s.finish()
    }
}
