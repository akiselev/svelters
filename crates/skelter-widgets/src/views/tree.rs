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

use super::common::{emit, ms, platform};
use super::virtual_list::{Mods, SELECTION, VirtualList, id_list, list_signals};
use crate::interact::Platform;
use crate::kind_props;
use crate::paint::{Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, Value};
use skelter_base::accesskit;
use skelter_base::color::Color;
use skelter_base::display::{Line, LineCap, Prim};
use skelter_base::event::{KeyState, MouseButton, PointerPhase};
use skelter_base::geom::{Point, Rect, Vec2};
use skelter_base::key::Key;
use skelter_base::time::{Span, Time};
use std::ops::Range;
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
    /// `indent` and `expanded-ids` as last written (the list props live in [`Self::list`]).
    pub props: TreeProps,
    pub list: VirtualList,
    nodes: HashMap<u64, TreeNode>,
    roots: Vec<u64>,
    expanded: HashSet<u64>,
    /// The flattened visible rows, by position (the list's ids, in the same order).
    rows: Vec<TreeRow>,
    /// JS flattens the tree (svelters-egui's protocol: `splice` and `setNodes`): the rows come
    /// from the list's ids and [`Self::facts`], and expanding or collapsing only asks JS.
    js: bool,
    /// `setNodes`' facts by id: level, flags (1 expandable, 2 expanded), label.
    facts: HashMap<u64, (i64, i64, String)>,
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
            props: TreeProps::default(),
            list: VirtualList::new(TREE_ROW_HEIGHT),
            nodes: HashMap::new(),
            roots: Vec::new(),
            expanded: HashSet::new(),
            rows: Vec::new(),
            js: false,
            facts: HashMap::new(),
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
        if self.js {
            if !self.row(id).is_some_and(|r| r.expandable && !r.expanded) {
                return false;
            }
            emit(cx, &SPEC, "expand", vec![Value::I64(id as i64)]);
            return true;
        }
        let Some(n) = self.nodes.get(&id) else { return false };
        if !n.expandable || !self.expanded.insert(id) {
            return false;
        }
        emit(cx, &SPEC, "expand", vec![Value::I64(id as i64)]);
        cx.prop_changed(P::expanded_ids.id());
        self.sync();
        true
    }

    /// Collapses `id`; a cursor inside it moves to it. False if nothing changed.
    pub fn collapse(&mut self, cx: &mut EventCx<'_>, id: u64) -> bool {
        if self.js {
            if !self.row(id).is_some_and(|r| r.expanded) {
                return false;
            }
            emit(cx, &SPEC, "collapse", vec![Value::I64(id as i64)]);
            return true;
        }
        if !self.expanded.remove(&id) {
            return false;
        }
        if let Some(c) = self.list.sel.cursor
            && self.is_descendant(c, id)
        {
            self.list.sel.cursor = Some(id);
            emit(cx, &SPEC, "current", vec![Value::I64(id as i64)]);
            cx.prop_changed(P::current.id());
        }
        emit(cx, &SPEC, "collapse", vec![Value::I64(id as i64)]);
        cx.prop_changed(P::expanded_ids.id());
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
        if self.is_expanded(c) || (self.js && self.row(c).is_some_and(|r| r.expanded)) {
            return self.collapse(cx, c);
        }
        let parent = match self.js {
            // JS mode: the nearest row above one level up
            true => self.rows.iter().position(|r| r.id == c).and_then(|p| {
                let d = self.rows[p].depth;
                self.rows[..p].iter().rev().find(|r| r.depth + 1 == d).map(|r| r.id)
            }),
            false => self.nodes.get(&c).and_then(|n| n.parent),
        };
        match parent {
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
            emit(cx, &SPEC, "current", vec![Value::I64(id as i64)]);
            cx.prop_changed(P::current.id());
        }
        if ch.selection {
            let v = self.list.selected_value();
            emit(cx, &SPEC, "selectionchange", vec![v]);
            cx.prop_changed(P::selected_ids.id());
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
    pub fn disclosure_rect(&self, pos: usize) -> Option<Rect> {
        let r = self.rows.get(pos).filter(|r| r.expandable)?;
        let row = self.list.row_rect(pos)?;
        let x0 = r.depth as f32 * self.indent;
        Some(Rect::new(x0, row.min.y, x0 + self.indent, row.max.y))
    }

    /// The pointer position's row, if it is on that row's disclosure triangle.
    fn on_disclosure(&self, p: Point) -> Option<u64> {
        let pos = self.list.pos_at(p)?;
        let c = self.list.to_content(p);
        self.disclosure_rect(pos).filter(|r| r.contains(c)).map(|_| self.rows[pos].id)
    }

    /// A drag hovers `into` the node `id` (`None`: it left, or hovers between rows). A collapsed
    /// expandable node held for `Policy::drag_expand_ms` expands (on the [`Event::Timer`] asked for).
    pub fn drag_over(&mut self, cx: &mut EventCx<'_>, id: Option<u64>) {
        let delay = ms(cx.policy.drag_expand_ms);
        let target = id.filter(|i| self.row(*i).is_some_and(|r| r.expandable && !r.expanded));
        match (target, self.drag_hover) {
            (Some(t), Some((h, _))) if t == h => {}
            (Some(t), _) if delay > Span::ZERO => {
                self.drag_hover = Some((t, cx.now));
                cx.wake_at(cx.now + delay);
            }
            _ => self.drag_hover = None,
        }
        self.drag_timer(cx);
    }

    fn drag_timer(&mut self, cx: &mut EventCx<'_>) {
        if let Some((id, since)) = self.drag_hover
            && cx.now - since >= ms(cx.policy.drag_expand_ms)
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

impl skelter_core::VirtualChildren for TreeView {
    fn row_count(&self) -> usize {
        self.list.len()
    }
    fn visible(&self, viewport: Rect) -> Range<usize> {
        skelter_core::VirtualChildren::visible(&self.list, viewport)
    }
    /// The row's content starts after its level's disclosure triangle (`(depth + 1) × indent`, the
    /// same for leaves, so labels line up), not under it.
    fn row_rect(&self, i: usize) -> Rect {
        let r = skelter_core::VirtualChildren::row_rect(&self.list, i);
        let x = self.rows.get(i).map_or(0.0, |row| (row.depth as f32 + 1.0) * self.indent);
        Rect::new((r.min.x + x).min(r.max.x), r.min.y, r.max.x, r.max.y)
    }
    fn content_extent(&self) -> Vec2 {
        skelter_core::VirtualChildren::content_extent(&self.list)
    }
    fn realize(&mut self, cx: &mut skelter_core::RealizeCx, _rows: Range<usize>) -> Vec<(usize, skelter_base::ids::NodeId)> {
        self.list.realize_rows(cx, &SPEC)
    }
}

/// `::part(twisty)`: the disclosure chevron, in the theme's icon colour.
pub const TWISTY_UA: Ua = Ua(&[UaRule::base(&[("color", Token("icon"))])]);

kind_props! {
    /// The tree's props: the list's six (same order and indices: they are forwarded to it), then its own.
    pub struct TreeProps;
    selection "selection": String = Enum("single") ; Enum(SELECTION), Paint;
    row_height "row-height": f64 = F64(24.0) ; F64, Layout;
    uniform_rows "uniform-rows": bool = Bool(false) ; Bool, Layout;
    selected_ids "selected-ids": Vec<Value> = None ; List, Paint, bindable;
    current "current": i64 = I64(-1) ; I64, Paint, bindable;
    pool_limit "pool-limit": i64 = I64(8) ; I64, Property;
    /// Points per level; the disclosure triangle sits in the last.
    indent "indent": f64 = F64(16.0) ; F64, Paint;
    /// The expanded node ids (native-owned: keys, presses and drags change it).
    expanded_ids "expanded-ids": Vec<Value> = None ; List, Structure, bindable;
}

/// The list's signals, then `expand(id)` (JS loads lazy children) and `collapse(id)`.
pub const SIGNALS: &[SignalSpec] = list_signals!(
    SignalSpec::new("expand", &[("id", crate::shim::core::ValueType::I64)], "A node expanded; JS answers `setChildren` if it never gave its children."),
    SignalSpec::new("collapse", &[("id", crate::shim::core::ValueType::I64)], ""),
);

pub static SPEC: KindSpec = KindSpec {
    tag: "tree-view",
    props: TreeProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::Tree,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: true,
};

/// The list's props are the first six of the tree's.
const LIST_PROPS: u32 = 6;

/// svelters-egui's `tree-view` methods: the list's ([`super::virtual_list::LIST_METHODS`], same
/// order), then `setNodes(ids, levels, flags, labels?, data?)`, `selection()` and
/// `setSelection(ids)`.
pub static TREE_METHODS: &[skelter_core::MethodSpec] = &[
    super::virtual_list::LIST_METHODS[0],
    super::virtual_list::LIST_METHODS[1],
    super::virtual_list::LIST_METHODS[2],
    super::virtual_list::LIST_METHODS[3],
    super::virtual_list::LIST_METHODS[4],
    skelter_core::MethodSpec::new(
        "setNodes",
        &[
            skelter_core::ArgSpec::new("ids", skelter_core::ValueType::List),
            skelter_core::ArgSpec::new("levels", skelter_core::ValueType::List),
            skelter_core::ArgSpec::new("flags", skelter_core::ValueType::List),
            skelter_core::ArgSpec::new("labels", skelter_core::ValueType::List).nullable(),
            skelter_core::ArgSpec::new("data", skelter_core::ValueType::List).nullable(),
        ],
        None,
    ),
    skelter_core::MethodSpec::new("selection", &[], Some(skelter_core::ArgSpec::new("return", skelter_core::ValueType::List))),
    skelter_core::MethodSpec::new("setSelection", &[skelter_core::ArgSpec::new("ids", skelter_core::ValueType::List)], None),
];

impl TreeView {
    /// The tree's keys and disclosure presses, then the list's defaults.
    fn tree_event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let macos = platform(cx.policy) == Platform::Macos;
        match ev {
            Event::Key(k) if k.state == KeyState::Pressed => {
                let m = Mods { shift: k.modifiers.contains(skelter_base::event::Modifiers::SHIFT), ctrl: k.modifiers.primary(macos) };
                let done = match k.key {
                    Some(Key::ArrowRight) => Some(self.right(cx, m)),
                    Some(Key::ArrowLeft) => Some(self.left(cx, m)),
                    _ if k.text.as_deref() == Some("*") => Some(self.expand_siblings(cx)),
                    _ => None,
                };
                match done {
                    Some(_) => {
                        cx.repaint = true;
                        Handled::Yes
                    }
                    None => self.list.handle(cx, &SPEC, ev),
                }
            }
            Event::Pointer(p) if p.phase == PointerPhase::Down && p.button == Some(MouseButton::Primary) => match self.on_disclosure(p.pos) {
                Some(id) => {
                    if !self.collapse(cx, id) {
                        self.expand(cx, id);
                    }
                    Handled::Yes
                }
                None => self.list.handle(cx, &SPEC, ev),
            },
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
            _ => self.list.handle(cx, &SPEC, ev),
        }
    }


    /// svelters-egui's tree methods (JS flattens the tree: `splice` is the visible rows, `setNodes`
    /// their facts), then the list's.
    fn call_tree(&mut self, cx: &mut EventCx, name: &str, args: &[skelter_core::Value]) -> Handled {
        use skelter_core::Value as V;
        let ints = |k: usize| -> Vec<i64> {
            match args.get(k) {
                Some(V::List(l)) => l.iter().map(|x| x.as_i64().unwrap_or(0)).collect(),
                _ => Vec::new(),
            }
        };
        let strs = |k: usize| -> Vec<String> {
            match args.get(k) {
                Some(V::List(l)) => l.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
                _ => Vec::new(),
            }
        };
        match name {
            "setNodes" => {
                let (ids, levels, flags, labels) = (ints(0), ints(1), ints(2), strs(3));
                self.set_nodes(&ids.iter().map(|&i| i as u64).collect::<Vec<_>>(), &levels, &flags, &labels);
            }
            "selection" => {
                cx.ret = Some(V::List(self.list.sel.selected.iter().map(|&i| V::I64(i as i64)).collect::<Vec<_>>().into()));
                return Handled::Yes;
            }
            "setSelection" => {
                if self.list.sel.mode != super::virtual_list::SelectMode::None {
                    self.list.sel.set(ints(0).into_iter().map(|i| i as u64));
                }
            }
            "splice" => {
                self.js = true;
                let h = self.list.call_list(cx, name, args).unwrap_or(Handled::No);
                self.js_rows();
                return h;
            }
            other => return self.list.call_list(cx, other, args).unwrap_or(Handled::No),
        }
        cx.refresh_virtual = true;
        cx.repaint = true;
        Handled::Yes
    }

    /// `setNodes(ids, levels, flags, labels)`: the facts of rows by id (flags: 1 expandable, 2
    /// expanded; a level < 0 forgets the id). JS owns the flattening from then on.
    pub fn set_nodes(&mut self, ids: &[u64], levels: &[i64], flags: &[i64], labels: &[String]) {
        self.js = true;
        for (k, &id) in ids.iter().enumerate() {
            let level = levels.get(k).copied().unwrap_or(0);
            if level < 0 {
                self.facts.remove(&id);
                continue;
            }
            self.facts.insert(id, (level, flags.get(k).copied().unwrap_or(0), labels.get(k).cloned().unwrap_or_default()));
        }
        self.js_rows();
    }

    /// JS mode: the rows are the list's ids with the facts `setNodes` gave.
    fn js_rows(&mut self) {
        self.rows = self
            .list
            .ids()
            .iter()
            .map(|&id| {
                let (level, flags) = self.facts.get(&id).map_or((0, 0), |f| (f.0, f.1));
                TreeRow { id, depth: level.max(0) as u32, expandable: flags & 1 != 0, expanded: flags & 2 != 0 }
            })
            .collect();
        let labels: Vec<String> = self.rows.iter().map(|r| self.facts.get(&r.id).map(|f| f.2.clone()).unwrap_or_default()).collect();
        self.list.set_labels(labels);
    }
}

impl Kind for TreeView {
    fn parts() -> &'static [&'static str] {
        &["row", "twisty"]
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        if prop.0 < LIST_PROPS {
            return self.list.set_list_prop(prop, v);
        }
        let spec = TreeProps::SPECS.get(prop.0 as usize).ok_or(PropError::UnknownProp)?;
        let bad = PropError::Type { prop: spec.name, expected: spec.ty };
        if prop == P::indent.id() {
            if !v.as_f64().is_some_and(|i| i >= 0.0) {
                return Err(bad);
            }
            self.props.set(prop, v)?;
            self.indent = self.props.indent as f32;
        } else if prop == P::expanded_ids.id() {
            let ids = id_list(v).ok_or(bad)?;
            self.props.set(prop, v)?;
            self.expanded = ids.into_iter().collect();
            self.sync();
        } else {
            return Err(PropError::UnknownProp);
        }
        Ok(())
    }

    fn get_prop(&self, prop: PropId) -> Option<Value> {
        if prop.0 < LIST_PROPS {
            return self.list.list_prop(prop);
        }
        if prop == P::expanded_ids.id() {
            let mut ids: Vec<u64> = self.expanded.iter().copied().collect();
            ids.sort_unstable();
            return Some(Value::List(ids.into_iter().map(|i| Value::I64(i as i64)).collect()));
        }
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        self.list.measure(cx, known, avail)
    }

    /// The list's box and rows, then each expandable row's disclosure chevron (`>` collapsed, `v`
    /// expanded) as `::part(twisty)`.
    fn record(&self, cx: &mut RecordCx) {
        self.list.record_rows(cx);
        let host = VirtualList::host_view(cx);
        let twisty = host.part(cx.style.part_css("twisty"), TWISTY_UA);
        let color = twisty.color("color").unwrap_or(Color::TRANSPARENT);
        for pos in self.list.window() {
            let Some(r) = self.disclosure_rect(pos) else { continue };
            let (c, s) = (Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0), 3.0);
            let row = self.rows[pos];
            let (a, b, d) = if row.expanded {
                (Point::new(c.x - s, c.y - s / 2.0), Point::new(c.x, c.y + s / 2.0), Point::new(c.x + s, c.y - s / 2.0))
            } else {
                (Point::new(c.x - s / 2.0, c.y - s), Point::new(c.x + s / 2.0, c.y), Point::new(c.x - s / 2.0, c.y + s))
            };
            for (from, to) in [(a, b), (b, d)] {
                let ink = Rect::new(from.x.min(to.x), from.y.min(to.y), from.x.max(to.x), from.y.max(to.y)).inflate(1.5);
                cx.list.push(Prim::Line(Line { from, to, width: 1.5, color, cap: LineCap::Round, dash: None, wavy: false }), ink);
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let h = self.tree_event(cx, ev);
        if self.list.reveal_pending() {
            cx.refresh_virtual = true;
        }
        h
    }
    fn layout_style(&self, cx: &crate::shim::core::StyleCx) -> taffy::Style {
        super::virtual_list::scrolling_layout(cx)
    }
    fn methods() -> &'static [skelter_core::MethodSpec] {
        TREE_METHODS
    }
    fn call(&mut self, cx: &mut EventCx, method: usize, args: &[skelter_core::Value]) -> Handled {
        TREE_METHODS.get(method).map_or(Handled::No, |m| self.call_tree(cx, m.name, args))
    }
    fn virtual_children(&mut self) -> Option<&mut dyn skelter_core::VirtualChildren> {
        Some(self)
    }
    fn self_scroll(&self) -> Option<skelter_core::SelfScroll> {
        Some(self.list.self_scroll_extent())
    }

    fn a11y(&self, cx: &mut A11yCx) {
        self.list.list_a11y(cx.node);
        cx.node.set_role(accesskit::Role::Tree);
    }
}

#[cfg(test)]
mod tests {
    use super::super::virtual_list::test_support::{send, with_cx};
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Native, Policy, StateBits};
    use skelter_base::event::{KeyEvent, KeyLocation, Modifiers, PointerEvent, PointerType};
    use skelter_base::geom::Size;

    fn pol() -> Policy {
        *Native::Gtk.policy()
    }

    fn expand(t: &mut TreeView, id: u64) -> bool {
        with_cx(t, &pol(), Time::ZERO, |t, cx| t.expand(cx, id)).0
    }

    fn collapse(t: &mut TreeView, id: u64) -> bool {
        with_cx(t, &pol(), Time::ZERO, |t, cx| t.collapse(cx, id)).0
    }

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
        let mut t = tree();
        assert_eq!(ids(&t), vec![1, 2, 3]);
        assert_eq!(t.list.ids(), &[1, 2, 3]);
        assert!(expand(&mut t, 1));
        assert_eq!(ids(&t), vec![1, 11, 12, 2, 3]);
        assert!(expand(&mut t, 11));
        assert_eq!(ids(&t), vec![1, 11, 111, 12, 2, 3]);
        assert_eq!(t.rows()[2].depth, 2);
        assert_eq!(t.list.ids(), &[1, 11, 111, 12, 2, 3]);
        // collapsing the parent hides the grandchild but keeps 11 expanded for later
        assert!(collapse(&mut t, 1));
        assert_eq!(ids(&t), vec![1, 2, 3]);
        assert!(t.is_expanded(11));
        expand(&mut t, 1);
        assert_eq!(ids(&t), vec![1, 11, 111, 12, 2, 3]);
        assert!(!expand(&mut t, 3), "a leaf does not expand");
    }

    #[test]
    fn lazy_children_raise_expand_and_arrive_later() {
        let pol = pol();
        let mut t = tree();
        let (_, sent) = with_cx(&mut t, &pol, Time::ZERO, |t, cx| t.expand(cx, 2));
        assert_eq!(sent.arg("expand"), Some(Value::I64(2)));
        assert!(sent.changed.contains(&"expanded-ids"));
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
        let mut t = tree();
        // realise the rows (answering setups)
        for _ in 0..2 {
            for r in t.list.plan() {
                if let super::super::virtual_list::RowRequest::Setup { slot } = r {
                    t.list.set_row(slot, Some(skelter_base::ids::NodeId::new(slot as u32 + 1, 1)));
                }
            }
        }
        let slot_of_3 = t.list.row_at_pos(2).unwrap();
        expand(&mut t, 1);
        // row 3 moved from 2 to 4 in its slot, without a rebind
        assert_eq!(t.list.slots()[slot_of_3].pos, Some(4));
        assert_eq!(t.list.slots()[slot_of_3].id, 3);
    }

    #[test]
    fn right_and_left_expand_descend_collapse_and_ascend() {
        let pol = pol();
        let mut t = tree();
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowDown));
        assert_eq!(t.list.sel.cursor, Some(1));
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowRight));
        assert!(t.is_expanded(1));
        assert_eq!(t.list.sel.cursor, Some(1), "the first Right only expands");
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowRight));
        assert_eq!(t.list.sel.cursor, Some(11));
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowRight));
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowRight));
        assert_eq!(t.list.sel.cursor, Some(111));
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowLeft));
        assert_eq!(t.list.sel.cursor, Some(11), "a leaf goes to its parent");
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowLeft));
        assert!(!t.is_expanded(11));
        assert_eq!(t.list.sel.is_selected(11), true);
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowDown));
        assert_eq!(t.list.sel.cursor, Some(12));
        // collapsing the root moves the cursor out of the hidden rows
        t.list.sel.cursor = Some(12);
        collapse(&mut t, 1);
        assert_eq!(t.list.sel.cursor, Some(1));
    }

    #[test]
    fn star_expands_the_siblings() {
        let pol = pol();
        let mut t = tree();
        send(&mut t, &pol, Time::ZERO, &key(Key::ArrowDown));
        let star = Event::Key(KeyEvent { key: None, physical: 0, text: Some("*".into()), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::SHIFT });
        assert_eq!(send(&mut t, &pol, Time::ZERO, &star).handled, Handled::Yes);
        assert!(t.is_expanded(1) && t.is_expanded(2) && !t.is_expanded(3));
    }

    #[test]
    fn a_press_on_the_triangle_toggles_without_selecting() {
        let pol = pol();
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
        send(&mut t, &pol, Time::ZERO, &press(8.0, 12.0));
        assert!(t.is_expanded(1));
        assert!(t.list.sel.selected.is_empty());
        // the label part of row 11 (depth 1) selects
        send(&mut t, &pol, Time::ZERO, &press(60.0, 36.0));
        assert_eq!(t.list.sel.cursor, Some(11));
        // row 11's triangle is one indent in
        send(&mut t, &pol, Time::ZERO, &press(20.0, 36.0));
        assert!(t.is_expanded(11));
    }

    #[test]
    fn a_held_drag_expands_after_the_delay() {
        let pol = pol();
        let delay = ms(pol.drag_expand_ms);
        let mut t = tree();
        let (_, sent) = with_cx(&mut t, &pol, Time::ZERO, |t, cx| t.drag_over(cx, Some(1)));
        assert_eq!(sent.wake, Some(Time::ZERO + delay));
        assert!(!t.is_expanded(1));
        with_cx(&mut t, &pol, Time::ZERO + Span::from_millis(200), |t, cx| t.drag_over(cx, Some(1)));
        assert!(!t.is_expanded(1), "still holding");
        send(&mut t, &pol, Time::ZERO + delay, &Event::Timer);
        assert!(t.is_expanded(1));
        // moving off resets
        with_cx(&mut t, &pol, Time::ZERO, |t, cx| {
            t.drag_over(cx, Some(2));
            t.drag_over(cx, None);
        });
        send(&mut t, &pol, Time::ZERO + Span::from_millis(5000), &Event::Timer);
        assert!(!t.is_expanded(2));
    }

    #[test]
    fn rows_are_tree_items_with_levels() {
        let mut t = tree();
        expand(&mut t, 1);
        let n = t.row_a11y(0).unwrap();
        assert_eq!(n.role(), accesskit::Role::TreeItem);
        assert_eq!(n.level(), Some(1));
        assert_eq!(n.is_expanded(), Some(true));
        let n = t.row_a11y(2).unwrap();
        assert_eq!(n.level(), Some(2));
        assert_eq!(n.is_expanded(), None, "a leaf");
        assert_eq!(n.size_of_set(), Some(5));
    }

    #[test]
    fn the_list_props_are_forwarded_by_index() {
        let (ts, ls) = (TreeView::spec(), VirtualList::spec());
        for i in 0..LIST_PROPS as usize {
            assert_eq!(ts.props[i].name, ls.props[i].name);
        }
        for (i, sig) in ls.signals.iter().enumerate() {
            assert_eq!(ts.signals[i].name, sig.name);
        }
        let mut t = tree();
        t.set_prop(ts.prop("selection").unwrap(), &Value::str("multi")).unwrap();
        assert!(t.list.sel.mode.is_multi());
        t.set_prop(P::expanded_ids.id(), &Value::List(vec![Value::I64(1)])).unwrap();
        assert_eq!(t.list.ids(), &[1, 11, 12, 2, 3]);
        assert_eq!(t.get_prop(P::expanded_ids.id()), Some(Value::List(vec![Value::I64(1)])));
        assert!(t.set_prop(P::indent.id(), &Value::F64(-1.0)).is_err());
    }

    #[test]
    fn records_rows_and_chevrons() {
        let mut t = tree();
        expand(&mut t, 1);
        let (list, _) = testkit::record(&t, Size::new(200.0, 240.0), StateBits::NONE);
        // rows 1, 11 and 2 are expandable: two lines per chevron
        assert_eq!(testkit::kinds(&list).iter().filter(|k| **k == "line").count(), 6);
    }
}
