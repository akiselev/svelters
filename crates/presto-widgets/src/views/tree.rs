//! `tree-view`: a tree drawn by the `virtual-list` row machinery ([`super::virtual_list`]): the M5
//! slots, recycling, row heights and keyed selection are the list's. The tree keeps the model (ids,
//! parents, children, the expanded set) natively and **flattens its visible rows** into the list:
//! expanding a node splices its loaded descendants in after it, collapsing splices them out, so only
//! the rows that moved rebind. Lazy children stay JS's: expanding a node whose children were never
//! given raises `expand(id)`, and JS answers with `setChildren`.
//! Ported from svelters-egui `kinds/treeview.rs` (where JS did the flattening).
//!
//! ```text
//! props  virtual-list's, plus indent (points per level; the disclosure triangle sits in the last)
//! calls  setChildren(parent | -1, ids, labels, expandable), expandNode(id), collapseNode(id)
//! async  virtual-list's, plus expand(id), collapse(id)
//! ```
//!
//! **Keyboard**: Right expands a collapsed node or goes to its first child; Left collapses an
//! expanded node or goes to its parent; `*` expands the cursor's siblings; the rest (moves, Space,
//! Enter, type-ahead) is the list's. **Pointer**: a press on a row's disclosure triangle toggles it
//! without selecting. **Drag**: a drag that holds over a collapsed node for `Policy::drag_expand`
//! expands it ([`TreeView::drag_over`]). **A11y**: a `Tree` of `TreeItem`s with level (1-based),
//! `expanded`, selection and position over the whole flattened list.

use super::draw;
use super::shim::{Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal};
use super::virtual_list::{Mods, SELECTION, VirtualChildren, VirtualList, id_list, set_list_prop};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::event::{KeyState, MouseButton, PointerPhase};
use presto_base::geom::Point;
use presto_base::key::Key;
use presto_base::time::Time;
use std::collections::{HashMap, HashSet};

/// The default row height of a tree, in points.
pub const TREE_ROW_HEIGHT: f32 = 24.0;

/// One node of the model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeNode {
    pub parent: Option<u64>,
    pub label: String,
    /// It has (or may load) children: it shows a disclosure triangle.
    pub expandable: bool,
    /// The children given so far (`None`: never given; expanding raises `expand`).
    pub children: Option<Vec<u64>>,
}

/// A visible row of the flattened tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub id: u64,
    /// 0 for the roots.
    pub depth: u32,
    pub expandable: bool,
    pub expanded: bool,
}

/// The state of one `tree-view`.
#[derive(Clone, Debug)]
pub struct TreeView {
    pub list: VirtualList,
    nodes: HashMap<u64, TreeNode>,
    roots: Vec<u64>,
    expanded: HashSet<u64>,
    /// The flattened visible rows, by position (the list's ids, in the same order).
    rows: Vec<TreeRow>,
    /// Points per level.
    pub indent: f32,
    /// The collapsed node a drag hovers over, and since when.
    drag_hover: Option<(u64, Time)>,
}

impl Default for TreeView {
    fn default() -> Self {
        TreeView::new()
    }
}

impl TreeView {
    pub fn new() -> TreeView {
        TreeView {
            list: VirtualList::new(TREE_ROW_HEIGHT),
            nodes: HashMap::new(),
            roots: Vec::new(),
            expanded: HashSet::new(),
            rows: Vec::new(),
            indent: 16.0,
            drag_hover: None,
        }
    }

    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }
    pub fn node(&self, id: u64) -> Option<&TreeNode> {
        self.nodes.get(&id)
    }
    pub fn is_expanded(&self, id: u64) -> bool {
        self.expanded.contains(&id)
    }
    fn row(&self, id: u64) -> Option<&TreeRow> {
        self.rows.iter().find(|r| r.id == id)
    }

    /// `setChildren(parent, ids, labels, expandable)`: the children of `parent` (`None`: the roots).
    /// Nodes no longer anywhere are forgotten with their descendants; a node that moves keeps its
    /// expanded state.
    pub fn set_children(&mut self, parent: Option<u64>, children: &[(u64, String, bool)]) -> Result<(), String> {
        if let Some(p) = parent
            && !self.nodes.contains_key(&p)
        {
            return Err(format!("setChildren: no node {p}"));
        }
        let mut seen = HashSet::new();
        if let Some(&(dup, ..)) = children.iter().find(|(id, ..)| !seen.insert(*id)) {
            return Err(format!("setChildren: id {dup} twice"));
        }
        // a node can't become its own descendant
        let mut up = parent;
        while let Some(a) = up {
            if seen.contains(&a) {
                return Err(format!("setChildren: {a} would be its own ancestor"));
            }
            up = self.nodes.get(&a).and_then(|n| n.parent);
        }
        let old: Vec<u64> = match parent {
            None => std::mem::take(&mut self.roots),
            Some(p) => self.nodes.get_mut(&p).and_then(|n| n.children.take()).unwrap_or_default(),
        };
        // nodes that moved here leave their previous parent's list
        for (id, ..) in children {
            if let Some(n) = self.nodes.get(id)
                && n.parent != parent
                && !old.contains(id)
            {
                let prev = n.parent;
                match prev {
                    None => self.roots.retain(|x| x != id),
                    Some(q) => {
                        if let Some(c) = self.nodes.get_mut(&q).and_then(|n| n.children.as_mut()) {
                            c.retain(|x| x != id);
                        }
                    }
                }
            }
        }
        for (id, label, expandable) in children {
            let n = self.nodes.entry(*id).or_default();
            n.parent = parent;
            n.label = label.clone();
            n.expandable = *expandable;
        }
        let ids: Vec<u64> = children.iter().map(|(id, ..)| *id).collect();
        for gone in old.into_iter().filter(|x| !seen.contains(x)) {
            self.forget(gone);
        }
        match parent {
            None => self.roots = ids,
            Some(p) => self.nodes.get_mut(&p).expect("checked").children = Some(ids),
        }
        self.sync();
        Ok(())
    }

    fn forget(&mut self, id: u64) {
        if let Some(n) = self.nodes.remove(&id) {
            self.expanded.remove(&id);
            for c in n.children.unwrap_or_default() {
                self.forget(c);
            }
        }
    }

    /// The visible rows: the roots and, under each expanded node, its loaded children, depth first.
    pub fn flatten(&self) -> Vec<TreeRow> {
        let mut out = Vec::new();
        let mut stack: Vec<(u64, u32)> = self.roots.iter().rev().map(|&id| (id, 0)).collect();
        while let Some((id, depth)) = stack.pop() {
            let Some(n) = self.nodes.get(&id) else { continue };
            let expanded = n.expandable && self.expanded.contains(&id);
            out.push(TreeRow { id, depth, expandable: n.expandable, expanded });
            if expanded && let Some(c) = &n.children {
                stack.extend(c.iter().rev().map(|&c| (c, depth + 1)));
            }
        }
        out
    }

    /// Re-flattens and splices only the changed run into the list (common prefix and suffix kept),
    /// so expanding or collapsing one node rebinds only the rows after it that moved.
    fn sync(&mut self) {
        let new = self.flatten();
        let old_ids = self.list.ids().to_vec();
        let new_ids: Vec<u64> = new.iter().map(|r| r.id).collect();
        let pre = old_ids.iter().zip(&new_ids).take_while(|(a, b)| a == b).count();
        let max_suf = old_ids.len().min(new_ids.len()) - pre;
        let suf = old_ids.iter().rev().zip(new_ids.iter().rev()).take(max_suf).take_while(|(a, b)| a == b).count();
        let removed = old_ids.len() - pre - suf;
        let added = &new_ids[pre..new_ids.len() - suf];
        if removed > 0 || !added.is_empty() {
            let labels: Vec<String> = added.iter().map(|id| self.nodes[id].label.clone()).collect();
            self.list.splice(pre, removed, added, Some(&labels)).expect("in range");
        }
        // labels of kept rows may have changed (setChildren with a new label)
        self.list.set_labels(new_ids.iter().map(|id| self.nodes[id].label.clone()).collect());
        self.rows = new;
    }

    /// Expands `id`; raises `expand(id)` so JS can load lazy children. False if nothing changed.
    pub fn expand(&mut self, cx: &mut EventCx<'_>, id: u64) -> bool {
        let Some(n) = self.nodes.get(&id) else { return false };
        if !n.expandable || !self.expanded.insert(id) {
            return false;
        }
        cx.emit("expand", Value::I64(id as i64));
        cx.mark(ChangeClass::Structure);
        self.sync();
        true
    }

    /// Collapses `id`; a cursor inside it moves to it. False if nothing changed.
    pub fn collapse(&mut self, cx: &mut EventCx<'_>, id: u64) -> bool {
        if !self.expanded.remove(&id) {
            return false;
        }
        if let Some(c) = self.list.sel.cursor
            && self.is_descendant(c, id)
        {
            self.list.sel.cursor = Some(id);
            cx.emit("current", Value::I64(id as i64));
        }
        cx.emit("collapse", Value::I64(id as i64));
        cx.mark(ChangeClass::Structure);
        self.sync();
        true
    }

    fn is_descendant(&self, id: u64, of: u64) -> bool {
        let mut up = self.nodes.get(&id).and_then(|n| n.parent);
        while let Some(a) = up {
            if a == of {
                return true;
            }
            up = self.nodes.get(&a).and_then(|n| n.parent);
        }
        false
    }

    /// The first visible child of `id`.
    pub fn first_child(&self, id: u64) -> Option<u64> {
        let p = self.rows.iter().position(|r| r.id == id)?;
        let r = self.rows[p];
        self.rows.get(p + 1).filter(|c| c.depth == r.depth + 1).map(|c| c.id)
    }

    /// Right: expand, or go to the first child.
    fn right(&mut self, cx: &mut EventCx<'_>, m: Mods) -> bool {
        let Some(c) = self.list.sel.cursor else { return false };
        let Some(r) = self.row(c).copied() else { return false };
        if r.expandable && !r.expanded {
            return self.expand(cx, c);
        }
        match self.first_child(c) {
            Some(child) => self.go(cx, child, m),
            None => false,
        }
    }

    /// Left: collapse, or go to the parent.
    fn left(&mut self, cx: &mut EventCx<'_>, m: Mods) -> bool {
        let Some(c) = self.list.sel.cursor else { return false };
        if self.is_expanded(c) {
            return self.collapse(cx, c);
        }
        match self.nodes.get(&c).and_then(|n| n.parent) {
            Some(p) => self.go(cx, p, m),
            None => false,
        }
    }

    fn go(&mut self, cx: &mut EventCx<'_>, id: u64, m: Mods) -> bool {
        let ids = self.list.ids().to_vec();
        let ch = self.list.sel.go_to(&ids, id, m);
        if ch.cursor {
            if let Some(p) = ids.iter().position(|&x| x == id) {
                self.list.scroll_to(p, super::virtual_list::Reveal::Nearest);
            }
            cx.emit("current", Value::I64(id as i64));
            cx.mark(ChangeClass::Paint);
        }
        if ch.selection {
            cx.emit("selectionchange", Value::List(self.list.sel.selected.iter().map(|&i| Value::I64(i as i64)).collect()));
        }
        ch.cursor || ch.selection
    }

    /// `*`: expands every expandable sibling of the cursor (and the cursor).
    fn expand_siblings(&mut self, cx: &mut EventCx<'_>) -> bool {
        let Some(c) = self.list.sel.cursor else { return false };
        let parent = self.nodes.get(&c).and_then(|n| n.parent);
        let sibs: Vec<u64> = match parent {
            None => self.roots.clone(),
            Some(p) => self.nodes.get(&p).and_then(|n| n.children.clone()).unwrap_or_default(),
        };
        let mut any = false;
        for s in sibs {
            any |= self.expand(cx, s);
        }
        any
    }

    /// The disclosure triangle's rect of the row at `pos` (list-local).
    pub fn disclosure_rect(&self, pos: usize) -> Option<presto_base::geom::Rect> {
        let r = self.rows.get(pos).filter(|r| r.expandable)?;
        let row = self.list.row_rect(pos)?;
        let x0 = r.depth as f32 * self.indent;
        Some(presto_base::geom::Rect::new(x0, row.min.y, x0 + self.indent, row.max.y))
    }

    /// The pointer position's row, if it is on that row's disclosure triangle.
    fn on_disclosure(&self, p: Point) -> Option<u64> {
        let pos = self.list.pos_at(p)?;
        self.disclosure_rect(pos).filter(|r| r.contains(p)).map(|_| self.rows[pos].id)
    }

    /// A drag hovers `into` the node `id` (`None`: it left, or hovers between rows). A collapsed
    /// expandable node held for `Policy::drag_expand` expands (on the [`Event::Timer`] asked for).
    pub fn drag_over(&mut self, cx: &mut EventCx<'_>, id: Option<u64>) {
        let delay = cx.policy.drag_expand;
        let target = id.filter(|i| self.row(*i).is_some_and(|r| r.expandable && !r.expanded));
        match (target, self.drag_hover) {
            (Some(t), Some((h, _))) if t == h => {}
            (Some(t), _) if delay > presto_base::time::Span::ZERO => {
                self.drag_hover = Some((t, cx.now));
                cx.wake_at(cx.now + delay);
            }
            _ => self.drag_hover = None,
        }
        self.drag_timer(cx);
    }

    fn drag_timer(&mut self, cx: &mut EventCx<'_>) {
        if let Some((id, since)) = self.drag_hover
            && cx.now - since >= cx.policy.drag_expand
        {
            self.drag_hover = None;
            self.expand(cx, id);
        }
    }

    /// The AccessKit node of the row at `pos`.
    pub fn row_a11y(&self, pos: usize) -> Option<accesskit::Node> {
        let r = self.rows.get(pos)?;
        let mut n = self.list.row_a11y(pos, accesskit::Role::TreeItem)?;
        n.set_level(r.depth as usize + 1);
        if r.expandable {
            n.set_expanded(r.expanded);
            n.add_action(if r.expanded { accesskit::Action::Collapse } else { accesskit::Action::Expand });
        }
        Some(n)
    }
}

static TREE_SPEC: KindSpec = KindSpec {
    name: "tree-view",
    props: &[
        prop("selection", PropType::Enum(SELECTION), ChangeClass::Paint),
        prop("row-height", PropType::F64, ChangeClass::Layout),
        prop("uniform-rows", PropType::Bool, ChangeClass::Layout),
        prop("selected-ids", PropType::List, ChangeClass::Paint),
        prop("current", PropType::I64, ChangeClass::Paint),
        prop("pool-limit", PropType::I64, ChangeClass::Property),
        prop("indent", PropType::F64, ChangeClass::Paint),
        prop("expanded-ids", PropType::List, ChangeClass::Structure),
    ],
    signals: &[
        signal("setup", false, false),
        signal("bind", false, false),
        signal("unbind", false, false),
        signal("teardown", false, false),
        signal("activate", true, false),
        signal("select", true, false),
        signal("selectionchange", true, false),
        signal("current", true, false),
        signal("row-clicked", true, false),
        signal("row-context", true, false),
        signal("row-hovered", true, false),
        signal("copy", true, false),
        signal("expand", true, false),
        signal("collapse", true, false),
    ],
    slots: &[""],
    role: accesskit::Role::Tree,
};

impl Kind for TreeView {
    fn spec() -> &'static KindSpec {
        &TREE_SPEC
    }

    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        match name {
            "indent" => {
                self.indent = value.as_f32().filter(|v| *v >= 0.0).ok_or(PropError::Invalid("indent >= 0"))?;
                Ok(ChangeClass::Paint)
            }
            "expanded-ids" => {
                self.expanded = id_list(value)?.into_iter().collect();
                self.sync();
                Ok(ChangeClass::Structure)
            }
            _ => set_list_prop(&mut self.list, name, value),
        }
    }

    /// The list's row backgrounds, then each expandable row's disclosure chevron (`>` collapsed,
    /// `v` expanded).
    fn record(&self, cx: &mut RecordCx<'_>) {
        self.list.record_rows(cx);
        let color = cx.palette.text_muted;
        for pos in self.list.window() {
            let Some(r) = self.disclosure_rect(pos) else { continue };
            let (c, s) = (Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0), 3.0);
            let row = self.rows[pos];
            let (a, b, d) = if row.expanded {
                (Point::new(c.x - s, c.y - s / 2.0), Point::new(c.x, c.y + s / 2.0), Point::new(c.x + s, c.y - s / 2.0))
            } else {
                (Point::new(c.x - s / 2.0, c.y - s), Point::new(c.x + s / 2.0, c.y), Point::new(c.x - s / 2.0, c.y + s))
            };
            draw::line(cx.list, a, b, 1.5, color);
            draw::line(cx.list, b, d, 1.5, color);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        let macos = cx.policy.os == super::shim::Os::MacOs;
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed => {
                let m = Mods { shift: k.modifiers.contains(presto_base::event::Modifiers::SHIFT), ctrl: k.modifiers.primary(macos) };
                let done = match k.key {
                    Some(Key::ArrowRight) => Some(self.right(cx, m)),
                    Some(Key::ArrowLeft) => Some(self.left(cx, m)),
                    _ if k.text.as_deref() == Some("*") => Some(self.expand_siblings(cx)),
                    _ => None,
                };
                match done {
                    Some(_) => Handled::Yes,
                    None => self.list.handle(cx, ev),
                }
            }
            Event::Pointer(p) if p.phase == PointerPhase::Down && p.button == Some(MouseButton::Primary) => {
                match self.on_disclosure(p.pos) {
                    Some(id) => {
                        if !self.collapse(cx, id) {
                            self.expand(cx, id);
                        }
                        Handled::Yes
                    }
                    None => self.list.handle(cx, ev),
                }
            }
            Event::Timer => {
                self.drag_timer(cx);
                Handled::No
            }
            Event::A11y(r) if matches!(r.action, accesskit::Action::Expand | accesskit::Action::Collapse) => {
                // the action addresses the cursor row (rows are not nodes of their own yet)
                let Some(c) = self.list.sel.cursor else { return Handled::No };
                if r.action == accesskit::Action::Expand { self.expand(cx, c) } else { self.collapse(cx, c) };
                Handled::Yes
            }
            _ => self.list.handle(cx, ev),
        }
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        self.list.a11y(node);
        node.set_role(accesskit::Role::Tree);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::shim::{Os, Policy};
    use presto_base::event::{KeyEvent, KeyLocation, Modifiers, PointerEvent, PointerType};
    use presto_base::geom::Size;
    use presto_base::time::Span;

    /// 1 ─┬─ 11 ─── 111
    ///    └─ 12
    /// 2 (lazy)
    /// 3
    fn tree() -> TreeView {
        let mut t = TreeView::new();
        let n = |id: u64, l: &str, e: bool| (id, l.to_string(), e);
        t.set_children(None, &[n(1, "one", true), n(2, "two", true), n(3, "three", false)]).unwrap();
        t.set_children(Some(1), &[n(11, "eleven", true), n(12, "twelve", false)]).unwrap();
        t.set_children(Some(11), &[n(111, "one-eleven", false)]).unwrap();
        t.list.set_viewport(0.0, 240.0, 200.0);
        t
    }

    fn ids(t: &TreeView) -> Vec<u64> {
        t.rows().iter().map(|r| r.id).collect()
    }

    fn key(k: Key) -> Event {
        Event::Key(KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() })
    }

    #[test]
    fn flattens_only_expanded_children() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        assert_eq!(ids(&t), vec![1, 2, 3]);
        assert_eq!(t.list.ids(), &[1, 2, 3]);
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        assert!(t.expand(&mut cx, 1));
        assert_eq!(ids(&t), vec![1, 11, 12, 2, 3]);
        assert!(t.expand(&mut cx, 11));
        assert_eq!(ids(&t), vec![1, 11, 111, 12, 2, 3]);
        assert_eq!(t.rows()[2].depth, 2);
        assert_eq!(t.list.ids(), &[1, 11, 111, 12, 2, 3]);
        // collapsing the parent hides the grandchild but keeps 11 expanded for later
        assert!(t.collapse(&mut cx, 1));
        assert_eq!(ids(&t), vec![1, 2, 3]);
        assert!(t.is_expanded(11));
        t.expand(&mut cx, 1);
        assert_eq!(ids(&t), vec![1, 11, 111, 12, 2, 3]);
        assert!(!t.expand(&mut cx, 3), "a leaf does not expand");
    }

    #[test]
    fn lazy_children_raise_expand_and_arrive_later() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.expand(&mut cx, 2);
        assert_eq!(cx.emitted("expand").next().unwrap().detail, Value::I64(2));
        assert_eq!(ids(&t), vec![1, 2, 3], "nothing loaded yet");
        t.set_children(Some(2), &[(21, "a".into(), false), (22, "b".into(), false)]).unwrap();
        assert_eq!(ids(&t), vec![1, 2, 21, 22, 3]);
        // children replaced: 21 forgotten
        t.set_children(Some(2), &[(22, "b".into(), false)]).unwrap();
        assert!(t.node(21).is_none());
        assert!(t.set_children(Some(99), &[]).is_err());
        assert!(t.set_children(Some(2), &[(2, "self".into(), false)]).is_err());
    }

    #[test]
    fn expanding_splices_only_the_new_rows() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        // realise the rows (answering setups)
        for _ in 0..2 {
            for r in t.list.plan() {
                if let super::super::virtual_list::RowRequest::Setup { slot } = r {
                    t.list.set_row(slot, Some(presto_base::ids::NodeId::new(slot as u32 + 1, 1)));
                }
            }
        }
        let slot_of_3 = t.list.row_at_pos(2).unwrap();
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.expand(&mut cx, 1);
        // row 3 moved from 2 to 4 in its slot, without a rebind
        assert_eq!(t.list.slots()[slot_of_3].pos, Some(4));
        assert_eq!(t.list.slots()[slot_of_3].id, 3);
    }

    #[test]
    fn right_and_left_expand_descend_collapse_and_ascend() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.event(&mut cx, &key(Key::ArrowDown));
        assert_eq!(t.list.sel.cursor, Some(1));
        t.event(&mut cx, &key(Key::ArrowRight));
        assert!(t.is_expanded(1));
        assert_eq!(t.list.sel.cursor, Some(1), "the first Right only expands");
        t.event(&mut cx, &key(Key::ArrowRight));
        assert_eq!(t.list.sel.cursor, Some(11));
        t.event(&mut cx, &key(Key::ArrowRight));
        t.event(&mut cx, &key(Key::ArrowRight));
        assert_eq!(t.list.sel.cursor, Some(111));
        t.event(&mut cx, &key(Key::ArrowLeft));
        assert_eq!(t.list.sel.cursor, Some(11), "a leaf goes to its parent");
        t.event(&mut cx, &key(Key::ArrowLeft));
        assert!(!t.is_expanded(11));
        assert_eq!(t.list.sel.is_selected(11), true);
        t.event(&mut cx, &key(Key::ArrowDown));
        assert_eq!(t.list.sel.cursor, Some(12));
        // collapsing the root moves the cursor out of the hidden rows
        t.list.sel.cursor = Some(12);
        t.collapse(&mut cx, 1);
        assert_eq!(t.list.sel.cursor, Some(1));
    }

    #[test]
    fn star_expands_the_siblings() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.event(&mut cx, &key(Key::ArrowDown));
        let star = Event::Key(KeyEvent { key: None, physical: 0, text: Some("*".into()), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::SHIFT });
        assert_eq!(t.event(&mut cx, &star), Handled::Yes);
        assert!(t.is_expanded(1) && t.is_expanded(2) && !t.is_expanded(3));
    }

    #[test]
    fn a_press_on_the_triangle_toggles_without_selecting() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        let press = |x: f32, y: f32| {
            Event::Pointer(PointerEvent {
                phase: PointerPhase::Down,
                pointer: 0,
                kind: PointerType::Mouse,
                button: Some(MouseButton::Primary),
                buttons: 1,
                pos: Point::new(x, y),
                modifiers: Modifiers::default(),
                pressure: 0.5,
                tilt: Default::default(),
                history: Default::default(),
            })
        };
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.event(&mut cx, &press(8.0, 12.0));
        assert!(t.is_expanded(1));
        assert!(t.list.sel.selected.is_empty());
        // the label part of row 11 (depth 1) selects
        t.event(&mut cx, &press(60.0, 36.0));
        assert_eq!(t.list.sel.cursor, Some(11));
        // row 11's triangle is one indent in
        t.event(&mut cx, &press(20.0, 36.0));
        assert!(t.is_expanded(11));
    }

    #[test]
    fn a_held_drag_expands_after_the_delay() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.drag_over(&mut cx, Some(1));
        assert_eq!(cx.wake, Some(Time::ZERO + pol.drag_expand));
        assert!(!t.is_expanded(1));
        let mut cx = EventCx::new(Time::ZERO + Span::from_millis(200), Size::new(200.0, 240.0), &pol);
        t.drag_over(&mut cx, Some(1));
        assert!(!t.is_expanded(1), "still holding");
        let mut cx = EventCx::new(Time::ZERO + pol.drag_expand, Size::new(200.0, 240.0), &pol);
        t.event(&mut cx, &Event::Timer);
        assert!(t.is_expanded(1));
        // moving off resets
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.drag_over(&mut cx, Some(2));
        t.drag_over(&mut cx, None);
        let mut cx = EventCx::new(Time::ZERO + Span::from_millis(5000), Size::new(200.0, 240.0), &pol);
        t.event(&mut cx, &Event::Timer);
        assert!(!t.is_expanded(2));
    }

    #[test]
    fn rows_are_tree_items_with_levels() {
        let pol = Policy::for_os(Os::Linux);
        let mut t = tree();
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 240.0), &pol);
        t.expand(&mut cx, 1);
        let n = t.row_a11y(0).unwrap();
        assert_eq!(n.role(), accesskit::Role::TreeItem);
        assert_eq!(n.level(), Some(1));
        assert_eq!(n.is_expanded(), Some(true));
        let n = t.row_a11y(2).unwrap();
        assert_eq!(n.level(), Some(2));
        assert_eq!(n.is_expanded(), None, "a leaf");
        assert_eq!(n.size_of_set(), Some(5));
    }
}
