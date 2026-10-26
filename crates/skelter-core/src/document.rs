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
use skelter_base::change::ChangeClass;
use skelter_base::geom::{Scale, Size};
use skelter_base::ids::{InputSeq, NodeId, Revision, WindowId};
use skelter_base::text::TextMeasure;
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

/// The deliverer of JS events at event time ([`Document::set_sync_delivery`]).
pub type SyncDelivery = Box<dyn FnMut(&mut Document, &crate::input::JsEvent) -> crate::kind::Outcome>;

/// The app's drag previews ([`Document::set_drag_preview_provider`]): the source node and the
/// press in its space, to the drag image.
pub type DragPreviewProvider = Box<dyn FnMut(&mut Document, NodeId, skelter_base::geom::Point) -> Option<crate::kind::DragPreview>>;

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
    pub(crate) scroll: HashMap<NodeId, skelter_base::geom::Vec2>,
    /// The revision of the last edit that changed something.
    pub(crate) last_change: Revision,
    /// Run and image ids are unique per document (the compositor holds them per window).
    pub(crate) run_counter: u64,
    pub(crate) images: crate::cx::ImageSink,
    pub(crate) key_hook: Option<Box<dyn FnMut(WindowId, &skelter_base::event::KeyEvent) -> bool>>,
    pub(crate) services: crate::cx::Services,
    /// Kinds' deadlines (`EventCx::wake_at`).
    pub(crate) timers: HashMap<NodeId, skelter_base::time::Time>,
    /// The realised rows of virtualising nodes ([`crate::VirtualChildren`]).
    pub(crate) virt: HashMap<NodeId, crate::virt::VirtState>,
    /// Peer requests ([`EventCx::peer_request`](crate::EventCx::peer_request)) not applied yet.
    pub(crate) peer_queue: std::collections::VecDeque<(NodeId, crate::kind::PeerTarget, crate::kind::PeerMsg)>,
    pub(crate) peer_running: bool,
    /// Delivers a JS event at event time and answers what its listeners did.
    pub(crate) sync_delivery: Option<SyncDelivery>,
    /// Makes the drag image of drags whose source kind supplies none.
    pub(crate) drag_preview_provider: Option<DragPreviewProvider>,
    /// Prop writes made while their node's kind was out (JS run during the kind's own handler):
    /// the kind gets `prop_changed` for them when it is back.
    pub(crate) missed_props: Vec<(NodeId, PropId)>,
    /// The deliverer is running (its depth), and the sync events raised meanwhile.
    pub(crate) delivering: u32,
    pub(crate) nested: Vec<crate::input::JsEvent>,
    /// The nodes with a non-empty `anchor`.
    pub(crate) anchors: HashMap<NodeId, Arc<str>>,
    /// Nodes with `focus-scope=dialog` or `autofocus` (the focus rules after style watch them).
    pub(crate) focus_watch: std::collections::HashSet<NodeId>,
    /// The nodes with a non-empty `for` prop (labels), and the anchor it names.
    pub(crate) label_for: HashMap<NodeId, Arc<str>>,
    /// The focused push button of a dialog scope is its default (`Policy::auto_default`).
    pub(crate) auto_default: bool,
    /// Per kind, its live nodes.
    pub(crate) kind_sets: crate::query::KindSets,
    /// Kinds that scroll their own content ([`Kind::self_scroll`]).
    pub(crate) self_scroll: HashMap<NodeId, crate::kind::SelfScroll>,
    /// The one selection's owner while single selection is on (GTK), and whether it is.
    pub(crate) selection_owner: Option<NodeId>,
    pub(crate) single_selection: bool,
    /// The shared taffy styles of the layout column.
    pub(crate) styles: crate::compact::Interner<taffy::Style>,
    /// Nodes whose kind asks for light dismiss now ([`Kind::light_dismiss`]).
    pub(crate) light_dismiss: std::collections::HashSet<NodeId>,
    /// Fast commits queued for the compositor ([`Document::take_fast_commits`]).
    pub(crate) fast: Vec<skelter_base::commit::FastCommit>,
    /// The last caret a kind placed (`crate::caret`).
    pub(crate) caret: Option<crate::caret::CaretSlot>,
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
            key_hook: None,
            services: Default::default(),
            timers: HashMap::new(),
            virt: HashMap::new(),
            peer_queue: Default::default(),
            peer_running: false,
            sync_delivery: None,
            drag_preview_provider: None,
            missed_props: Vec::new(),
            delivering: 0,
            nested: Vec::new(),
            anchors: HashMap::new(),
            focus_watch: std::collections::HashSet::new(),
            label_for: HashMap::new(),
            auto_default: false,
            kind_sets: Default::default(),
            self_scroll: HashMap::new(),
            light_dismiss: Default::default(),
            styles: Default::default(),
            selection_owner: None,
            single_selection: false,
            fast: Vec::new(),
            caret: None,
        }
    }

    /// Installs the deliverer of JS events at event time (svelters-skelter's `call_sync`): cancelable
    /// events then reach their listeners before the native default, which honours `preventDefault`
    /// ([`EventCx::emit_sync`](crate::EventCx::emit_sync), and the DOM pointer, key and wheel events
    /// of [`Document::dispatch`]). Events it delivered are not listed in
    /// [`Dispatched::events`](crate::Dispatched::events).
    ///
    /// The deliverer gets the document lent to it: listeners read the tree, write props and build
    /// nodes directly (writes belong in [`Document::js_transaction`] with the event's revision). The
    /// path is the one computed when the event was built: a listener node that is gone when its turn
    /// comes is skipped (`Document::contains`). While the deliverer runs, a sync event a
    /// listener's action raises (`el.click()`, a `focus()`'s focus events) is not delivered inside
    /// the listener: it is delivered, not cancelable, right after the listener call that caused it
    /// returns (before the outer default runs), and so on until none is left. A write to the node
    /// whose kind is mid-handler reaches that kind's `prop_changed` when the handler returns.
    pub fn set_sync_delivery(&mut self, f: SyncDelivery) {
        self.sync_delivery = Some(f);
    }

    /// Puts node `node`'s kind back after a call and replays the prop writes it missed meanwhile.
    pub(crate) fn restore_kind(&mut self, node: NodeId, mut k: Box<dyn Kind>) {
        let Some(i) = self.idx(node) else { return };
        self.refresh_light_dismiss(node, &*k);
        if self.missed_props.iter().any(|(n, _)| *n == node) {
            let missed: Vec<PropId> = self.missed_props.iter().filter(|(n, _)| *n == node).map(|(_, p)| *p).collect();
            self.missed_props.retain(|(n, _)| *n != node);
            let mut request = Dirty::EMPTY;
            for p in missed {
                let v = self.props[i].get(p).cloned();
                let mut cx = PropCx { node: self.view_at(i), request: Dirty::EMPTY };
                // the write is stored already: a rejection now cannot undo it
                let _ = k.prop_changed(&mut cx, p, v.as_ref());
                request.insert(cx.request);
            }
            self.kinds[i] = Some(k);
            self.apply_request(i, request);
        } else {
            self.kinds[i] = Some(k);
        }
        // the state its props imply and its inert children follow writes made while it was out
        self.refresh_prop_state(i);
        self.refresh_children_inert(i);
    }

    /// The focused push button of a `dialog` focus scope is the scope's default (`:default`, Enter
    /// activates it), as Windows, KDE and GTK do (`Policy::auto_default`); off (macOS), Enter goes to
    /// the button whose `default` prop is set whatever has focus.
    pub fn set_auto_default(&mut self, on: bool) {
        if self.auto_default != on {
            self.auto_default = on;
            let focused: Vec<NodeId> = self.windows.iter().filter_map(|w| w.focus).collect();
            for f in focused {
                crate::input::scope::refresh_default(self, f);
            }
        }
    }

    /// GTK's rule: one text selection in the whole app. A text kind that selects something calls
    /// [`EventCx::claim_selection`](crate::EventCx::claim_selection); the previous owner gets
    /// [`Kind::selection_lost`] and collapses its selection. Off (the default) on other platforms.
    pub fn set_single_selection(&mut self, on: bool) {
        self.single_selection = on;
        if !on {
            self.selection_owner = None;
        }
    }

    /// The node of `window` whose `anchor` is `anchor`.
    pub fn find_anchor(&self, window: WindowId, anchor: &str) -> Option<NodeId> {
        self.anchors.iter().find(|(n, a)| &***a == anchor && self.window_of(**n) == Some(window)).map(|(n, _)| *n)
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
        self.idx(id).unwrap_or_else(|| panic!("skelter-core: stale node {id:?}"))
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
            css: self.style.nodes[i].css.as_deref(),
            doc: self,
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
    pub fn layout_rect(&self, id: NodeId) -> Option<skelter_base::geom::Rect> {
        let i = self.idx(id)?;
        let l = &self.layout[i].layout;
        Some(skelter_base::geom::Rect::new(l.location.x, l.location.y, l.location.x + l.size.width, l.location.y + l.size.height))
    }
    /// Calls `f` with the node's kind value downcast-free (for kinds to read their own state in tests
    /// and in svelters-skelter's `bind:` reads).
    pub fn with_kind<R>(&self, id: NodeId, f: impl FnOnce(&dyn Kind) -> R) -> Option<R> {
        let i = self.idx(id)?;
        self.kinds[i].as_deref().map(f)
    }

    // ---- structure ----

    /// Creates a detached node of `kind`.
    ///
    /// # Panics
    /// If `kind` is not registered: the caller (svelters-skelter) validates kinds from the spec first.
    pub fn create(&mut self, kind: KindId) -> NodeId {
        let entry = self.registry.get(kind).unwrap_or_else(|| panic!("skelter-core: unregistered kind {kind:?}"));
        let mut value = (entry.factory)();
        let virtual_kind = value.virtual_children().is_some();
        let folds = entry.spec.folds_children;
        let rev = self.stamp();
        let (id, fresh) = self.slots.alloc();
        let core = NodeCore {
            kind,
            parent: None,
            slot: None,
            window: None,
            flags: NodeFlags::EMPTY,
            dirty: Dirty::STYLE | Dirty::LAYOUT | Dirty::RECORD,
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
        if folds {
            self.core[i].flags.insert(NodeFlags::FOLDS);
        }
        if virtual_kind {
            self.core[i].flags.insert(NodeFlags::VIRTUAL);
            self.virt.insert(id, crate::virt::VirtState::default());
        }
        self.kind_sets.add(kind, id);
        self.refresh_prop_state(i);
        self.journal.push(Change::Created { node: id, kind, revision: rev });
        id
    }

    /// Creates a window's root node of `kind` and attaches the window.
    pub fn create_window(&mut self, window: WindowId, kind: KindId, size: Size, scale: Scale) -> NodeId {
        let root = self.create(kind);
        self.attach_window(window, root, size, scale).expect("a new node is detached");
        root
    }

    /// Presents a detached node (a JS-created `window` node) as `window`'s root. A window that had
    /// another root keeps that root's nodes, detached.
    pub fn attach_window(&mut self, window: WindowId, root: NodeId, size: Size, scale: Scale) -> Result<(), TreeError> {
        let i = self.idx(root).ok_or(TreeError::Stale(root))?;
        if let Some(p) = self.core[i].parent {
            return Err(TreeError::NotAChild { parent: p, child: root });
        }
        if let Some(old) = self.window_root(window).filter(|&r| r != root)
            && let Some(oi) = self.idx(old)
        {
            self.forget_in_window(window, oi, false);
            self.core[oi].flags.remove(NodeFlags::ROOT);
            self.set_window(oi, None);
        }
        self.stamp();
        self.core[i].flags.insert(NodeFlags::ROOT);
        self.set_window(i, Some(window));
        self.windows.retain(|w| w.id != window);
        self.windows.push(WindowState::new(window, root, size, scale));
        self.layout[i].cache.clear();
        self.mark_subtree_style(i);
        self.mark_subtree(i, Dirty::RECORD);
        Ok(())
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
                // the root's AccessKit transform is the scale (children are relative to it)
                self.mark(root, Dirty::A11Y);
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
            // a move within one window keeps pointer capture, focus and hover (decided,
            // coordinator 2026-10-26: berth's reconcile re-inserts a dragged tab)
            let window = self.core[ci].window;
            let keep_input = window.is_some() && window == self.core[pi].window;
            self.detach_as(old, child, keep_input);
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
        if self.core[ci].flags.intersects(NodeFlags::NONPASSIVE.union(NodeFlags::DESC_NONPASSIVE)) {
            self.refresh_desc_nonpassive(pi);
        }
        self.refresh_inert(ci);
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
        self.detach_as(parent, child, false);
    }

    /// [`detach`](Self::detach); `keep_input` (a move inside the same window) keeps the subtree's
    /// pointer capture, press, focus and hover.
    fn detach_as(&mut self, parent: NodeId, child: NodeId, keep_input: bool) {
        let pi = self.ix(parent);
        let ci = self.ix(child);
        self.unhoist(pi, child);
        self.children[pi].retain(|&c| c != child);
        self.core[ci].parent = None;
        self.core[ci].slot = None;
        if let Some(w) = self.core[ci].window {
            self.forget_in_window(w, ci, keep_input);
        }
        self.set_window(ci, None);
        self.structure_changed(pi);
        if self.core[ci].flags.intersects(NodeFlags::NONPASSIVE.union(NodeFlags::DESC_NONPASSIVE)) {
            self.refresh_desc_nonpassive(pi);
        }
        self.refresh_inert(ci);
    }

    /// `child`'s subtree leaves the tree above it: absolute boxes in it leave the hoisted lists of
    /// their containing blocks (`parent` and its ancestors). A containing block whose layout is
    /// cached keeps its list, and rounding walks it: a destroyed box was a stale id, a moved one got
    /// this tree's layout (A31).
    fn unhoist(&mut self, parent: usize, child: NodeId) {
        let mut a = Some(parent);
        while let Some(i) = a {
            if !self.layout[i].hoisted.is_empty() {
                let mut list = std::mem::take(&mut self.layout[i].hoisted);
                list.retain(|&h| !self.in_subtree(h, child));
                self.layout[i].hoisted = list;
            }
            a = self.core[i].parent.map(|p| self.ix(p));
        }
    }

    /// `n` is `root` or below it; a stale `n` counts as inside (it is dropped either way).
    fn in_subtree(&self, n: NodeId, root: NodeId) -> bool {
        let mut cur = n;
        loop {
            if cur == root {
                return true;
            }
            let Some(i) = self.idx(cur) else { return true };
            match self.core[i].parent {
                Some(p) => cur = p,
                None => return false,
            }
        }
    }

    /// Destroys `node` and its subtree (removing it from its parent first). Stale ids are ignored.
    pub fn destroy(&mut self, node: NodeId) {
        let Some(i) = self.idx(node) else { return };
        let rev = self.stamp();
        if let Some(p) = self.core[i].parent {
            self.detach(p, node);
        } else if let Some(w) = self.core[i].window {
            self.forget_in_window(w, i, false);
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
        self.kind_sets.remove(self.core[i].kind, id);
        self.kinds[i] = None;
        self.props[i] = PropStore::default();
        self.listeners[i].clear();
        self.native_rev.retain(|(n, _), _| *n != id);
        self.input.forget(id);
        self.style.reset(i);
        self.anim.forget(id);
        self.scroll.remove(&id);
        self.timers.remove(&id);
        self.virt.remove(&id);
        self.anchors.remove(&id);
        self.label_for.remove(&id);
        self.peer_queue.retain(|(n, ..)| *n != id);
        self.missed_props.retain(|(n, _)| *n != id);
        self.self_scroll.remove(&id);
        self.light_dismiss.remove(&id);
        if self.selection_owner == Some(id) {
            self.selection_owner = None;
        }
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
    /// commit's removals; focus, hover and capture inside it are dropped unless `keep_input` (the
    /// subtree is moving inside `w`).
    fn forget_in_window(&mut self, w: WindowId, i: usize, keep_input: bool) {
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
        if !keep_input {
            crate::input::focus::scopes_closed(self, w, &gone);
        }
        for &id in &gone {
            let n = self.ix(id);
            let paint = std::mem::take(&mut self.record[n]);
            if let Some(ws) = self.window_mut(w) {
                ws.retire(id, paint, keep_input);
            }
            self.core[n].flags.remove(NodeFlags::IN_RELAYOUT | NodeFlags::IN_REMEASURE);
            self.core[n].dirty.insert(Dirty::RECORD | Dirty::LAYOUT);
            self.layout[n].cache.clear();
        }
        if !keep_input {
            self.input.leave_subtree(&gone);
        }
        if let Some(ws) = self.window_mut(w) {
            for id in &gone {
                ws.a11y_sent.remove(id);
            }
        }
    }

    /// The children layout, recording and hit testing see: none for a folding kind.
    pub(crate) fn layout_children(&self, i: usize) -> &[NodeId] {
        if self.core[i].flags.contains(NodeFlags::FOLDS) { &[] } else { &self.children[i] }
    }

    /// Node `i` (a folded child, its props, style or text) changed: the folding ancestors directly
    /// above it remeasure and repaint.
    pub(crate) fn fold_up(&mut self, mut i: usize) {
        while let Some(p) = self.core[i].parent {
            let pi = self.ix(p);
            if !self.core[pi].flags.contains(NodeFlags::FOLDS) {
                return;
            }
            self.mark_measure(pi);
            self.mark(p, Dirty::PAINT | Dirty::A11Y);
            i = pi;
        }
    }

    /// The children of `pi` changed.
    fn structure_changed(&mut self, pi: usize) {
        if self.core[pi].flags.contains(NodeFlags::FOLDS) {
            self.mark_measure(pi);
            let id = self.slots.id_at(pi).expect("live");
            self.mark(id, Dirty::PAINT);
        }
        self.fold_up(pi);
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
        let registry = self.registry.clone();
        let entry = registry.get(kind).expect("registered kind");
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
        if self.kinds[i].is_none() && !native {
            // the kind is mid-handler (JS runs inside it): it hears of the write when it is back
            self.missed_props.push((node, prop));
        }
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
            let on = v.as_ref().and_then(Value::as_bool).unwrap_or(false) || self.core[i].flags.contains(NodeFlags::INERT);
            self.set_state(node, StateMask::DISABLED, on);
        } else if prop == common::INVALID {
            let on = v.as_ref().and_then(Value::as_bool).unwrap_or(false);
            self.set_state(node, StateMask::INVALID, on);
        } else if prop == common::ANCHOR {
            match v.as_ref().and_then(Value::as_str).filter(|a| !a.is_empty()) {
                Some(a) => self.anchors.insert(node, Arc::from(a)),
                None => self.anchors.remove(&node),
            };
            // its `labelled_by` follows the labels naming the new anchor
            self.mark(node, Dirty::A11Y);
        } else if prop == common::FOCUS_SCOPE || prop == common::AUTOFOCUS {
            if crate::input::focus::watched(self, node) {
                self.focus_watch.insert(node);
            } else {
                self.focus_watch.remove(&node);
            }
        } else if prop == common::DIALOG_ROLE || prop == common::STATUS_SECTION {
            // the containers that order their children by it
            if let Some(p) = self.core[i].parent {
                self.mark_layout(p);
                self.mark(p, Dirty::PAINT);
            }
        } else if spec.name == "default" && spec.ty == crate::value::ValueType::Bool && entry.prop(prop).is_some() {
            let on = v.as_ref().and_then(Value::as_bool).unwrap_or(false);
            self.set_state(node, StateMask::DEFAULT, on);
            if self.core[i].window.is_some() {
                crate::input::scope::refresh_default(self, node);
            }
        } else if spec.name == "for" && entry.prop(prop).is_some() {
            let named = v.as_ref().and_then(Value::as_str).filter(|a| !a.is_empty()).map(Arc::<str>::from);
            let old = match named.clone() {
                Some(a) => self.label_for.insert(node, a),
                None => self.label_for.remove(&node),
            };
            for a in old.into_iter().chain(named) {
                if let Some(t) = self.core[i].window.and_then(|w| self.find_anchor(w, &a)) {
                    self.mark(t, Dirty::A11Y);
                }
            }
        }
        // a kind that derives layout from props (orientation, chrome) gets its layout style again
        if class == ChangeClass::Layout && !is_text {
            self.core[i].dirty.insert(Dirty::STYLE);
            self.mark_up(i, Dirty::DESC_STYLE);
            if entry.spec.places_children {
                self.replace_children(i);
            }
        }
        self.refresh_prop_state(i);
        self.refresh_children_inert(i);
        if let Some(k) = self.kinds[i].take() {
            self.refresh_light_dismiss(node, &*k);
            self.kinds[i] = Some(k);
        }
        if [common::ARIA_LIVE, common::ARIA_BUSY, common::ROLE, common::ARIA_LEVEL, common::ARIA_EXPANDED, common::ARIA_SELECTED, common::ARIA_DESCRIPTION].contains(&prop) {
            self.mark(node, Dirty::A11Y);
        }
        // ancestors named by their descendants' text re-project
        if is_text || prop == common::ARIA_LABEL {
            let mut a = self.core[i].parent;
            while let Some(p) = a {
                let pi = self.ix(p);
                if self.registry.get(self.core[pi].kind).is_some_and(|e| e.spec.name_from.contains(&crate::kind::NameSource::Children)) {
                    self.mark(p, Dirty::A11Y);
                }
                a = self.core[pi].parent;
            }
        }
        // attribute selectors may match the prop
        self.restyle_attr(i, spec.name);
        self.fold_up(i);
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
        let was = self.listeners[i].iter().any(|l| l.signal == signal);
        let capture = flags.contains(ListenFlags::CAPTURE);
        let ls = &mut self.listeners[i];
        ls.retain(|l| !(l.signal == signal && l.flags.contains(ListenFlags::CAPTURE) == capture));
        ls.push(Listener { signal, flags });
        self.update_passive(i);
        if !was {
            self.listen_changed(i, signal, true);
        }
    }

    /// Removes a listener.
    pub fn unlisten(&mut self, node: NodeId, signal: SignalId, capture: bool) {
        let Some(i) = self.idx(node) else { return };
        let was = self.listeners[i].iter().any(|l| l.signal == signal);
        self.listeners[i].retain(|l| !(l.signal == signal && l.flags.contains(ListenFlags::CAPTURE) == capture));
        self.update_passive(i);
        if was && !self.listeners[i].iter().any(|l| l.signal == signal) {
            self.listen_changed(i, signal, false);
        }
    }

    /// Tells node `i`'s kind that JS started or stopped listening to `signal`.
    fn listen_changed(&mut self, i: usize, signal: SignalId, listened: bool) {
        let Some(mut k) = self.kinds[i].take() else { return };
        let request = {
            let mut cx = PropCx { node: self.view_at(i), request: Dirty::EMPTY };
            k.listen_changed(&mut cx, signal, listened);
            cx.request
        };
        self.kinds[i] = Some(k);
        self.apply_request(i, request);
    }

    /// The extra work a kind asked for in a [`PropCx`].
    fn apply_request(&mut self, i: usize, request: Dirty) {
        if request.is_empty() {
            return;
        }
        if request.contains(Dirty::MEASURE) {
            self.mark_measure(i);
        }
        if request.contains(Dirty::LAYOUT) {
            self.invalidate_style(i);
        }
        let node = self.slots.id_at(i).expect("live");
        self.mark(node, request);
    }

    /// Whether `node`'s kind `k` asks for light dismiss now.
    pub(crate) fn refresh_light_dismiss(&mut self, node: NodeId, k: &dyn Kind) {
        let Some(i) = self.idx(node) else { return };
        if k.light_dismiss(&self.view_at(i)) {
            self.light_dismiss.insert(node);
        } else {
            self.light_dismiss.remove(&node);
        }
    }

    /// Sets the bits of the kind's [`KindSpec::prop_states`](crate::KindSpec::prop_states) as its
    /// [`Kind::prop_state`] says.
    pub(crate) fn refresh_prop_state(&mut self, i: usize) {
        let mask = self.registry.get(self.core[i].kind).map_or(StateMask::NONE, |e| e.spec.prop_states);
        if mask == StateMask::NONE {
            return;
        }
        let Some(bits) = self.kinds[i].as_deref().map(|k| k.prop_state(&self.view_at(i))) else { return };
        let node = self.slots.id_at(i).expect("live");
        self.set_state(node, StateMask(mask.0 & bits.0), true);
        self.set_state(node, StateMask(mask.0 & !bits.0), false);
    }

    /// The kind of node `i` may have started or stopped making its descendants inert.
    fn refresh_children_inert(&mut self, i: usize) {
        let inert = self.kinds[i].as_deref().is_some_and(|k| k.children_inert());
        if inert == self.core[i].flags.contains(NodeFlags::INERT_CHILDREN) {
            return;
        }
        if inert {
            self.core[i].flags.insert(NodeFlags::INERT_CHILDREN);
        } else {
            self.core[i].flags.remove(NodeFlags::INERT_CHILDREN);
        }
        for c in self.children[i].clone() {
            let ci = self.ix(c);
            self.refresh_inert(ci);
        }
    }

    /// Recomputes [`NodeFlags::INERT`] (and so `:disabled`) on `i`'s subtree from its ancestors.
    pub(crate) fn refresh_inert(&mut self, i: usize) {
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            let inherited = self.core[n].parent.is_some_and(|p| {
                let pi = self.ix(p);
                self.core[pi].flags.intersects(NodeFlags::INERT.union(NodeFlags::INERT_CHILDREN))
            });
            if inherited == self.core[n].flags.contains(NodeFlags::INERT) && n != i {
                continue;
            }
            if inherited {
                self.core[n].flags.insert(NodeFlags::INERT);
            } else {
                self.core[n].flags.remove(NodeFlags::INERT);
            }
            let id = self.slots.id_at(n).expect("live");
            let own = self.view_at(n).prop_bool(common::DISABLED);
            self.set_state(id, StateMask::DISABLED, own || inherited);
            if inherited && self.core[n].window.is_some_and(|w| self.focused(w) == Some(id)) {
                crate::input::focus::clear_focus(self, self.core[n].window.expect("in a window"));
            }
            stack.extend(self.children[n].iter().map(|&c| self.ix(c)));
        }
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
            if let Some(p) = self.core[i].parent {
                let pi = self.ix(p);
                self.refresh_desc_nonpassive(pi);
            }
        }
    }

    /// Recomputes [`NodeFlags::DESC_NONPASSIVE`] on `i` and up its ancestors until a node's bit does
    /// not change.
    pub(crate) fn refresh_desc_nonpassive(&mut self, mut i: usize) {
        let any = NodeFlags::NONPASSIVE.union(NodeFlags::DESC_NONPASSIVE);
        loop {
            let has = self.children[i].iter().any(|&c| self.core[self.ix(c)].flags.intersects(any));
            if has == self.core[i].flags.contains(NodeFlags::DESC_NONPASSIVE) {
                return;
            }
            if has {
                self.core[i].flags.insert(NodeFlags::DESC_NONPASSIVE);
            } else {
                self.core[i].flags.remove(NodeFlags::DESC_NONPASSIVE);
            }
            self.core[i].dirty.insert(Dirty::PROPERTY);
            self.mark_up(i, Dirty::DESC_RECORD);
            match self.core[i].parent {
                Some(p) => i = self.ix(p),
                None => return,
            }
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

    /// The host's provider states (cadatomic's `cad.pending`, kinds' state bits): sheets added from now
    /// on resolve `:state(ns.name)` through `names`, and a rule matches a node whose state has that bit
    /// ([`Document::set_state`], `EventCx::set_state`). Without it `:state()` is a parse error.
    pub fn set_state_names(&mut self, names: Box<dyn svelters_style::StateNames>) {
        self.style.state_names = Some(names);
    }
    /// The resolver [`Document::set_state_names`] set, for parsing sheets elsewhere (a precompiled one).
    pub fn state_names(&self) -> Option<&dyn svelters_style::StateNames> {
        self.style.state_names.as_deref()
    }

    pub fn remove_sheet(&mut self, id: svelters_style::SheetId) {
        self.style.remove_sheet(id);
        self.restyle_all();
    }

    /// Adds a sheet already parsed (a precompiled one, `svelters_style::binary`), as [`Document::add_sheet`].
    pub fn add_parsed_sheet(&mut self, sheet: &svelters_style::Stylesheet) -> svelters_style::SheetId {
        let id = self.style.add_parsed(sheet);
        self.restyle_all();
        id
    }

    /// Loads resolved utility rules (a precompiled sheet's) without the vocabulary lookup; returns
    /// how many were added (none while utilities are off).
    pub fn preload_utilities(&mut self, utilities: &[svelters_style::Utility]) -> usize {
        let n = self.style.preload_utilities(utilities);
        if n > 0 {
            self.restyle_all();
        }
        n
    }

    /// Restyles, re-measures and re-records every node of every window: a service the kinds paint or
    /// measure from changed (the widgets' theme: scheme, contrast, density, accent, token overrides).
    pub fn repaint_all(&mut self) {
        self.restyle_all();
        for i in 0..self.core.len() {
            let Some(node) = self.slots.id_at(i) else { continue };
            self.mark_measure(i);
            self.mark(node, Dirty::PAINT | Dirty::A11Y);
        }
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
        // pointer-only states are not in the accessibility tree: a hover over unstyled nodes costs
        // no frame (`needs_frame`)
        let pointer_only = StateMask(StateMask::HOVERED.0 | StateMask::PRESSED.0 | StateMask::FOCUS_WITHIN.0);
        if (old.0 ^ new.0) & !pointer_only.0 != 0 {
            self.core[i].dirty.insert(Dirty::A11Y);
            self.mark_up(i, Dirty::DESC_RECORD);
        }
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

    /// Moves keyboard focus to `node` (in its window): in this transaction when it can take focus now;
    /// when only being hidden stops it (it or an ancestor may be shown in this transaction), after the
    /// frame's next restyle, with its focus events then. A node still hidden then, or one that can't
    /// take focus at all, doesn't take it (as the DOM's `focus()`).
    pub fn focus(&mut self, node: NodeId, reason: FocusReason) {
        crate::input::focus::request_focus(self, node, reason);
    }

    /// The node a [`Document::focus`] call waits to focus at the next restyle (`None`: none).
    pub fn pending_focus(&self, window: WindowId) -> Option<NodeId> {
        crate::input::focus::pending_focus(self, window)
    }

    /// Clears focus in `window` (and a focus request waiting for style).
    pub fn blur(&mut self, window: WindowId) {
        crate::input::focus::clear_focus(self, window);
    }

    /// The focused node of `window`.
    pub fn focused(&self, window: WindowId) -> Option<NodeId> {
        self.window(window).and_then(|w| w.focus)
    }

    /// What has focus in `window`: the focused node and the part inside it that has it.
    pub fn focus_target(&self, window: WindowId) -> Option<crate::FocusTarget> {
        self.window(window).and_then(|w| w.focus.map(|node| crate::FocusTarget { node, sub: w.focus_sub }))
    }

    /// Sets the part of `window`'s focused node that has focus (`None`: the node itself); the
    /// accessibility tree's focus follows at the next frame. No-op without a focused node.
    pub fn set_focus_sub(&mut self, window: WindowId, sub: Option<u32>) {
        let Some(w) = self.window_mut(window) else { return };
        let Some(f) = w.focus else { return };
        if w.focus_sub != sub {
            w.focus_sub = sub;
            self.mark(f, Dirty::A11Y);
        }
    }

    /// JS events queued outside dispatch (focus moves from `focus()`, signals kinds emitted while
    /// preparing): svelters-skelter delivers them after its own task.
    pub fn take_events(&mut self) -> Vec<crate::input::JsEvent> {
        std::mem::take(&mut self.input_events)
    }

    // ---- services, timers, methods ----

    /// Lends kinds a service by type (replacing one of the same type).
    pub fn set_service<T: 'static>(&mut self, t: T) -> Option<T> {
        self.services.insert(t)
    }
    pub fn service<T: 'static>(&mut self) -> Option<&mut T> {
        self.services.get_mut()
    }

    pub(crate) fn set_timer(&mut self, node: NodeId, at: Option<skelter_base::time::Time>) {
        match at {
            Some(t) => {
                self.timers.insert(node, t);
            }
            None => {
                self.timers.remove(&node);
            }
        }
    }

    /// The earliest kind deadline (the platform's `set_timer`).
    pub fn next_timer(&self) -> Option<skelter_base::time::Time> {
        self.timers.values().min().copied()
    }

    /// Delivers [`Event::Timer`](crate::Event::Timer) to every node whose deadline is at or before
    /// `now`, as one transaction; returns the JS events their kinds emitted.
    pub fn run_timers(&mut self, now: skelter_base::time::Time) -> Vec<crate::input::JsEvent> {
        let mut due: Vec<(skelter_base::time::Time, NodeId)> = self.timers.iter().filter(|&(_, &t)| t <= now).map(|(&n, &t)| (t, n)).collect();
        if due.is_empty() {
            return Vec::new();
        }
        due.sort();
        let mut emitted = Vec::new();
        self.transaction(|doc| {
            for (t, n) in due {
                doc.timers.remove(&n);
                doc.input.now = t;
                crate::input::dispatch::call_kind(doc, n, &crate::kind::Event::Timer, &mut emitted, None);
            }
        });
        for e in emitted {
            crate::input::dispatch::queue_emitted(self, e);
        }
        self.take_events()
    }

    /// Calls a kind's method (from JS). Arguments are checked against the spec's params.
    pub fn call(&mut self, node: NodeId, method: crate::ids::MethodId, args: &[Value]) -> Result<(Option<Value>, Vec<crate::input::JsEvent>), PropError> {
        let i = self.idx(node).ok_or(PropError::Stale(node))?;
        let entry = self.registry.get(self.core[i].kind).expect("registered kind");
        let spec = entry.method(method).ok_or_else(|| PropError::Rejected(format!("unknown method {method:?}")))?;
        let required = spec.params.iter().filter(|p| !p.nullable).count();
        if args.len() < required || args.len() > spec.params.len() {
            return Err(PropError::Rejected(format!("{} takes {} arguments, got {}", spec.name, spec.params.len(), args.len())));
        }
        let mut checked = Vec::with_capacity(args.len());
        for (a, p) in args.iter().zip(spec.params) {
            let got = a.ty();
            checked.push(a.clone().coerce(p.ty).ok_or(PropError::Rejected(format!("{}: argument `{}` takes {}, got {}", spec.name, p.name, p.ty.name(), got.name())))?);
        }
        let mut k = self.kinds[i].take().expect("kind present");
        let mut emitted = Vec::new();
        let r = self.transaction(|doc| {
            let mut cx = crate::cx::EventCx { doc, node, emitted: &mut emitted, pointer: None };
            k.call(&mut cx, method, &checked)
        });
        self.restore_kind(node, k);
        if !self.peer_queue.is_empty() {
            self.transaction(|doc| crate::input::peers::run(doc, &mut emitted));
        }
        for e in emitted {
            crate::input::dispatch::queue_emitted(self, e);
        }
        Ok((r?, self.take_events()))
    }

    /// An assistive technology's action request: the target's kind handles it first; unhandled
    /// `Focus`, `Blur` and `ScrollIntoView` get their defaults.
    /// An action on a virtual child ([`crate::a11y::virtual_id`]) reaches its owner's kind as
    /// [`Event::A11yChild`](crate::Event::A11yChild); unhandled, `Focus` focuses the owner.
    pub fn a11y_action(&mut self, window: WindowId, req: &accesskit::ActionRequest) -> crate::input::Dispatched {
        let child = crate::a11y::virtual_child(req.target_node);
        let node = child.map_or(NodeId(req.target_node.0), |(owner, _)| owner);
        let mut out = crate::input::Dispatched { window, revision: self.revision, ..Default::default() };
        if self.window_of(node) != Some(window) {
            return out;
        }
        out.target = Some(node);
        let mut emitted = Vec::new();
        let before = self.revision;
        self.transaction(|doc| {
            let ev = match child {
                Some((_, index)) => crate::kind::Event::A11yChild(req, index),
                None => crate::kind::Event::A11y(req),
            };
            let handled = !doc.state(node).contains(StateMask::DISABLED) && crate::input::dispatch::call_kind(doc, node, &ev, &mut emitted, None) == crate::kind::Handled::Yes;
            out.handled = handled;
            if !handled {
                match req.action {
                    accesskit::Action::Focus => {
                        doc.focus(node, FocusReason::Programmatic);
                        out.handled = doc.focused(window) == Some(node);
                    }
                    accesskit::Action::Blur => doc.blur(window),
                    accesskit::Action::ScrollIntoView => doc.scroll_into_view(node),
                    _ => {}
                }
            }
        });
        for e in emitted {
            crate::input::dispatch::queue_emitted(self, e);
        }
        out.events = self.take_events();
        out.changed = self.revision > before && self.last_change >= self.revision;
        out.revision = self.revision;
        out
    }

    /// Drains the change journal (svelters-skelter reads it for `bind:` and observers).
    pub fn drain_changes(&mut self) -> Vec<Change> {
        self.journal.drain()
    }
}
