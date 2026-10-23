//! Tree edits through one path (ARCH-SCENE §6.4, §12): create, insert, remove, destroy and the
//! subtree free of a native close, for every origin. `ops.rs` calls them with [`Origin::Js`] and
//! `SceneMut` with the Rust origin, so the kind checks, the owner checks (S3: Rust edits only Rust-
//! and Host-owned structure), the cycle check, the root cache and the invalidation are in one place.
//!
//! Also the root table (§4.5): every window is a root from its creation; EM1 adds slot roots and
//! SG4 overlay roots through [`Scene::add_root`]. A node caches its root (`NodeCore::root`), set for
//! the whole subtree when it moves between roots (O(subtree), once per such insert).
//!
//! [`Scene::attached`] is the attach hook (called for each node that enters a root): it derives the
//! node's egui id from its root's id base (EM1, ARCH-EMBED §4.4, `ids.rs`).
//!
//! SG2: every edit here is journaled (`Created`, `Inserted`, `Removed`, `Destroyed`, `Root`), and a
//! freed node's extension entries are dropped with an `Ext { Removed }` record each (R19).
//! Writer lane: SCENE.

use super::access::{Origin, SceneError};
use super::dirty::RootDirty;
use super::id::{KindId, NodeId, RootIx};
use super::journal::{Change, ExtChange, RootChange};
use super::node::{Dirty, LayoutBox, NodeCore, NodeFlags, Owner, Payload, W};
use super::{Common, Scene};
use crate::kinds;

/// What a root is (ARCH-SCENE §4.5).
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TreeRoot {
    /// A `window` node: a desktop window of the runner, or one shown in kittest.
    Window { node: NodeId },
    /// An embed slot (EM1, ARCH-EMBED §4.1): a host-owned box, drawn by `Embed::show_slot`; the
    /// root mounted into it is the runtime's subtree under `node`.
    Slot { node: NodeId },
    /// An overlay root (SG4, ARCH-SCENE §7.5): an embed slot drawn by `Embed::show_overlay` in an
    /// `egui::Area` of order `layer` over other content (a 3D viewport), click-through except where
    /// one of its nodes takes input.
    Overlay { node: NodeId, layer: egui::Order },
}

impl TreeRoot {
    /// The top node of a slot or overlay root (the embed's `SlotId`), `None` for a window.
    pub fn slot_node(self) -> Option<NodeId> {
        match self {
            TreeRoot::Slot { node } | TreeRoot::Overlay { node, .. } => Some(node),
            TreeRoot::Window { .. } => None,
        }
    }
}

pub(crate) struct RootEntry {
    pub kind: TreeRoot,
    pub top: NodeId,
    pub dirty: RootDirty,
    /// The available size of its last layout (`None`: never laid out).
    pub size: Option<egui::Vec2>,
    /// The egui id base of the root mounted here (EM1: `ids::root_base` of the embed's salt and the
    /// root's key); `None`: nodes keep the unsalted `ids::node_id` (windows, empty slots).
    pub base: Option<egui::Id>,
    /// The viewport it is drawn in (SG2: damage routing; `Scene::root_changed` keeps it).
    pub viewport: egui::ViewportId,
    /// Shown (the embed's `set_visible`, through `Scene::root_changed`; SG3: hit-testing).
    pub visible: bool,
    /// The key of the root mounted here (the embed's `RootKey`; SG3: `SceneRef::by_path`).
    pub key: Option<Box<str>>,
    /// Anchor → the nodes of this root carrying it (SG3, `query.rs`).
    pub anchors: std::collections::HashMap<Box<str>, Vec<NodeId>>,
    /// SG4: what its last layouts laid out (boundaries, or the top), until `ScenePass::after_layout`
    /// sees it at the root's draw.
    pub laid: Vec<NodeId>,
}

impl RootDirty {
    /// No layout work queued.
    pub fn is_empty(&self) -> bool {
        !self.full && self.relayout.is_empty() && self.remeasure.is_empty()
    }
}

impl RootEntry {
    pub fn laid_out(&self) -> bool {
        self.size.is_some()
    }
}

#[derive(Default)]
pub(crate) struct RootTable {
    entries: Vec<Option<RootEntry>>,
    free: Vec<u32>,
}

impl RootTable {
    pub fn get(&self, r: RootIx) -> Option<&RootEntry> {
        self.entries.get(r.0 as usize).and_then(Option::as_ref)
    }
    pub fn get_mut(&mut self, r: RootIx) -> Option<&mut RootEntry> {
        self.entries.get_mut(r.0 as usize).and_then(Option::as_mut)
    }
    fn add(&mut self, e: RootEntry) -> RootIx {
        match self.free.pop() {
            Some(i) => {
                self.entries[i as usize] = Some(e);
                RootIx(i)
            }
            None => {
                self.entries.push(Some(e));
                RootIx(self.entries.len() as u32 - 1)
            }
        }
    }
    fn remove(&mut self, r: RootIx) -> Option<RootEntry> {
        let e = self.entries.get_mut(r.0 as usize)?.take()?;
        self.free.push(r.0);
        Some(e)
    }
    pub fn iter(&self) -> impl Iterator<Item = (RootIx, &RootEntry)> {
        self.entries.iter().enumerate().filter_map(|(i, e)| e.as_ref().map(|e| (RootIx(i as u32), e)))
    }
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }
}

/// What `destroy` leaves for the caller's own cleanup (egui memory, window lists).
pub(crate) struct Destroyed {
    pub egui_id: egui::Id,
    pub was_window: bool,
}

fn builtin_payload(kind: usize) -> Option<W> {
    Some(match kinds::kind(kind).id {
        "Window" => W::Window { title: String::new(), default_w: 480.0, default_h: 360.0 },
        "Box" => W::Box,
        "Label" => W::Label { text: String::new(), wrap: "wrap", selectable: false, weight: 400, rich: None },
        // RT2 (`kinds/rich.rs`)
        "Span" | "Bold" | "Italic" | "Code" | "Anchor" => W::Run { text: String::new(), weight: 0, href: String::new(), anchor: kinds::kind(kind).id == "Anchor" },
        "Button" => W::Button { text: String::new(), selected: false, frame: true, small: false, weight: 400 },
        "Checkbox" => W::Checkbox { text: String::new(), checked: false },
        "TextEdit" => W::TextEdit { text: String::new(), hint: String::new(), multiline: false, password: false, char_limit: 0 },
        "VirtualList" => W::VirtualList(Box::default()),
        "TreeView" => W::VirtualList(Box::new(crate::kinds::VList::tree())),
        // WG1: the defaults match the kind tables (`kinds/builtin.rs`, `kinds/containers.rs`)
        "ToggleButton" => W::ToggleButton { text: String::new(), checked: false, radio: false, frame: true, weight: 400 },
        "ComboBox" => W::ComboBox { options: Vec::new(), selected: 0, hint: String::new() },
        "Separator" => W::Separator { vertical: false },
        "Icon" => W::Icon { name: String::new(), glyph: String::new(), family: "icons".into(), size: None, weight: 400, frame: false, selected: false, source: String::new(), tint: true },
        "Grid" => W::Grid { columns: "auto 1fr".into(), justify_items: "stretch" },
        "CollapsingHeader" => W::CollapsingHeader { text: String::new(), open: false },
        // WG2a: the defaults match the kind tables (`kinds/menus.rs`, `kinds/popups.rs`)
        id @ ("MenuEntry" | "MenuBar" | "Menu" | "Action" | "MenuSeparator" | "ContextMenu" | "MenuButton") => W::Menu(crate::menus::payload(id)),
        id @ ("Popup" | "Modal" | "InnerWindow") => W::Overlay(Box::new(crate::overlays::payload(id))),
        // WG2b: the defaults match the kind tables (`kinds/{inputs,panes,media}.rs`)
        "Hyperlink" | "RadioButton" | "Switch" | "RangeSlider" | "DragValue" | "Sides" | "Frame" | "Stack" | "Image" | "ProgressBar" | "Spinner" | "Canvas" => {
            crate::elements::payload(kinds::kind(kind).id)?
        }
        // WG4a: the defaults match `kinds/views.rs`
        "Table" => W::Table(Box::default()),
        "TableColumn" => W::TableColumn(Box::default()),
        "TabView" => W::TabView { selected: 0 },
        "Tab" => W::Tab { title: String::new(), closable: false },
        "ToastHost" => W::ToastHost(Box::default()),
        "Toast" => W::Toast(Box::default()),
        // RA1: a provider kind's widget struct with its defaults
        _ => W::Ext(crate::provider::ExtWidget::new((kinds::KindRegistry::global().provider(kind)?.entry.new)())),
    })
}

fn is_js(o: Origin) -> bool {
    matches!(o, Origin::Js)
}

/// The origin of an owner's writes.
fn origin_of(o: Owner) -> Origin {
    match o {
        Owner::Js => Origin::Js,
        Owner::Rust(p) => Origin::Rust(p),
        Owner::Host => Origin::Host,
    }
}

impl Scene {
    /// Creates a detached node of kind index `kind` (a window: a new root of its own).
    pub(crate) fn create(&mut self, kind: usize, owner: Owner) -> Result<NodeId, SceneError> {
        let tag = (kind < kinds::KindRegistry::global().len()).then(|| kinds::kind(kind).tag).flatten().ok_or_else(|| SceneError::Invalid(format!("unknown kind index {kind}")))?;
        // OW0: a built-in kind whose own widget is switched on (`crate::widgets`) gets it
        let own = crate::widgets::own_payload(&self.own, kind);
        let mut w = own.or_else(|| builtin_payload(kind)).ok_or_else(|| SceneError::Invalid(format!("<{tag}> is not creatable")))?;
        crate::widgets::mark_own(&self.own, tag, &mut w); // OW3: menu roots and `popup`
        let (id, fresh) = self.slots.alloc();
        let tick = self.stamp();
        let i = id.index() as usize;
        // SG6: a window's padding is its only non-default common prop at creation (interned)
        let common = matches!(w, W::Window { .. }).then(|| {
            let c = Common { padding: 8.0, ..Common::DEFAULT };
            let k = super::compact::common_key(&c);
            self.commons.intern(c, k)
        });
        let style = crate::layout::style_of(common.as_deref().unwrap_or(&Common::DEFAULT), &w, &super::Visual::IDENTITY);
        let lb = LayoutBox::new(self.intern_style(style));
        let mut core = NodeCore::new(kind as u32, owner, crate::ids::node_id(id.to_raw()), tick);
        if matches!(w, W::VirtualList(_)) {
            // LS1: a list's size never depends on its rows; scrolling relays out the list alone
            core.flags.insert(NodeFlags::LAYOUT_BOUNDARY);
        }
        if fresh {
            // SG6: the columns grow by a quarter, not double (ARCH-SCENE §8.5 counts capacity)
            fn grow<T>(v: &mut Vec<T>) {
                if v.len() == v.capacity() {
                    v.reserve_exact((v.capacity() / 4).max(64));
                }
            }
            grow(&mut self.core);
            grow(&mut self.children);
            grow(&mut self.layout);
            grow(&mut self.payload);
            grow(&mut self.cold);
            grow(&mut self.common);
            self.core.push(core);
            self.children.push(Vec::new());
            self.layout.push(lb);
            self.payload.push(Payload::Builtin(w));
            self.cold.push(None);
            self.common.push(common);
        } else {
            self.core[i] = core;
            self.children[i] = Vec::new();
            self.layout[i] = lb;
            self.payload[i] = Payload::Builtin(w);
            self.cold[i] = None;
            self.common[i] = common;
        }
        self.index.add(kind as u32, id);
        self.record(Change::Created { node: id, kind: KindId(kinds::kind_id(kind)), owner }, origin_of(owner));
        if kinds::kind(kind).toplevel {
            self.add_root(TreeRoot::Window { node: id }, id);
        }
        self.counters.created += 1;
        Ok(id)
    }

    /// Makes the parentless node `top` the top of a new root. EM1 uses it for slot roots.
    pub(crate) fn add_root(&mut self, kind: TreeRoot, top: NodeId) -> RootIx {
        let ti = self.ix(top);
        debug_assert!(self.core[ti].parent.is_none() && self.core[ti].root.is_detached());
        let r = self.roots.add(RootEntry {
            kind,
            top,
            dirty: RootDirty { full: true, ..Default::default() },
            size: None,
            base: None,
            viewport: egui::ViewportId::ROOT,
            visible: true,
            key: None,
            anchors: Default::default(),
            laid: Vec::new(),
        });
        self.core[ti].flags.insert(NodeFlags::ROOT);
        self.set_root_subtree(top, r);
        self.record(Change::Root { root: r, what: RootChange::Added }, Origin::Host);
        r
    }

    /// Something about root `r` changed that the embed owns (visibility, viewport): journals it, and
    /// keeps the root's viewport (damage routing, R20) and visibility (hit-testing). The embed's
    /// `set_visible`/`set_viewport` call it.
    pub(crate) fn root_changed(&mut self, r: RootIx, what: RootChange) {
        let Some(e) = self.roots.get_mut(r) else { return };
        match what {
            RootChange::Viewport(vp) => e.viewport = vp,
            RootChange::Visible(v) => e.visible = v,
            _ => {}
        }
        self.record(Change::Root { root: r, what }, Origin::Host);
    }

    /// Sets the key of the root mounted in root `r` (`SceneRef::by_path`); `None` when unmounted.
    pub(crate) fn set_root_key(&mut self, r: RootIx, key: Option<&str>) {
        if let Some(e) = self.roots.get_mut(r) {
            e.key = key.map(Box::from);
        }
    }

    /// The entry of the root whose top is the live node `h` (a slot's or a window's own root).
    pub(crate) fn slot_root(&self, h: super::Handle) -> Option<&RootEntry> {
        let id = NodeId::from_raw(h)?;
        let i = self.idx(id)?;
        self.roots.get(self.core[i].root).filter(|e| e.top == id)
    }

    /// Ends root `r`: its tree stays, detached.
    pub(crate) fn remove_root(&mut self, r: RootIx) {
        if self.roots.get(r).is_none() {
            return;
        }
        self.record(Change::Root { root: r, what: RootChange::Removed }, Origin::Host);
        let Some(e) = self.roots.remove(r) else { return };
        if let Some(ti) = self.idx(e.top) {
            self.core[ti].flags.remove(NodeFlags::ROOT);
            self.set_root_subtree(e.top, RootIx::DETACHED);
        }
    }

    /// The attach hook: `id` just entered root `r`. Its egui id derives from the root's id base
    /// (ARCH-EMBED §4.4): `base.with(("a", anchor))` when anchored, else `base.with(("h", handle))`;
    /// in a root without a base, the unsalted `ids::node_id`. Host-owned nodes (slots) keep the id
    /// the embed gave them. Deterministic, so a move within the root keeps the id.
    #[inline]
    pub(crate) fn attached(&mut self, id: NodeId, r: RootIx) {
        let i = self.ix(id);
        if self.core[i].owner == Owner::Host {
            return;
        }
        let h = id.to_raw();
        self.core[i].egui_id = match self.roots.get(r).and_then(|e| e.base) {
            Some(base) => crate::ids::rooted_id(base, h, &self.common(i).anchor),
            None => crate::ids::node_id(h),
        };
    }

    /// Sets the egui id base of root `r` (EM1: a root mounted into a slot, `None` when unmounted).
    /// Nodes attached from now on take ids from it; the ones already in the root are re-derived.
    pub(crate) fn set_root_base(&mut self, r: RootIx, base: Option<egui::Id>) {
        let Some(e) = self.roots.get_mut(r) else { return };
        if e.base == base {
            return;
        }
        e.base = base;
        let top = e.top;
        let mut stack = vec![top];
        while let Some(x) = stack.pop() {
            self.attached(x, r);
            stack.extend(self.children[self.ix(x)].iter().copied());
        }
    }

    /// Sets the root cache of `top`'s subtree to `r`, moving queued work along (a queued node keeps
    /// its queue membership in an attached root; in a detached tree its work is dropped and its
    /// caches cleared up to the top, so it is laid out fresh when it is attached again).
    pub(crate) fn set_root_subtree(&mut self, top: NodeId, r: RootIx) {
        let mut stack = vec![top];
        let mut requeue: Vec<(NodeId, NodeFlags)> = Vec::new();
        while let Some(x) = stack.pop() {
            let xi = self.ix(x);
            let old = self.core[xi].root;
            self.core[xi].root = r;
            // SG3: the anchor maps follow the node
            if old != r
                && let Some(a) = self.common[xi].as_deref().map(|c| c.anchor.as_str()).filter(|a| !a.is_empty())
            {
                super::query::index_anchor(&mut self.roots, old, x, a, false);
                super::query::index_anchor(&mut self.roots, r, x, a, true);
            }
            let q = self.core[xi].flags & (NodeFlags::IN_RELAYOUT | NodeFlags::IN_REMEASURE);
            if !q.is_empty() {
                requeue.push((x, q));
            }
            if !r.is_detached() {
                self.attached(x, r);
            }
            stack.extend(
                self.children[xi]
                    .iter()
                    .chain(&self.cold(xi).attached)
                    .rev()
                    .copied(),
            );
        }
        for (x, q) in requeue {
            let xi = self.ix(x);
            if r.is_detached() {
                self.core[xi].flags.remove(q);
                self.core[xi].dirty.remove(Dirty::MEASURE);
                if q.contains(NodeFlags::IN_REMEASURE) {
                    self.layout[xi].intrinsic = None;
                }
                self.invalidate_content(x);
            } else {
                let e = &mut self.roots.get_mut(r).expect("live root").dirty;
                if q.contains(NodeFlags::IN_RELAYOUT) {
                    e.relayout.push(x);
                }
                if q.contains(NodeFlags::IN_REMEASURE) {
                    e.remeasure.push(x);
                }
            }
        }
    }

    /// Checks that `origin` may change the structure under `parent` (and move `child`).
    pub(crate) fn may_edit(&self, origin: Origin, nodes: &[NodeId]) -> Result<(), SceneError> {
        if is_js(origin) {
            return Ok(());
        }
        for &n in nodes {
            let owner = self.core[self.ix(n)].owner;
            if owner == Owner::Js {
                return Err(SceneError::NotOwner { node: n, owner });
            }
        }
        Ok(())
    }

    /// Takes `child` out of its parent's children, invalidating the parent's layout; its subtree
    /// is detached from its root when `detach_root`.
    fn unlink(&mut self, child: NodeId, detach_root: bool, origin: Origin) {
        let ci = self.ix(child);
        let Some(p) = self.core[ci].parent else { return };
        if self.core[ci].flags.contains(NodeFlags::ATTACHED) {
            return self.unlink_attached(child, detach_root, origin); // EV2b (provider/behavior.rs)
        }
        let pi = self.ix(p);
        self.children[pi].retain(|&c| c != child);
        self.core[ci].parent = None;
        crate::rich::touched(self, pi); // RT2: a run left its label
        self.core[ci].flags.remove(NodeFlags::SHADOW); // SG4: no longer a shadow top
        let t = self.stamp();
        self.record(Change::Removed { parent: p, node: child }, origin);
        self.core[pi].dirty.insert(Dirty::CHILDREN);
        self.core[pi].changed = t;
        self.core[ci].changed = t;
        self.invalidate_content(p);
        if detach_root && !self.core[ci].root.is_detached() {
            self.set_root_subtree(child, RootIx::DETACHED);
        }
    }

    /// Moves `child` under `parent`, before `before` (last when `None`). `Ok(false)`: nothing
    /// changed (`child` is `before` itself).
    pub(crate) fn insert(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, origin: Origin) -> Result<bool, SceneError> {
        let pi = self.idx(parent).ok_or(SceneError::Stale(parent.to_raw()))?;
        let ci = self.idx(child).ok_or(SceneError::Stale(child.to_raw()))?;
        let (pk, ck) = (self.core[pi].kind as usize, self.core[ci].kind as usize);
        if kinds::KindRegistry::global().attaches(ck) {
            return self.insert_attached(parent, child, origin); // EV2b: a behaviour (provider/behavior.rs)
        }
        // WG2a: a kind that attaches (`context-menu`, `popup`) goes under any element
        if kinds::slot_of(pk).is_none() && kinds::attaches_to(kinds::kind(ck).id).is_none() {
            return Err(SceneError::Invalid(format!("<{}> takes no children", kinds::kind(pk).tag.unwrap_or("?"))));
        }
        // RT2: a label and the runs take runs only (`kinds/rich.rs`)
        if let Some(acc) = kinds::rich::slot_accepts(kinds::kind(pk).id)
            && !acc.contains(&kinds::kind(ck).id)
            && kinds::attaches_to(kinds::kind(ck).id).is_none()
        {
            return Err(SceneError::Invalid(format!("<{}> takes inline runs only (span, b, i, code, a), not <{}>", kinds::kind(pk).tag.unwrap_or("?"), kinds::kind(ck).tag.unwrap_or("?"))));
        }
        if kinds::kind(ck).toplevel || self.core[ci].flags.contains(NodeFlags::ROOT) {
            return Err(SceneError::Invalid("a window cannot be inserted under another element".into()));
        }
        if parent == child {
            return Err(SceneError::Invalid("cannot insert an element into itself".into()));
        }
        self.may_edit(origin, &[parent, child])?;
        if let Some(old) = self.core[ci].parent
            && !self.core[ci].flags.contains(NodeFlags::SHADOW)
        {
            self.may_edit(origin, &[old])?;
        }
        // A parent inside the child's own subtree would make a cycle.
        let mut a = Some(parent);
        while let Some(x) = a {
            if x == child {
                return Err(SceneError::Invalid("cannot insert an element under its own descendant".into()));
            }
            a = self.core[self.ix(x)].parent;
        }
        if let Some(b) = before {
            if b == child {
                return Ok(false); // already in place
            }
            if self.idx(b).and_then(|bi| self.core[bi].parent) != Some(parent) {
                return Err(SceneError::Invalid(format!("handle {b} is not a child of {parent}")));
            }
        }
        self.unlink(child, false, origin);
        let kids = &mut self.children[pi];
        let at = before.and_then(|b| kids.iter().position(|&c| c == b)).unwrap_or(kids.len());
        kids.insert(at, child);
        self.core[ci].parent = Some(parent);
        let t = self.stamp();
        self.core[pi].dirty.insert(Dirty::CHILDREN);
        self.core[pi].changed = t;
        self.core[ci].changed = t;
        let r = self.core[pi].root;
        if self.core[ci].root != r {
            self.set_root_subtree(child, r);
        }
        self.record(Change::Inserted { parent, node: child, before }, origin);
        self.invalidate_content(parent);
        crate::rich::touched(self, pi); // RT2: a run joined a label
        Ok(true)
    }

    /// LS1: makes `child` the last child of virtualiser `parent` (a `virtual-list` adopting a JS row
    /// outlet, or a row a Rust row source built). Not a light child JS placed: no slot or owner
    /// check, the rows are the virtualiser's own (ARCH-SCENE's shadow rows until SG4 has them).
    pub(crate) fn adopt(&mut self, parent: NodeId, child: NodeId, origin: Origin) -> Result<(), SceneError> {
        let pi = self.idx(parent).ok_or(SceneError::Stale(parent.to_raw()))?;
        let ci = self.idx(child).ok_or(SceneError::Stale(child.to_raw()))?;
        if self.core[ci].parent == Some(parent) {
            return Ok(());
        }
        if kinds::kind(self.core[ci].kind as usize).toplevel || self.core[ci].flags.contains(NodeFlags::ROOT) || parent == child {
            return Err(SceneError::Invalid("a row cannot be a window or the list itself".into()));
        }
        let mut a = Some(parent);
        while let Some(x) = a {
            if x == child {
                return Err(SceneError::Invalid("cannot adopt an ancestor".into()));
            }
            a = self.core[self.ix(x)].parent;
        }
        self.unlink(child, false, origin);
        self.children[pi].push(child);
        self.core[ci].parent = Some(parent);
        let t = self.stamp();
        self.core[pi].dirty.insert(Dirty::CHILDREN);
        self.core[pi].changed = t;
        self.core[ci].changed = t;
        let r = self.core[pi].root;
        if self.core[ci].root != r {
            self.set_root_subtree(child, r);
        }
        self.record(Change::Inserted { parent, node: child, before: None }, origin);
        self.invalidate_content(parent);
        Ok(())
    }

    /// Takes `child` out of `parent`; it stays alive, detached.
    pub(crate) fn remove(&mut self, parent: NodeId, child: NodeId, origin: Origin) -> Result<(), SceneError> {
        self.idx(parent).ok_or(SceneError::Stale(parent.to_raw()))?;
        let ci = self.idx(child).ok_or(SceneError::Stale(child.to_raw()))?;
        if self.core[ci].parent != Some(parent) {
            return Err(SceneError::Invalid(format!("handle {child} is not a child of {parent}")));
        }
        // SG4: a shadow top leaves its host whoever owns the host (the host's light children are
        // the runtime's; its shadow children are not)
        let shadow = self.core[ci].flags.contains(NodeFlags::SHADOW);
        let (only_child, both) = ([child], [parent, child]);
        self.may_edit(origin, if shadow { &only_child[..] } else { &both[..] })?;
        self.unlink(child, true, origin);
        Ok(())
    }

    /// SG4 (ARCH-SCENE §4.4): makes `child` a shadow top of the provider node `host`, before
    /// `before` among its shadow children (last when `None`). The host lays it out and draws it
    /// (`WidgetCx::children`); its kind need not take light children, and a runtime-owned host is
    /// fine (its light children are the runtime's, its shadow children are not). `child` must not
    /// be runtime-owned, a window or a root; it moves from wherever it was.
    pub(crate) fn insert_shadow(&mut self, host: NodeId, child: NodeId, before: Option<NodeId>, origin: Origin) -> Result<bool, SceneError> {
        let hi = self.idx(host).ok_or(SceneError::Stale(host.to_raw()))?;
        let ci = self.idx(child).ok_or(SceneError::Stale(child.to_raw()))?;
        let hk = self.core[hi].kind as usize;
        if kinds::KindRegistry::global().provider(hk).is_none() {
            return Err(SceneError::Invalid(format!("<{}> is not a provider kind: only providers host shadow children", self.tag_at(hi))));
        }
        let ck = self.core[ci].kind as usize;
        if kinds::kind(ck).toplevel || self.core[ci].flags.contains(NodeFlags::ROOT) {
            return Err(SceneError::Invalid("a window or root cannot be a shadow child".into()));
        }
        if self.core[ci].owner == Owner::Js {
            return Err(SceneError::NotOwner { node: child, owner: Owner::Js });
        }
        let mut a = Some(host);
        while let Some(x) = a {
            if x == child {
                return Err(SceneError::Invalid("cannot host a node under its own descendant".into()));
            }
            a = self.core[self.ix(x)].parent;
        }
        if let Some(b) = before {
            if b == child {
                return Ok(false);
            }
            if self.idx(b).and_then(|bi| self.core[bi].parent) != Some(host) {
                return Err(SceneError::Invalid(format!("handle {b} is not a shadow child of {host}")));
            }
        }
        if let Some(old) = self.core[ci].parent
            && !self.core[ci].flags.contains(NodeFlags::SHADOW)
        {
            self.may_edit(origin, &[old])?;
        }
        self.unlink(child, false, origin);
        let kids = &mut self.children[hi];
        let at = before.and_then(|b| kids.iter().position(|&c| c == b)).unwrap_or(kids.len());
        kids.insert(at, child);
        self.core[ci].parent = Some(host);
        self.core[ci].flags.insert(NodeFlags::SHADOW);
        let t = self.stamp();
        self.core[hi].dirty.insert(Dirty::CHILDREN);
        self.core[hi].changed = t;
        self.core[ci].changed = t;
        let r = self.core[hi].root;
        if self.core[ci].root != r {
            self.set_root_subtree(child, r);
        }
        self.record(Change::Inserted { parent: host, node: child, before }, origin);
        self.invalidate_content(host);
        Ok(true)
    }

    /// Detaches and frees `id`. Its children are orphaned (detached, alive): a runtime destroys them
    /// itself. A root's top ends its root.
    pub(crate) fn destroy(&mut self, id: NodeId, origin: Origin) -> Result<Destroyed, SceneError> {
        let i = self.idx(id).ok_or(SceneError::Stale(id.to_raw()))?;
        self.may_edit(origin, &[id])?;
        if let Some(p) = self.core[i].parent
            && !self.core[i].flags.contains(NodeFlags::SHADOW)
        {
            self.may_edit(origin, &[p])?;
        }
        self.unlink(id, true, origin);
        if self.core[i].flags.contains(NodeFlags::ROOT) {
            let r = self.core[i].root;
            self.remove_root(r);
        }
        // SG4: a provider's shadow subtrees die with it (no runtime knows them to destroy them)
        let shadow: Vec<NodeId> = self.children[i].iter().copied().filter(|&c| self.core[self.ix(c)].flags.contains(NodeFlags::SHADOW)).collect();
        for c in shadow {
            self.free_subtree_from(c, origin);
        }
        let kids = std::mem::take(&mut self.children[i]);
        for &c in &kids {
            // orphaned: journaled while both are live
            self.record(Change::Removed { parent: id, node: c }, origin);
        }
        for c in kids {
            let ki = self.ix(c);
            self.core[ki].parent = None;
            if !self.core[ki].root.is_detached() {
                self.set_root_subtree(c, RootIx::DETACHED);
            }
        }
        self.orphan_attached(i, origin); // EV2b: its behaviours stay alive, detached
        let d = Destroyed { egui_id: self.core[i].egui_id, was_window: matches!(self.w(i), W::Window { .. }) };
        self.free_slot(i, origin);
        Ok(d)
    }

    /// Frees slot `i`: its extension entries (each journaled `Ext { Removed }`, R19), then the node
    /// (`Destroyed`), then the columns.
    fn free_slot(&mut self, i: usize, origin: Origin) {
        if let Some(node) = self.slots.id_at(i) {
            let Scene { ext, journal, core, roots, tick, .. } = self;
            ext.free(i as u32, |k| {
                if journal.wants_ext(k) {
                    journal.record(core, roots, Change::Ext { node, ext: k, what: ExtChange::Removed }, origin, *tick);
                }
            });
            let kind = KindId(kinds::kind_id(self.core[i].kind as usize));
            self.record(Change::Destroyed { node, kind }, origin);
            // SG3: out of the indexes, after the records
            self.index.remove(self.core[i].kind, node);
            // OW-CSS: its running transitions end with it
            self.style.forget(node);
            let r = self.core[i].root;
            if let Some(a) = self.common[i].as_deref().map(|c| c.anchor.as_str()).filter(|a| !a.is_empty()) {
                super::query::index_anchor(&mut self.roots, r, node, a, false);
            }
        }
        self.slots.free(i);
        self.payload[i] = Payload::Vacant;
        self.cold[i] = None;
        self.common[i] = None;
        self.ink.remove(&(i as u32));
        self.paint_cache.remove(i as u32);
        self.children[i] = Vec::new();
        self.core[i].flags = NodeFlags::EMPTY;
        self.counters.destroyed += 1;
    }

    /// Frees `top` and its whole subtree (a native close); returns the freed ids in pre-order
    /// (empty if `top` is not live).
    pub(crate) fn free_subtree(&mut self, top: NodeId) -> Vec<NodeId> {
        self.free_subtree_from(top, Origin::Host)
    }

    /// [`Scene::free_subtree`] journaled as written by `origin`.
    pub(crate) fn free_subtree_from(&mut self, top: NodeId, origin: Origin) -> Vec<NodeId> {
        let Some(ti) = self.idx(top) else { return Vec::new() };
        self.unlink(top, true, origin);
        if self.core[ti].flags.contains(NodeFlags::ROOT) {
            let r = self.core[ti].root;
            self.remove_root(r);
        }
        let mut dead = Vec::new();
        let mut stack = vec![top];
        while let Some(x) = stack.pop() {
            dead.push(x);
            let xi = self.ix(x);
            stack.extend(
                self.children[xi]
                    .iter()
                    .chain(&self.cold(xi).attached)
                    .rev()
                    .copied(),
            );
        }
        for &x in &dead {
            let xi = self.ix(x);
            self.free_slot(xi, origin);
        }
        dead
    }
}
