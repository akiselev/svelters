//! `tree-view` on egui (WG5b; QT-SURVEY P0, EGUI.md §5 Tier 1): a tree drawn by the `virtual-list`
//! machinery ([`super::virtuallist`]). It is the same `W::VirtualList` with a [`Tree`] attached:
//! the M5 protocol (`splice`, `setRow`, `setup`/`bind`/`unbind`/`teardown`), recycled row slots and
//! the row height model are the list's. JS (`@svelters/lists`' `<TreeView>`) keeps the tree and
//! **flattens its visible rows** into the list: expanding a node splices its children in after it,
//! collapsing splices them out. Lazy children are JS's: `expand(id)` reaches `TreeView`, which calls
//! the app's `children(node)` (an array or a Promise) and splices the result.
//!
//! ```text
//! props  selection "none"|"single"|"multi", row-height i64, indent i64 (points per level),
//!        row-drag-type str (non-empty: rows drag), row-drop-types str (non-empty: rows take drops)
//! calls  virtual-list's, plus setNodes(ids[], levels[], flags[], labels[], data[]) (flags: 1
//!        expandable, 2 expanded; a level < 0 forgets the id), setSelection(ids[]), selection()
//! async  expand(id), collapse(id), selection-changed(), activate(pos), copy(),
//!        drop-row(id, where, type, data)
//! ```
//!
//! **Selection** is by id ([`KeySelect`], shared with WG5a's lists): ids filtered out stay selected
//! and come back selected (SV46; `TreeView` keeps an item's id for as long as the view lives).
//! `selection-changed` has no arguments; JS reads `selection()`.
//!
//! **Keyboard** (when the tree has focus: a click on a row, or Tab): Up/Down/PageUp/PageDown/
//! Home/End move (Shift extends, Ctrl moves without selecting), Right expands a collapsed node or
//! goes to its first child, Left collapses an expanded node or goes to its parent, Space toggles
//! the cursor's selection, Enter activates it, Ctrl+C raises `copy` (`TreeView` writes the selected
//! rows to `svelters:clipboard`, the host's `Platform` clipboard), typed text is type-ahead on the
//! row labels. The tree is one EV3 tab stop and claims these keys (`focus::Role::Tree`,
//! [`claims`]), so a host keymap leaves them.
//!
//! **Drag and drop** of rows: with `row-drag-type`, dragging a row starts a [`DragPayload`] of the
//! selected rows (or the dragged row, when it is not selected): the `row-drag-type` entry is the
//! rows' `data` (from `setNodes`) joined by newlines, `text/plain` their labels. Any WG4b target
//! or Rust panel takes it. With `row-drop-types`, a drag over a row shows where it would land
//! (`before`, `after`, or `into` the row: the row's top quarter, bottom quarter, or middle) and the
//! release raises `drop-row(id, where, type, data)` (`id` -1 and `into`: below the last row), the
//! entry taken by [`DragPayload::best`].
//!
//! **A11y**: the list is a `Tree`, rows are `TreeItem`s with their level (1-based), `expanded` for
//! expandable rows, `selected`, and position and set size over the whole flattened list.
//!
//! Writer lane: WG5b.

use super::dnd::DragPayload;
use super::{Def, KindDef, MethodDef, P, PropDef, SignalDef, Ty, prop};
use crate::scene::{NodeId, Scene, W};
use crate::views::selection::{Change, KeySelect, Mode, Mods, Nav};
use crate::walk::Out;
use egui::{CursorIcon, DragAndDrop, Id, Rect, Sense, StrokeKind, Ui};
use std::collections::HashMap;
use svelters_abi::Value;

pub const SELECTION: &[&str] = &["none", "single", "multi"];

pub(super) static PROPS: &[PropDef] = &[
    prop("selection", P::ListSelection, Ty::Enum(SELECTION), Def::Str("single"), "`single` or `multi` (Shift ranges, Ctrl toggles); `none`: rows are not selectable."),
    prop("row-height", P::RowHeight, Ty::I64, Def::I64(24), "The height a row is assumed to have before it is measured, and its minimum, in points."),
    prop("indent", P::TreeIndent, Ty::I64, Def::I64(16), "Points per level; the disclosure triangle sits in the last one."),
    prop("row-drag-type", P::RowDragType, Ty::Str, Def::Str(""), "Non-empty: rows drag, as a payload of this type (their `data`) plus `text/plain` (their labels)."),
    prop("row-drop-types", P::RowDropTypes, Ty::Str, Def::Str(""), "Non-empty: rows take drops of these types (WG4b patterns) and raise `drop-row`."),
];

const ROW: (&str, Ty) = ("row", Ty::I64);
const POS: (&str, Ty) = ("pos", Ty::I64);
const ID: (&str, Ty) = ("id", Ty::I64);

pub(super) static SIGNALS: &[SignalDef] = &[
    SignalDef { name: "setup", args: &[ROW], sync: true, doc: "Row slot `row` was created: give it content with `setRow`." },
    SignalDef { name: "bind", args: &[ROW, POS, ID, ("gen", Ty::I64)], sync: true, doc: "Slot `row` now shows item `id` at `pos`; `gen` counts the binds of the slot." },
    SignalDef { name: "unbind", args: &[ROW, ("gen", Ty::I64)], sync: true, doc: "Slot `row` no longer shows binding `gen`." },
    SignalDef { name: "teardown", args: &[ROW], sync: true, doc: "Slot `row` is gone: release its widget." },
    SignalDef { name: "activate", args: &[POS], sync: false, doc: "The user activated the row at `pos` (double click, Enter)." },
    SignalDef { name: "expand", args: &[ID], sync: false, doc: "The user expanded node `id` (disclosure click, Right): splice its children in." },
    SignalDef { name: "collapse", args: &[ID], sync: false, doc: "The user collapsed node `id` (disclosure click, Left): splice its children out." },
    SignalDef { name: "selection-changed", args: &[], sync: false, doc: "The user changed the selection: read it with `selection()`." },
    SignalDef { name: "copy", args: &[], sync: false, doc: "Ctrl+C on the focused tree: copy the selected rows." },
    SignalDef {
        name: "drop-row",
        args: &[ID, ("where", Ty::Str), ("type", Ty::Str), ("data", Ty::Str)],
        sync: false,
        doc: "A drag was dropped `before`, `after` or `into` row `id` (-1: below the last row), as the entry of `type` it took.",
    },
];

pub(super) static METHODS: &[MethodDef] = &[
    MethodDef { name: "splice", doc: "`splice(pos, removed, ids)`: replaces `removed` rows at `pos` with `ids` (the visible rows, flattened)." },
    MethodDef { name: "count", doc: "The number of visible rows." },
    MethodDef { name: "scrollTo", doc: "`scrollTo(pos)`: scrolls the row into view." },
    MethodDef { name: "setRow", doc: "`setRow(row, widget)`: the content of row slot `row` (called from `setup`); null clears it." },
    MethodDef { name: "rowAt", doc: "`rowAt(pos)`: the slot bound to `pos`, or -1." },
    MethodDef { name: "setNodes", doc: "`setNodes(ids, levels, flags, labels, data)`: the tree facts of rows by id (flags: 1 expandable, 2 expanded; a level < 0 forgets the id)." },
    MethodDef { name: "setSelection", doc: "`setSelection(ids)`: replaces the selection (single keeps the first); raises nothing." },
    MethodDef { name: "selection", doc: "The selected ids, hidden ones included." },
];

pub(super) const DEF: KindDef = KindDef {
    id: "TreeView",
    tag: Some("tree-view"),
    native: "svelters_egui::kinds::treeview (a virtual-list with tree rows)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: PROPS,
    signals: SIGNALS,
    methods: METHODS,
    doc: "A virtualized tree: JS keeps the tree and flattens its visible rows into the list; children load when a node expands.",
};

/// Expandable (a disclosure triangle).
pub const EXPANDABLE: i64 = 1;
/// Expanded (its children follow it).
pub const EXPANDED: i64 = 2;

/// What the tree knows of one row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeNode {
    /// 0 for roots.
    pub level: u32,
    pub expandable: bool,
    pub expanded: bool,
    /// Type-ahead, the drag's `text/plain`.
    pub label: String,
    /// The drag's `row-drag-type` entry.
    pub data: String,
}

/// Where a drop lands relative to a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAt {
    Before,
    Into,
    After,
}

impl DropAt {
    pub fn name(self) -> &'static str {
        match self {
            DropAt::Before => "before",
            DropAt::Into => "into",
            DropAt::After => "after",
        }
    }

    /// From the pointer's height `t` (0 top, 1 bottom) within a row.
    pub fn of(t: f32) -> DropAt {
        if t < 0.25 {
            DropAt::Before
        } else if t > 0.75 {
            DropAt::After
        } else {
            DropAt::Into
        }
    }
}

/// A `tree-view`'s state, on its `VList`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub nodes: HashMap<u64, TreeNode>,
    pub sel: KeySelect,
    pub indent: f32,
    pub drag_type: String,
    pub drop_types: String,
}

impl Default for Tree {
    fn default() -> Self {
        Tree { nodes: HashMap::new(), sel: KeySelect::new(Mode::Single), indent: 16.0, drag_type: String::new(), drop_types: String::new() }
    }
}

/// A key the tree handles, decoded from egui's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKey {
    Nav(Nav),
    Left,
    Right,
    Space,
    Enter,
    Copy,
}

impl TreeKey {
    /// The tree's key for an egui key press, if it is one.
    pub fn of(key: egui::Key, m: egui::Modifiers) -> Option<TreeKey> {
        use egui::Key as K;
        Some(match key {
            K::ArrowUp => TreeKey::Nav(Nav::Up),
            K::ArrowDown => TreeKey::Nav(Nav::Down),
            K::PageUp => TreeKey::Nav(Nav::PageUp),
            K::PageDown => TreeKey::Nav(Nav::PageDown),
            K::Home => TreeKey::Nav(Nav::Home),
            K::End => TreeKey::Nav(Nav::End),
            K::ArrowLeft => TreeKey::Left,
            K::ArrowRight => TreeKey::Right,
            K::Space => TreeKey::Space,
            K::Enter => TreeKey::Enter,
            K::C if m.command => TreeKey::Copy,
            _ => return None,
        })
    }
}

/// What a key did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyOut {
    pub change: Change,
    pub expand: Option<u64>,
    pub collapse: Option<u64>,
    pub activate: Option<u64>,
    pub copy: bool,
}

impl Tree {
    /// A fresh tree with the same props (a cloned node).
    pub fn fresh(&self) -> Tree {
        Tree { sel: KeySelect::new(self.sel.mode), indent: self.indent, drag_type: self.drag_type.clone(), drop_types: self.drop_types.clone(), ..Tree::default() }
    }

    pub fn level(&self, id: u64) -> u32 {
        self.nodes.get(&id).map_or(0, |n| n.level)
    }

    /// The points before a row's content: its level's indent plus the disclosure gutter.
    pub fn inset(&self, id: u64) -> f32 {
        (self.level(id) + 1) as f32 * self.indent
    }

    /// The row before `id` with a smaller level.
    pub fn parent(&self, ids: &[u64], id: u64) -> Option<u64> {
        let p = ids.iter().position(|x| *x == id)?;
        let l = self.level(id);
        ids[..p].iter().rev().copied().find(|x| self.level(*x) < l)
    }

    /// The row after `id`, when it is deeper (its first child, shown).
    pub fn first_child(&self, ids: &[u64], id: u64) -> Option<u64> {
        let p = ids.iter().position(|x| *x == id)?;
        ids.get(p + 1).copied().filter(|x| self.level(*x) > self.level(id))
    }

    /// `setNodes`.
    pub fn set_nodes(&mut self, ids: &[u64], levels: &[i64], flags: &[i64], labels: &[String], data: &[String]) {
        for (k, &id) in ids.iter().enumerate() {
            let level = levels.get(k).copied().unwrap_or(0);
            if level < 0 {
                self.nodes.remove(&id);
                continue;
            }
            let f = flags.get(k).copied().unwrap_or(0);
            self.nodes.insert(
                id,
                TreeNode {
                    level: level as u32,
                    expandable: f & EXPANDABLE != 0,
                    expanded: f & EXPANDED != 0,
                    label: labels.get(k).cloned().unwrap_or_default(),
                    data: data.get(k).cloned().unwrap_or_default(),
                },
            );
        }
    }

    /// A key press, over the visible rows `ids`; `page` rows per PageUp/PageDown.
    pub fn key(&mut self, ids: &[u64], k: TreeKey, m: Mods, page: usize) -> KeyOut {
        let mut o = KeyOut::default();
        let cur = self.sel.cursor.filter(|c| ids.contains(c));
        match k {
            TreeKey::Nav(n) => o.change = self.sel.nav(ids, n, m, page),
            TreeKey::Left | TreeKey::Right if cur.is_none() => o.change = self.sel.nav(ids, Nav::Home, Mods::default(), page),
            TreeKey::Right => {
                let c = cur.unwrap();
                let n = self.nodes.get(&c);
                if n.is_some_and(|n| n.expandable && !n.expanded) {
                    o.expand = Some(c);
                } else if let Some(child) = self.first_child(ids, c) {
                    o.change = self.sel.go_to(ids, child, m);
                }
            }
            TreeKey::Left => {
                let c = cur.unwrap();
                if self.nodes.get(&c).is_some_and(|n| n.expandable && n.expanded) {
                    o.collapse = Some(c);
                } else if let Some(p) = self.parent(ids, c) {
                    o.change = self.sel.go_to(ids, p, m);
                }
            }
            TreeKey::Space => o.change = self.sel.toggle_cursor(),
            TreeKey::Enter => o.activate = cur,
            TreeKey::Copy => o.copy = !self.sel.selected.is_empty(),
        }
        o
    }

    /// Type-ahead on the row labels.
    pub fn type_ahead(&mut self, ids: &[u64], text: &str, now: f64) -> Change {
        let Tree { nodes, sel, .. } = self;
        sel.type_ahead(ids, text, now, |id| nodes.get(&id).map(|n| n.label.clone()).unwrap_or_default())
    }

    /// The rows a drag starting on `id` carries: the selection (visible rows, in order) when `id`
    /// is selected, else `id` alone.
    pub fn drag_rows(&self, ids: &[u64], id: u64) -> Vec<u64> {
        if self.sel.is_selected(id) { ids.iter().copied().filter(|x| self.sel.is_selected(*x)).collect() } else { vec![id] }
    }

    /// The payload of rows `rows`: `row-drag-type` (their data, a line each) and `text/plain`
    /// (their labels).
    pub fn mime(&self, rows: &[u64]) -> svelters_host::platform::MimeData {
        let join = |f: fn(&TreeNode) -> &str| rows.iter().map(|r| self.nodes.get(r).map_or("", f)).collect::<Vec<_>>().join("\n");
        let mut m = svelters_host::platform::MimeData::new();
        if !self.drag_type.is_empty() {
            m = m.with(self.drag_type.clone(), join(|n| &n.data));
        }
        m.with("text/plain", join(|n| &n.label))
    }
}

// ---- the walk (called from `virtuallist::paint`) ----

/// Raises signal `name` of the tree being painted, when listened.
pub(crate) type Emit<'a> = dyn Fn(&Scene, &mut Vec<Out>, &str, Vec<Value>) + 'a;

/// One tree row: `full` is the row across the whole list (the content sits at its inset). The
/// row senses clicks (selection, focus, double click: `activate`) and drags (`row-drag-type`);
/// the disclosure triangle in the gutter raises `expand`/`collapse`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tree_row(s: &mut Scene, i: usize, key: u64, pos: usize, len: usize, full: Rect, indent: f32, drag_type: &str, focused: bool, row_id: Id, list_id: Id, ui: &mut Ui, out: &mut Vec<Out>, emit: &Emit<'_>) {
    let ctx = ui.ctx().clone();
    let (node, mode, is_sel, is_cursor) = {
        let W::VirtualList(v) = s.w(i) else { return };
        let Some(t) = v.tree.as_deref() else { return };
        (t.nodes.get(&key).cloned().unwrap_or_default(), t.sel.mode, t.sel.is_selected(key), t.sel.cursor == Some(key))
    };
    let selectable = mode != Mode::None;
    let drags = !drag_type.is_empty();
    let sense = match (selectable, drags) {
        (true, true) => Sense::click_and_drag(),
        (true, false) => Sense::click(),
        (false, true) => Sense::drag(),
        (false, false) => Sense::hover(),
    };
    let rresp = ui.interact(full, row_id, sense);
    crate::scene::virt::a11y_row(&ctx, row_id, pos, len, |n| {
        n.set_role(egui::accesskit::Role::TreeItem);
        n.set_level(node.level as usize + 1);
        if node.expandable {
            n.set_expanded(node.expanded);
        }
        if selectable {
            n.set_selected(is_sel);
        }
    });
    if is_sel {
        ui.painter().rect_filled(full, 0.0, ui.visuals().selection.bg_fill);
    } else if rresp.hovered() {
        ui.painter().rect_filled(full, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    if focused && is_cursor {
        ui.painter().rect_stroke(full.shrink(0.5), 0.0, ui.visuals().selection.stroke, StrokeKind::Inside);
    }
    if node.expandable {
        let side = indent.min(full.height()).max(1.0);
        let g = Rect::from_center_size(egui::pos2(full.min.x + (node.level as f32 + 0.5) * indent, full.center().y), egui::vec2(side, side));
        let tr = ui.interact(g, row_id.with("disclosure"), Sense::click());
        egui::collapsing_header::paint_default_icon(ui, if node.expanded { 1.0 } else { 0.0 }, &tr);
        if tr.clicked() {
            emit(s, out, if node.expanded { "collapse" } else { "expand" }, vec![Value::I64(key as i64)]);
            ctx.request_repaint();
        }
    }
    if rresp.clicked() || rresp.drag_started() {
        ctx.memory_mut(|m| m.request_focus(list_id));
    }
    if selectable && rresp.clicked() {
        let m = ui.input(|inp| inp.modifiers);
        let changed = {
            let W::VirtualList(v) = s.w_mut(i) else { return };
            let v: &mut super::VList = v;
            let Some(t) = v.tree.as_deref_mut() else { return };
            t.sel.click(&v.ids, key, Mods { shift: m.shift, ctrl: m.command }).selection
        };
        if changed {
            emit(s, out, "selection-changed", vec![]);
        }
        ctx.request_repaint();
    }
    if rresp.double_clicked() {
        emit(s, out, "activate", vec![Value::I64(pos as i64)]);
    }
    if drags {
        if rresp.drag_started() {
            let W::VirtualList(v) = s.w(i) else { return };
            let Some(t) = v.tree.as_deref() else { return };
            let mut p = DragPayload::from_mime(t.mime(&t.drag_rows(&v.ids, key)));
            p.source = Some(list_id);
            DragAndDrop::set_payload(&ctx, p);
        }
        if rresp.dragged() {
            ctx.set_cursor_icon(if super::dnd::over_target(&ctx) { CursorIcon::Grabbing } else { CursorIcon::NoDrop });
            super::dnd::ghost(ui, row_id, if node.label.is_empty() { drag_type } else { &node.label });
        }
    }
}

/// `row-drop-types`: when an accepted drag is over the rows, where it would land (and the row's
/// rect from its inset, `None` below the last row); on release raises `drop-row`. Runs before the
/// rows are painted, so drop targets inside rows (painted later) win the pointer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tree_drop_target(s: &Scene, i: usize, placed: &[(NodeId, usize)], origin: egui::Vec2, r: Rect, clip: Rect, drop_types: &str, ui: &Ui, out: &mut Vec<Out>, emit: &Emit<'_>) -> Option<(DropAt, Option<Rect>)> {
    let ctx = ui.ctx().clone();
    let zone = s.core[i].egui_id.with("row-drop");
    DragAndDrop::payload::<DragPayload>(&ctx).filter(|p| p.best(drop_types).is_some())?;
    if !ctx.rect_contains_pointer(ui.layer_id(), clip) {
        return None;
    }
    super::dnd::claim(&ctx, zone);
    ctx.request_repaint(); // the claim decides next pass
    if !super::dnd::claimed(&ctx, zone) {
        return None;
    }
    let ptr = ctx.pointer_latest_pos()?;
    let W::VirtualList(v) = s.w(i) else { return None };
    let mut hit: Option<(i64, DropAt, Rect)> = None;
    for &(n, pos) in placed {
        let Some(ni) = s.idx(n) else { continue };
        let rr = s.core[ni].rect.translate(origin);
        if pos < v.len() && rr.y_range().contains(ptr.y) {
            let t = (ptr.y - rr.min.y) / rr.height().max(1.0);
            let x = (r.min.x + v.row_inset(pos) - v.tree.as_ref().map_or(0.0, |t| t.indent)).max(r.min.x);
            hit = Some((v.key_at(pos) as i64, DropAt::of(t), Rect::from_x_y_ranges(x..=r.max.x, rr.y_range())));
        }
    }
    if ctx.input(|inp| inp.pointer.any_released())
        && let Some(p) = DragAndDrop::take_payload::<DragPayload>(&ctx)
    {
        p.accept();
        let (ty, data) = p.best(drop_types).map(|(t, d)| (t.to_string(), d)).unwrap_or_default();
        let (id, at) = hit.map_or((-1, DropAt::Into), |h| (h.0, h.1));
        emit(s, out, "drop-row", vec![Value::I64(id), Value::Str(at.name().into()), Value::Str(ty), Value::Str(data)]);
    }
    Some(hit.map_or((DropAt::Into, None), |h| (h.1, Some(h.2))))
}

/// EV3 (`focus::Role::Tree`): the keys a focused tree takes, its [`TreeKey`]s and the keys that
/// type a character (type-ahead). Alt chords stay the app's.
pub(crate) fn claims(key: egui::Key, m: egui::Modifiers) -> bool {
    if m.alt {
        return false;
    }
    if TreeKey::of(key, m).is_some() {
        return true;
    }
    let s = key.symbol_or_name();
    !(m.command || m.ctrl || m.mac_cmd) && s.len() == 1 && s.as_bytes()[0].is_ascii_graphic()
}

/// The focused tree's input this pass: its key presses (EV3's `focus::record_with`, called right
/// after with `Role::Tree`, takes them out of the input and reports them to the host keymap) and
/// its type-ahead text and `Copy` events (taken out here). The arrows are also kept from egui's
/// directional focus moves (egui's focus lock filter: it acts in `begin_pass`, before EV3).
pub(crate) fn tree_input(list_id: Id, ui: &mut Ui) -> (Vec<(TreeKey, egui::Modifiers)>, String) {
    ui.ctx().memory_mut(|m| m.set_focus_lock_filter(list_id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
    let mut keys = Vec::new();
    let mut text = String::new();
    ui.input_mut(|inp| {
        inp.events.retain(|e| match e {
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                if let Some(k) = TreeKey::of(*key, *modifiers).filter(|_| claims(*key, *modifiers)) {
                    keys.push((k, *modifiers));
                }
                true
            }
            egui::Event::Copy => {
                keys.push((TreeKey::Copy, egui::Modifiers::COMMAND));
                false
            }
            egui::Event::Text(t) => {
                if !t.trim().is_empty() {
                    text.push_str(t);
                }
                false
            }
            _ => true,
        })
    });
    (keys, text)
}

/// Applies the focused tree's input ([`tree_input`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tree_keys(s: &mut Scene, i: usize, keys: Vec<(TreeKey, egui::Modifiers)>, text: String, ctx: &egui::Context, ui: &mut Ui, out: &mut Vec<Out>, emit: &Emit<'_>) {
    if keys.is_empty() && text.is_empty() {
        return;
    }
    let now = ui.input(|inp| inp.time);
    let mut outs = Vec::new();
    {
        let W::VirtualList(v) = s.w_mut(i) else { return };
        let v: &mut super::VList = v;
        let page = ((v.view.y / v.heights.estimate().max(1.0)) as usize).max(1);
        let Some(t) = v.tree.as_deref_mut() else { return };
        for (k, m) in keys {
            outs.push(t.key(&v.ids, k, Mods { shift: m.shift, ctrl: m.command }, page));
        }
        if !text.is_empty() {
            outs.push(KeyOut { change: t.type_ahead(&v.ids, &text, now), ..KeyOut::default() });
        }
        let cursor = t.sel.cursor.and_then(|c| v.ids.iter().position(|x| *x == c));
        if outs.iter().any(|o| o.change.cursor) {
            v.reveal = cursor;
        }
        for o in &mut outs {
            // `activate` carries a position
            o.activate = o.activate.and_then(|a| v.ids.iter().position(|x| *x == a)).map(|p| p as u64);
        }
    }
    for o in &outs {
        if let Some(id) = o.expand {
            emit(s, out, "expand", vec![Value::I64(id as i64)]);
        }
        if let Some(id) = o.collapse {
            emit(s, out, "collapse", vec![Value::I64(id as i64)]);
        }
        if let Some(p) = o.activate {
            emit(s, out, "activate", vec![Value::I64(p as i64)]);
        }
        if o.copy {
            emit(s, out, "copy", vec![]);
        }
    }
    if outs.iter().any(|o| o.change.selection) {
        emit(s, out, "selection-changed", vec![]);
    }
    s.invalidate_content(s.slots.id_at(i).expect("live list"));
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a(0) > b(1) > c(2); d(0) collapsed
    fn tree() -> (Tree, Vec<u64>) {
        let mut t = Tree { sel: KeySelect::new(Mode::Multi), ..Tree::default() };
        let s = |x: &str| x.to_string();
        t.set_nodes(&[1, 2, 3, 4], &[0, 1, 2, 0], &[3, 3, 0, 1], &[s("alpha"), s("beta"), s("gamma"), s("delta")], &[s("a"), s("b"), s("c"), s("d")]);
        (t, vec![1, 2, 3, 4])
    }

    #[test]
    fn right_expands_then_enters_left_collapses_then_leaves() {
        let (mut t, ids) = tree();
        let m = Mods::default();
        t.key(&ids, TreeKey::Nav(Nav::End), m, 1);
        assert_eq!(t.sel.cursor, Some(4));
        assert_eq!(t.key(&ids, TreeKey::Right, m, 1).expand, Some(4), "collapsed: expand");
        t.sel.go_to(&ids, 2, m);
        assert_eq!(t.key(&ids, TreeKey::Right, m, 1).expand, None);
        assert_eq!(t.sel.cursor, Some(3), "expanded: first child");
        let o = t.key(&ids, TreeKey::Left, m, 1);
        assert_eq!((o.collapse, t.sel.cursor), (None, Some(2)), "a leaf: to its parent");
        assert_eq!(t.key(&ids, TreeKey::Left, m, 1).collapse, Some(2), "expanded: collapse");
        assert!(t.sel.is_selected(2));
    }

    #[test]
    fn levels_inset_type_ahead_and_drag_rows() {
        let (mut t, ids) = tree();
        assert_eq!(t.inset(3), 48.0);
        assert_eq!(t.parent(&ids, 4), None);
        assert!(t.type_ahead(&ids, "g", 0.0).cursor);
        assert_eq!(t.sel.cursor, Some(3));
        assert_eq!(t.drag_rows(&ids, 1), vec![1]);
        t.sel.set([3, 1]);
        assert_eq!(t.drag_rows(&ids, 3), vec![1, 3]);
        t.drag_type = "cad/part".into();
        let m = t.mime(&[1, 3]);
        assert_eq!(m.best(&["cad/*"]).map(|(t, d)| (t.to_string(), d.to_vec())), Some(("cad/part".into(), b"a\nc".to_vec())));
        assert_eq!(m.best(&["text/plain"]).map(|(_, d)| d.to_vec()), Some(b"alpha\ngamma".to_vec()));
        t.set_nodes(&[3], &[-1], &[], &[], &[]);
        assert!(!t.nodes.contains_key(&3));
        assert_eq!(DropAt::of(0.1), DropAt::Before);
        assert_eq!(DropAt::of(0.5).name(), "into");
    }
}
