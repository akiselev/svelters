//! [`Document`]: the retained tree of one app (all its windows), on the UI thread.
//!
//! Nodes are stored as parallel columns indexed by `NodeId::index()` (as svelters-egui's scene): the
//! hot core record, children, the layout box, the kind value, the written props, the listeners, the
//! style inputs and computed style, the state bits and the recorder's per-node state. Every edit goes
//! through the functions here, which mark exactly the work the edit causes (architecture §4.3):
//!
//! | Edit | Marks |
//! |---|---|
//! | create, insert, remove, reorder, slot | parent's layout to its boundary, `CHILDREN`, restyle of the subtree |
//! | a `Layout` prop (text, font) | the leaf's measure (layout only if its size changes), paint |
//! | a `Paint` prop | the node's paint |
//! | a `Property` prop | the node's property-tree entries |
//!
//! A transaction bumps the [`Revision`] once; an edit outside one is its own transaction.

use crate::common;
use crate::cx::{NodeView, PropCx, TextShaper};
use crate::error::{PropError, TreeError};
use crate::ids::{KindId, PropId, SignalId, SlotId};
use crate::journal::{Change, Journal};
use crate::kind::{FocusReason, Kind};
use crate::registry::KindRegistry;
use crate::style::{ComputedBox, DocStyle, StyleInput};
use crate::tree::slots::SlotTable;
use crate::tree::{Dirty, LayoutBox, ListenFlags, Listener, NodeCore, NodeFlags, PropStore};
use crate::value::Value;
use crate::window::WindowState;
use presto_base::change::ChangeClass;
use presto_base::geom::{Scale, Size};
use presto_base::ids::{InputSeq, NodeId, Revision, WindowId};
use presto_base::text::TextMeasure;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;
use svelters_style::StateMask;

/// Who is writing: native code (Rust, a kind's default handler) or JS that saw `seen`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writer {
    Native,
    /// A JS task started by an event dispatched at revision `seen`.
    Js { seen: Revision },
}

/// The retained tree of one app.
pub struct Document {
    pub(crate) registry: Arc<KindRegistry>,
    pub(crate) text: Box<dyn TextMeasure>,
    pub(crate) shaper: Option<Box<dyn TextShaper>>,
    pub(crate) slots: SlotTable,
    pub(crate) core: Vec<NodeCore>,
    pub(crate) children: Vec<Vec<NodeId>>,
    pub(crate) layout: Vec<LayoutBox>,
    pub(crate) kinds: Vec<Option<Box<dyn Kind>>>,
    pub(crate) props: Vec<PropStore>,
    pub(crate) listeners: Vec<SmallVec<[Listener; 2]>>,
    pub(crate) state: Vec<StateMask>,
    /// Revision of the last native write per (node, prop) of `native_owned` props.
    pub(crate) native_rev: HashMap<(NodeId, PropId), Revision>,
    pub(crate) style: DocStyle,
    pub(crate) record: Vec<crate::record::NodePaint>,
    pub(crate) windows: Vec<WindowState>,
    pub(crate) revision: Revision,
    tx_depth: u32,
    pub(crate) writer: Writer,
    /// The newest input the tree has consumed.
    pub(crate) input_seq: InputSeq,
    pub(crate) journal: Journal,
    pub(crate) input: crate::input::InputState,
    /// JS events queued outside dispatch (programmatic focus moves, kinds' signals).
    pub(crate) input_events: Vec<crate::input::JsEvent>,
    pub(crate) anim: crate::anim::AnimState,
    /// Native-owned scroll offsets of scroll containers.
    pub(crate) scroll: HashMap<NodeId, presto_base::geom::Vec2>,
    /// The revision of the last edit that changed something.
    pub(crate) last_change: Revision,
    /// Run and image ids are unique per document (the compositor holds them per window).
    pub(crate) run_counter: u64,
    pub(crate) images: crate::cx::ImageSink,
}

impl Document {
    pub fn new(registry: Arc<KindRegistry>, text: Box<dyn TextMeasure>) -> Self {
        Document {
            registry,
            text,
            shaper: None,
            slots: SlotTable::default(),
            core: Vec::new(),
            children: Vec::new(),
            layout: Vec::new(),
            kinds: Vec::new(),
            props: Vec::new(),
            listeners: Vec::new(),
            state: Vec::new(),
            native_rev: HashMap::new(),
            style: DocStyle::default(),
            record: Vec::new(),
            windows: Vec::new(),
            revision: Revision(0),
            tx_depth: 0,
            writer: Writer::Native,
            input_seq: InputSeq(0),
            journal: Journal::default(),
            input: Default::default(),
            input_events: Vec::new(),
            anim: Default::default(),
            scroll: HashMap::new(),
            last_change: Revision(0),
            run_counter: 0,
            images: Default::default(),
        }
    }

    /// Gives the document a text shaper, so text kinds record glyph runs.
    pub fn set_text_shaper(&mut self, shaper: Box<dyn TextShaper>) {
        self.shaper = Some(shaper);
    }

    pub fn registry(&self) -> &Arc<KindRegistry> {
        &self.registry
    }

    /// The current revision (one per transaction).
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Live nodes.
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn is_empty(&self) -> bool {
        self.slots.len() == 0
    }

    // ---- ids ----

    #[inline]
    pub(crate) fn idx(&self, id: NodeId) -> Option<usize> {
        self.slots.index(id)
    }

    #[inline]
    pub(crate) fn ix(&self, id: NodeId) -> usize {
        self.idx(id).unwrap_or_else(|| panic!("presto-core: stale node {id:?}"))
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.idx(id).is_some()
    }

    /// A new revision for an edit, or the transaction's.
    pub(crate) fn stamp(&mut self) -> Revision {
        if self.tx_depth == 0 {
            self.revision.0 += 1;
        }
        self.last_change = self.revision;
        self.revision
    }

    /// Runs `f` as one transaction: every edit in it shares one revision.
    pub fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.tx_depth == 0 {
            self.revision.0 += 1;
        }
        self.tx_depth += 1;
        let r = f(self);
        self.tx_depth -= 1;
        r
    }

    /// Runs `f` as a JS task that started from an event dispatched at `seen`: its writes of native-
    /// owned props the kind changed after `seen` are rejected (architecture §4.5).
    pub fn js_transaction<R>(&mut self, seen: Revision, f: impl FnOnce(&mut Self) -> R) -> R {
        let prev = std::mem::replace(&mut self.writer, Writer::Js { seen });
        let r = self.transaction(f);
        self.writer = prev;
        r
    }

    /// Records that the tree has seen input `seq` (the next commit carries it).
    pub fn consumed_input(&mut self, seq: InputSeq) {
        if seq > self.input_seq {
            self.input_seq = seq;
        }
    }

    // ---- views ----

    /// A read view of a live node.
    pub fn view(&self, id: NodeId) -> Option<NodeView<'_>> {
        let i = self.idx(id)?;
        Some(self.view_at(i))
    }

    pub(crate) fn view_at(&self, i: usize) -> NodeView<'_> {
        let core = &self.core[i];
        let entry = self.registry.get(core.kind).expect("registered kind");
        let l = &self.layout[i].layout;
        NodeView {
            id: self.slots.id_at(i).expect("live slot"),
            entry,
            props: &self.props[i],
            style: self.style.computed(i),
            state: self.state[i],
            size: Size::new(l.size.width, l.size.height),
            children: &self.children[i],
            window: core.window,
        }
    }

    pub fn kind_of(&self, id: NodeId) -> Option<KindId> {
        self.idx(id).map(|i| self.core[i].kind)
    }
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.idx(id).and_then(|i| self.core[i].parent)
    }
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.idx(id).map_or(&[], |i| &self.children[i])
    }
    pub fn slot_of(&self, id: NodeId) -> Option<SlotId> {
        self.idx(id).and_then(|i| self.core[i].slot)
    }
    pub fn window_of(&self, id: NodeId) -> Option<WindowId> {
        self.idx(id).and_then(|i| self.core[i].window)
    }
    pub fn flags(&self, id: NodeId) -> Option<NodeFlags> {
        self.idx(id).map(|i| self.core[i].flags)
    }
    pub fn dirty(&self, id: NodeId) -> Option<Dirty> {
        self.idx(id).map(|i| self.core[i].dirty)
    }
    pub fn state(&self, id: NodeId) -> StateMask {
        self.idx(id).map_or(StateMask::NONE, |i| self.state[i])
    }
    /// The node's border box from the last layout, relative to its parent's border box.
    pub fn layout_rect(&self, id: NodeId) -> Option<presto_base::geom::Rect> {
        let i = self.idx(id)?;
        let l = &self.layout[i].layout;
        Some(presto_base::geom::Rect::new(l.location.x, l.location.y, l.location.x + l.size.width, l.location.y + l.size.height))
    }
    /// Calls `f` with the node's kind value downcast-free (for kinds to read their own state in tests
    /// and in svelters-presto's `bind:` reads).
    pub fn with_kind<R>(&self, id: NodeId, f: impl FnOnce(&dyn Kind) -> R) -> Option<R> {
        let i = self.idx(id)?;
        self.kinds[i].as_deref().map(f)
    }

    // ---- structure ----

    /// Creates a detached node of `kind`.
    ///
    /// # Panics
    /// If `kind` is not registered: the caller (svelters-presto) validates kinds from the spec first.
    pub fn create(&mut self, kind: KindId) -> NodeId {
        let entry = self.registry.get(kind).unwrap_or_else(|| panic!("presto-core: unregistered kind {kind:?}"));
        let value = (entry.factory)();
        let rev = self.stamp();
        let (id, fresh) = self.slots.alloc();
        let core = NodeCore {
            kind,
            parent: None,
            slot: None,
            window: None,
            flags: NodeFlags::EMPTY,
            dirty: Dirty::STYLE | Dirty::LAYOUT | Dirty::MEASURE | Dirty::RECORD,
            changed: rev,
        };
        let i = id.index() as usize;
        if fresh {
            self.core.push(core);
            self.children.push(Vec::new());
            self.layout.push(LayoutBox::default());
            self.kinds.push(Some(value));
            self.props.push(PropStore::default());
            self.listeners.push(SmallVec::new());
            self.state.push(StateMask::NONE);
            self.record.push(Default::default());
            self.style.grow();
        } else {
            self.core[i] = core;
            self.children[i].clear();
            self.layout[i] = LayoutBox::default();
            self.kinds[i] = Some(value);
            self.props[i] = PropStore::default();
            self.listeners[i].clear();
            self.state[i] = StateMask::NONE;
            self.record[i] = Default::default();
            self.style.reset(i);
        }
        self.journal.push(Change::Created { node: id, kind, revision: rev });
        id
    }

    /// Creates a window's root node of `kind` and attaches the window.
    pub fn create_window(&mut self, window: WindowId, kind: KindId, size: Size, scale: Scale) -> NodeId {
        let root = self.create(kind);
        let i = self.ix(root);
        self.core[i].flags.insert(NodeFlags::ROOT);
        self.core[i].window = Some(window);
        self.windows.retain(|w| w.id != window);
        self.windows.push(WindowState::new(window, root, size, scale));
        self.mark_subtree_style(i);
        root
    }

    /// Removes a window: its root and every node under it are destroyed.
    pub fn close_window(&mut self, window: WindowId) {
        if let Some(w) = self.windows.iter().position(|w| w.id == window) {
            let root = self.windows[w].root;
            self.windows.remove(w);
            if self.contains(root) {
                self.destroy(root);
            }
        }
    }

    /// The window's content size (points) and scale changed.
    pub fn resize_window(&mut self, window: WindowId, size: Size, scale: Scale) {
        if let Some(w) = self.window_mut(window) {
            let scale_changed = w.scale != scale;
            w.size = size;
            w.scale = scale;
            w.dirty.full = true;
            let root = w.root;
            if scale_changed {
                // text and hairlines re-record at the new scale
                if let Some(i) = self.idx(root) {
                    self.mark_subtree(i, Dirty::PAINT);
                }
            }
        }
    }

    pub fn window_root(&self, window: WindowId) -> Option<NodeId> {
        self.windows.iter().find(|w| w.id == window).map(|w| w.root)
    }

    pub(crate) fn window_mut(&mut self, window: WindowId) -> Option<&mut WindowState> {
        self.windows.iter_mut().find(|w| w.id == window)
    }
    pub(crate) fn window(&self, window: WindowId) -> Option<&WindowState> {
        self.windows.iter().find(|w| w.id == window)
    }

    /// Inserts `child` under `parent` before `before` (`None`: last), in `slot` (`None`: the default
    /// slot). A child that has a parent is moved.
    pub fn insert(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>, slot: Option<SlotId>) -> Result<(), TreeError> {
        let pi = self.idx(parent).ok_or(TreeError::Stale(parent))?;
        let ci = self.idx(child).ok_or(TreeError::Stale(child))?;
        if self.core[ci].flags.contains(NodeFlags::ROOT) {
            return Err(TreeError::IsRoot(child));
        }
        let entry = self.registry.get(self.core[pi].kind).expect("registered kind");
        if !entry.spec.container {
            return Err(TreeError::NotAContainer(parent));
        }
        if let Some(s) = slot {
            let spec = entry.slot(s).ok_or(TreeError::UnknownSlot { parent, slot: s })?;
            if spec.single && self.children[pi].iter().any(|&c| c != child && self.core[self.ix(c)].slot == Some(s)) {
                return Err(TreeError::SlotFull { parent, slot: s });
            }
        }
        // the parent must not be the child or under it
        let mut a = Some(parent);
        while let Some(n) = a {
            if n == child {
                return Err(TreeError::Cycle { parent, child });
            }
            a = self.core[self.ix(n)].parent;
        }
        if before == Some(child) {
            return if self.core[ci].parent == Some(parent) { Ok(()) } else { Err(TreeError::NotAChild { parent, child }) };
        }
        if let Some(b) = before {
            let bi = self.idx(b).ok_or(TreeError::Stale(b))?;
            if self.core[bi].parent != Some(parent) {
                return Err(TreeError::NotAChild { parent, child: b });
            }
        }
        let rev = self.stamp();
        if let Some(old) = self.core[ci].parent {
            self.detach(old, child);
        }
        let list = &mut self.children[pi];
        let at = match before {
            Some(b) => list.iter().position(|&c| c == b).expect("checked above"),
            None => list.len(),
        };
        list.insert(at, child);
        self.core[ci].parent = Some(parent);
        self.core[ci].slot = slot;
        self.core[ci].changed = rev;
        let window = self.core[pi].window;
        self.set_window(ci, window);
        self.structure_changed(pi);
        self.mark_subtree_style(ci);
        self.mark_subtree(ci, Dirty::RECORD);
        self.journal.push(Change::Inserted { parent, child, revision: rev });
        Ok(())
    }

    /// Removes `child` from `parent`; it stays alive, detached.
    pub fn remove(&mut self, parent: NodeId, child: NodeId) -> Result<(), TreeError> {
        let pi = self.idx(parent).ok_or(TreeError::Stale(parent))?;
        let ci = self.idx(child).ok_or(TreeError::Stale(child))?;
        if self.core[ci].parent != Some(parent) {
            return Err(TreeError::NotAChild { parent, child });
        }
        let rev = self.stamp();
        let _ = pi;
        self.detach(parent, child);
        self.core[ci].changed = rev;
        self.journal.push(Change::Removed { parent, child, revision: rev });
        Ok(())
    }

    /// Unlinks `child` from `parent` and detaches its subtree from its window.
    fn detach(&mut self, parent: NodeId, child: NodeId) {
        let pi = self.ix(parent);
        let ci = self.ix(child);
        self.children[pi].retain(|&c| c != child);
        self.core[ci].parent = None;
        self.core[ci].slot = None;
        if let Some(w) = self.core[ci].window {
            self.forget_in_window(w, ci);
        }
        self.set_window(ci, None);
        self.structure_changed(pi);
    }

    /// Destroys `node` and its subtree (removing it from its parent first). Stale ids are ignored.
    pub fn destroy(&mut self, node: NodeId) {
        let Some(i) = self.idx(node) else { return };
        let rev = self.stamp();
        if let Some(p) = self.core[i].parent {
            self.detach(p, node);
        } else if let Some(w) = self.core[i].window {
            self.forget_in_window(w, i);
        }
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            let ni = self.ix(n);
            stack.extend(self.children[ni].drain(..));
            self.free(ni);
        }
        self.journal.push(Change::Destroyed { node, revision: rev });
    }

    fn free(&mut self, i: usize) {
        let id = self.slots.id_at(i).expect("live slot");
        self.kinds[i] = None;
        self.props[i] = PropStore::default();
        self.listeners[i].clear();
        self.native_rev.retain(|(n, _), _| *n != id);
        self.input.forget(id);
        self.style.reset(i);
        self.anim.forget(id);
        self.scroll.remove(&id);
        self.slots.free(i);
    }

    /// Sets the window of `i`'s subtree.
    fn set_window(&mut self, i: usize, window: Option<WindowId>) {
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            self.core[n].window = window;
            for &c in &self.children[n] {
                stack.push(self.ix(c));
            }
        }
    }

    /// `i` (and its subtree) leaves window `w`: its lists, property nodes and layers go in the next
    /// commit's removals; focus, hover and capture inside it are dropped.
    fn forget_in_window(&mut self, w: WindowId, i: usize) {
        let mut gone = Vec::new();
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            if let Some(id) = self.slots.id_at(n) {
                gone.push(id);
            }
            for &c in &self.children[n] {
                stack.push(self.ix(c));
            }
        }
        // a closing dialog scope gives focus back while its nodes are still in the window
        crate::input::focus::scopes_closed(self, w, &gone);
        for &id in &gone {
            let n = self.ix(id);
            let paint = std::mem::take(&mut self.record[n]);
            if let Some(ws) = self.window_mut(w) {
                ws.retire(id, paint);
            }
            self.core[n].flags.remove(NodeFlags::IN_RELAYOUT | NodeFlags::IN_REMEASURE);
            self.core[n].dirty.insert(Dirty::RECORD | Dirty::LAYOUT);
            self.layout[n].cache.clear();
        }
        self.input.leave_subtree(&gone);
        if let Some(ws) = self.window_mut(w) {
            ws.a11y_removed = true;
        }
    }

    /// The children of `pi` changed.
    fn structure_changed(&mut self, pi: usize) {
        self.core[pi].dirty.insert(Dirty::CHILDREN | Dirty::A11Y);
        self.mark_up(pi, Dirty::DESC_RECORD);
        self.invalidate_content(pi);
        // structural selectors (:first-child, :has) may restyle around it
        self.restyle_structure(pi);
    }

    // ---- props ----

    /// Writes a prop. Returns the change class the write caused (from the kind's spec).
    pub fn set_prop(&mut self, node: NodeId, prop: PropId, v: Value) -> Result<ChangeClass, PropError> {
        self.write_prop(node, prop, Some(v), false)
    }

    /// Removes a written prop (it reads as its default again).
    pub fn reset_prop(&mut self, node: NodeId, prop: PropId) -> Result<ChangeClass, PropError> {
        self.write_prop(node, prop, None, false)
    }

    /// A kind's (or Rust's) write of native-owned state.
    pub(crate) fn set_prop_native(&mut self, node: NodeId, prop: PropId, v: Value) -> Result<ChangeClass, PropError> {
        self.write_prop(node, prop, Some(v), true)
    }

    fn write_prop(&mut self, node: NodeId, prop: PropId, v: Option<Value>, native: bool) -> Result<ChangeClass, PropError> {
        let i = self.idx(node).ok_or(PropError::Stale(node))?;
        let kind = self.core[i].kind;
        let entry = self.registry.get(kind).expect("registered kind");
        let spec = entry.prop(prop).or_else(|| common::prop(prop)).ok_or(PropError::Unknown(prop))?;
        let v = match v {
            Some(v) => {
                let got = v.ty();
                Some(v.coerce(spec.ty).ok_or(PropError::WrongType { prop, expected: spec.ty, got })?)
            }
            None => None,
        };
        if spec.native_owned && !native
            && let Writer::Js { seen } = self.writer
        {
            let changed = self.native_rev.get(&(node, prop)).copied().or_else(|| {
                self.kinds[i].as_deref().and_then(|k| k.native_state()).and_then(|s| s.changed_at(prop))
            });
            if let Some(changed) = changed
                && changed > seen
            {
                return Err(PropError::Superseded { prop, changed, seen });
            }
        }
        let class = spec.change;
        let is_text = entry.text_prop == Some(prop);
        let old = self.props[i].set(prop, v.clone());
        if old == v {
            return Ok(class);
        }
        // the kind sees the stored value; it may reject it (undo) or ask for more work
        let mut request = Dirty::EMPTY;
        if let Some(mut k) = self.kinds[i].take() {
            let res = {
                let mut cx = PropCx { node: self.view_at(i), request: Dirty::EMPTY };
                let r = k.prop_changed(&mut cx, prop, v.as_ref());
                request = cx.request;
                r
            };
            self.kinds[i] = Some(k);
            if let Err(e) = res {
                self.props[i].set(prop, old);
                return Err(e);
            }
        }
        let rev = self.stamp();
        self.core[i].changed = rev;
        if native && spec.native_owned {
            self.native_rev.insert((node, prop), rev);
        }
        self.mark_class(i, class, is_text);
        if !request.is_empty() {
            if request.contains(Dirty::MEASURE) {
                self.mark_measure(i);
            }
            if request.contains(Dirty::LAYOUT) {
                self.invalidate_style(i);
            }
            self.mark(node, request);
        }
        // common props with their own effects
        if prop == common::DISABLED {
            let on = v.as_ref().and_then(Value::as_bool).unwrap_or(false);
            self.set_state(node, StateMask::DISABLED, on);
        }
        // attribute selectors may match the prop
        self.restyle_attr(i, spec.name);
        self.journal.push(Change::Prop { node, prop, revision: rev, native });
        Ok(class)
    }

    /// Marks the work of `class` on node `i`.
    pub(crate) fn mark_class(&mut self, i: usize, class: ChangeClass, text: bool) {
        let id = self.slots.id_at(i).expect("live");
        match class {
            ChangeClass::Property => self.mark(id, Dirty::PROPERTY),
            ChangeClass::Paint => self.mark(id, Dirty::PAINT | Dirty::A11Y),
            ChangeClass::Layout => {
                if text || self.children[i].is_empty() {
                    self.mark_measure(i);
                } else {
                    self.invalidate_style(i);
                }
                self.mark(id, Dirty::PAINT | Dirty::A11Y);
            }
            ChangeClass::Structure => {
                self.invalidate_style(i);
                self.mark(id, Dirty::RECORD);
                if let Some(p) = self.core[i].parent {
                    let pi = self.ix(p);
                    self.core[pi].dirty.insert(Dirty::CHILDREN);
                }
            }
        }
    }

    /// The value of a prop: the kind's live native state, the written value, or the default.
    pub fn get_prop(&self, node: NodeId, prop: PropId) -> Option<Value> {
        let i = self.idx(node)?;
        if let Some(v) = self.kinds[i].as_deref().and_then(|k| k.native_state()).and_then(|s| s.get(prop)) {
            return Some(v);
        }
        self.view_at(i).prop(prop).cloned()
    }

    // ---- listeners ----

    /// Adds a listener (replacing one for the same signal and capture flag).
    pub fn listen(&mut self, node: NodeId, signal: SignalId, flags: ListenFlags) {
        let Some(i) = self.idx(node) else { return };
        let capture = flags.contains(ListenFlags::CAPTURE);
        let ls = &mut self.listeners[i];
        ls.retain(|l| !(l.signal == signal && l.flags.contains(ListenFlags::CAPTURE) == capture));
        ls.push(Listener { signal, flags });
        self.update_passive(i);
    }

    /// Removes a listener.
    pub fn unlisten(&mut self, node: NodeId, signal: SignalId, capture: bool) {
        let Some(i) = self.idx(node) else { return };
        self.listeners[i].retain(|l| !(l.signal == signal && l.flags.contains(ListenFlags::CAPTURE) == capture));
        self.update_passive(i);
    }

    pub fn listeners(&self, node: NodeId) -> &[Listener] {
        self.idx(node).map_or(&[], |i| &self.listeners[i])
    }

    /// A non-passive wheel or scroll-path listener makes scrolling under it wait for JS (§11.5).
    fn update_passive(&mut self, i: usize) {
        let nonpassive = self.listeners[i].iter().any(|l| {
            !l.flags.contains(ListenFlags::PASSIVE) && matches!(l.signal, s if s == common::WHEEL || s == common::POINTERMOVE)
        });
        let was = self.core[i].flags.contains(NodeFlags::NONPASSIVE);
        if nonpassive != was {
            if nonpassive {
                self.core[i].flags.insert(NodeFlags::NONPASSIVE);
            } else {
                self.core[i].flags.remove(NodeFlags::NONPASSIVE);
            }
            // the scroll nodes above it change their `passive` bit
            self.mark_ancestors_property(i);
        }
    }

    fn mark_ancestors_property(&mut self, i: usize) {
        let mut a = self.core[i].parent;
        while let Some(p) = a {
            let pi = self.ix(p);
            self.core[pi].dirty.insert(Dirty::PROPERTY);
            self.mark_up(pi, Dirty::DESC_RECORD);
            a = self.core[pi].parent;
        }
    }

    // ---- style ----

    /// Sets the node's classes, `#id` and inline declarations.
    pub fn set_style(&mut self, node: NodeId, style: StyleInput) {
        let Some(i) = self.idx(node) else { return };
        let old = self.style.input(i).cloned();
        if self.style.set_input(i, style.clone()) {
            let rev = self.stamp();
            self.core[i].changed = rev;
            self.restyle_inputs(i, old.as_ref(), &style);
            self.journal.push(Change::Style { node, revision: rev });
        }
    }

    /// Adds a style sheet to every window (an `Origin::Author` sheet).
    pub fn add_sheet(&mut self, css: &str) -> svelters_style::SheetId {
        let id = self.style.add_sheet(css);
        self.restyle_all();
        id
    }

    pub fn remove_sheet(&mut self, id: svelters_style::SheetId) {
        self.style.remove_sheet(id);
        self.restyle_all();
    }

    pub(crate) fn restyle_all(&mut self) {
        let roots: Vec<NodeId> = self.windows.iter().map(|w| w.root).collect();
        for r in roots {
            if let Some(i) = self.idx(r) {
                self.core[i].dirty.insert(Dirty::STYLE | Dirty::STYLE_SUBTREE);
            }
        }
    }

    /// The computed style of a node.
    pub fn computed(&self, node: NodeId) -> Option<&ComputedBox> {
        self.idx(node).map(|i| self.style.computed(i))
    }

    // ---- state ----

    /// Sets or clears state bits (`:hover`, `:checked`); restyles the node when they change.
    pub fn set_state(&mut self, node: NodeId, bits: StateMask, on: bool) {
        let Some(i) = self.idx(node) else { return };
        let old = self.state[i];
        let new = if on { StateMask(old.0 | bits.0) } else { StateMask(old.0 & !bits.0) };
        if new == old {
            return;
        }
        self.state[i] = new;
        self.stamp();
        self.core[i].dirty.insert(Dirty::A11Y);
        self.mark_up(i, Dirty::DESC_RECORD);
        self.restyle_state(i, old, new);
        self.journal.push(Change::State { node, state: new });
    }

    // ---- dirty marking ----

    /// Sets `bits` on the node and the matching descendant bit on its ancestors.
    pub(crate) fn mark(&mut self, node: NodeId, bits: Dirty) {
        let Some(i) = self.idx(node) else { return };
        self.core[i].dirty.insert(bits);
        if bits.intersects(Dirty::RECORD) {
            self.mark_up(i, Dirty::DESC_RECORD);
        }
        if bits.intersects(Dirty::STYLE) {
            self.mark_up(i, Dirty::DESC_STYLE);
        }
    }

    /// Sets `bits` on every ancestor of `i`, stopping at one that has them.
    pub(crate) fn mark_up(&mut self, i: usize, bits: Dirty) {
        let mut a = self.core[i].parent;
        while let Some(p) = a {
            let pi = self.ix(p);
            if self.core[pi].dirty.contains(bits) {
                return;
            }
            self.core[pi].dirty.insert(bits);
            a = self.core[pi].parent;
        }
    }

    /// Sets `bits` on `i`'s whole subtree.
    pub(crate) fn mark_subtree(&mut self, i: usize, bits: Dirty) {
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            self.core[n].dirty.insert(bits.union(Dirty::DESC_RECORD));
            for &c in &self.children[n] {
                stack.push(self.ix(c));
            }
        }
        self.mark_up(i, Dirty::DESC_RECORD);
    }

    fn mark_subtree_style(&mut self, i: usize) {
        self.core[i].dirty.insert(Dirty::STYLE | Dirty::STYLE_SUBTREE);
        self.mark_up(i, Dirty::DESC_STYLE);
    }

    /// A node's own layout changed (a kind asked): relayout to the boundary.
    pub(crate) fn mark_layout(&mut self, node: NodeId) {
        if let Some(i) = self.idx(node) {
            self.invalidate_style(i);
        }
    }

    // ---- focus ----

    /// Moves keyboard focus to `node` (in its window), in this transaction.
    pub fn focus(&mut self, node: NodeId, reason: FocusReason) {
        crate::input::focus::set_focus(self, Some(node), reason);
    }

    /// Clears focus in `window`.
    pub fn blur(&mut self, window: WindowId) {
        if let Some(f) = self.focused(window) {
            let _ = f;
            crate::input::focus::clear_focus(self, window);
        }
    }

    /// The focused node of `window`.
    pub fn focused(&self, window: WindowId) -> Option<NodeId> {
        self.window(window).and_then(|w| w.focus)
    }

    /// JS events queued outside dispatch (focus moves from `focus()`, signals kinds emitted while
    /// preparing): svelters-presto delivers them after its own task.
    pub fn take_events(&mut self) -> Vec<crate::input::JsEvent> {
        std::mem::take(&mut self.input_events)
    }

    /// Drains the change journal (svelters-presto reads it for `bind:` and observers).
    pub fn drain_changes(&mut self) -> Vec<Change> {
        self.journal.drain()
    }
}
