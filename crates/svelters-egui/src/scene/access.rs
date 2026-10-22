//! The access API (ARCH-SCENE §6): [`SceneHandle`] (the entry point, `Rc`, UI thread), the read
//! views [`SceneRef`]/[`NodeRef`], the mutation views [`SceneMut`]/[`NodeMut`], and the one write
//! path for props ([`Scene::set_value`]) every origin goes through.
//!
//! Phases (§5.2 with R17): `write` works in `Idle` and `Logic`; from the end of the embed's logic to
//! the end of the egui pass (`Ui`) it returns `Err(SceneBusy)`, and `defer` queues the mutation for
//! the start of the next logic. A non-empty defer queue is pending work (R16):
//! [`SceneHandle::deferred_len`] feeds `Busy` (EM1's `embed/settle.rs`).
//!
//! Prop writes from Rust (R18): [`NodeMut::set`] stores the value, marks the dirty bits and, when a
//! runtime observes the prop, announces it (`prop_changed`) at the next logic. It never fires the
//! kind's events (input, change, …): Rust writes are not user input.
//!
//! Writer lane: SCENE.

use super::dump::{DumpOpts, SceneStats};
use super::id::{Handle, KindId, NodeId, PropId, ProviderId, RootIx, Tick};
use super::keys::{PropKey, PropValue};
use super::node::{Common, Dirty, NodeFlags, Owner, StateBits, W};
use super::props;
use super::tree::TreeRoot;
use super::{Member, Phase, Scene, member_bit};
use crate::backend::EguiBackend;
use crate::kinds::{self, KINDS};
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::Rc;
use svelters_abi::Value;

/// Who wrote (recorded per change from SG2's journal on).
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Origin {
    /// A runtime's host op.
    Js,
    /// A widget from user input during the walk.
    Widget,
    /// Rust code through `SceneMut`.
    Rust(ProviderId),
    /// The CSS engine (CS5e).
    Style,
    /// The animator (TR1 channels).
    Animator,
    /// The embed itself.
    Host,
}

/// `SceneHandle::write` outside `Idle`/`Logic`, or while the scene is already borrowed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SceneBusy;

impl std::fmt::Display for SceneBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the scene is busy (drawing or borrowed): use SceneHandle::defer")
    }
}
impl std::error::Error for SceneBusy {}

#[non_exhaustive]
#[derive(Clone, PartialEq, Debug)]
pub enum SceneError {
    /// A stale or foreign id (raw handle).
    Stale(u64),
    /// The origin may not change this node's structure (S3: Rust edits Rust- and Host-owned
    /// structure only).
    NotOwner { node: NodeId, owner: Owner },
    /// A kind rule (no children, toplevel, cycle, `before` not a child, …).
    Invalid(String),
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SceneError::Stale(h) => write!(f, "stale or foreign handle {h}"),
            SceneError::NotOwner { node, owner } => write!(f, "{node} is owned by {owner:?}: not editable from this origin"),
            SceneError::Invalid(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for SceneError {}

#[non_exhaustive]
#[derive(Clone, PartialEq, Debug)]
pub enum PropError {
    Stale(u64),
    /// The kind has no such prop.
    Unknown(String),
    /// The value does not fit the prop.
    Invalid(String),
}

impl std::fmt::Display for PropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropError::Stale(h) => write!(f, "stale or foreign handle {h}"),
            PropError::Unknown(m) | PropError::Invalid(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for PropError {}

type Deferred = Box<dyn FnOnce(&mut SceneMut<'_>)>;

pub(crate) struct SceneCell {
    scene: RefCell<Scene>,
    phase: Cell<Phase>,
    deferred: RefCell<Vec<(ProviderId, Deferred)>>,
    /// Asks for a frame (set by the backend): queued work needs a logic pass.
    wake: RefCell<Option<Box<dyn Fn()>>>,
}

/// The scene of one embed (today: one `EguiBackend`). `Rc`, `!Send`: UI thread only. From
/// [`EguiBackend::scene`].
#[derive(Clone)]
pub struct SceneHandle(Rc<SceneCell>);

impl Default for SceneHandle {
    fn default() -> Self {
        SceneHandle(Rc::new(SceneCell { scene: RefCell::new(Scene::default()), phase: Cell::new(Phase::Idle), deferred: RefCell::new(Vec::new()), wake: RefCell::new(None) }))
    }
}

impl SceneHandle {
    pub(crate) fn borrow(&self) -> Ref<'_, Scene> {
        self.0.scene.borrow()
    }

    pub(crate) fn borrow_mut(&self) -> RefMut<'_, Scene> {
        self.0.scene.borrow_mut()
    }

    pub(crate) fn set_phase(&self, p: Phase) {
        self.0.phase.set(p);
    }

    pub(crate) fn set_wake(&self, f: impl Fn() + 'static) {
        *self.0.wake.borrow_mut() = Some(Box::new(f));
    }

    fn wake(&self) {
        if let Some(f) = self.0.wake.borrow().as_ref() {
            f();
        }
    }

    pub fn phase(&self) -> Phase {
        self.0.phase.get()
    }

    /// Read access, any phase. Panics if called while the scene is being mutated (inside a
    /// `write` closure or from a widget during the walk): use [`SceneHandle::try_read`] there.
    pub fn read<R>(&self, f: impl FnOnce(SceneRef<'_>) -> R) -> R {
        let s = self.0.scene.try_borrow().expect("svelters-egui: SceneHandle::read while the scene is being mutated (use try_read or defer)");
        f(SceneRef { s: &s })
    }

    pub fn try_read<R>(&self, f: impl FnOnce(SceneRef<'_>) -> R) -> Result<R, SceneBusy> {
        let s = self.0.scene.try_borrow().map_err(|_| SceneBusy)?;
        Ok(f(SceneRef { s: &s }))
    }

    /// Write access in `Idle`/`Logic`, as one batch (one tick). `Err(SceneBusy)` in `Ui` (R17) or
    /// when the scene is already borrowed; use [`SceneHandle::defer`] there.
    pub fn write<R>(&self, origin: ProviderId, f: impl FnOnce(&mut SceneMut<'_>) -> R) -> Result<R, SceneBusy> {
        if self.0.phase.get() == Phase::Ui {
            if let Ok(mut s) = self.0.scene.try_borrow_mut() {
                s.counters.writes_busy += 1;
            }
            return Err(SceneBusy);
        }
        let r = self.write_now(Origin::Rust(origin), f)?;
        self.wake();
        Ok(r)
    }

    fn write_now<R>(&self, origin: Origin, f: impl FnOnce(&mut SceneMut<'_>) -> R) -> Result<R, SceneBusy> {
        let mut s = self.0.scene.try_borrow_mut().map_err(|_| SceneBusy)?;
        let was = s.in_batch;
        if !was {
            s.tick += 1;
            s.in_batch = true;
        }
        s.counters.writes += 1;
        let r = f(&mut SceneMut { s: &mut s, origin });
        s.in_batch = was;
        Ok(r)
    }

    /// Queues a mutation for the start of the next logic (callable in any phase on the UI
    /// thread, including from widgets during the walk).
    pub fn defer(&self, origin: ProviderId, f: impl FnOnce(&mut SceneMut<'_>) + 'static) {
        self.0.deferred.borrow_mut().push((origin, Box::new(f)));
        self.wake();
    }

    /// Deferred mutations waiting for the next logic (R16: non-zero is pending work).
    pub fn deferred_len(&self) -> usize {
        self.0.deferred.borrow().len()
    }

    /// Runs the deferred mutations (logic step 2b). Ones deferred meanwhile wait for the next logic.
    pub(crate) fn run_deferred(&self) -> usize {
        let q = std::mem::take(&mut *self.0.deferred.borrow_mut());
        let n = q.len();
        for (origin, f) in q {
            if self.write_now(Origin::Rust(origin), f).is_err() {
                panic!("svelters-egui: deferred scene write while the scene is borrowed");
            }
        }
        if n > 0 {
            self.borrow_mut().counters.deferred_run += n as u64;
        }
        n
    }

    /// The prop changes Rust writes announced since the last call (R18), for `prop_changed`.
    pub(crate) fn take_announcements(&self) -> Vec<(Handle, u32)> {
        let mut s = self.borrow_mut();
        let all = std::mem::take(&mut s.announce);
        let v: Vec<(Handle, u32)> = all.into_iter().filter(|(id, _)| s.idx(*id).is_some()).map(|(id, p)| (id.to_raw(), p)).collect();
        s.counters.announced += v.len() as u64;
        v
    }
}

// ---- reads ----

/// Read access to the whole scene.
#[derive(Copy, Clone)]
pub struct SceneRef<'a> {
    pub(crate) s: &'a Scene,
}

impl<'a> SceneRef<'a> {
    pub(crate) fn new(s: &'a Scene) -> SceneRef<'a> {
        SceneRef { s }
    }

    pub fn get(self, id: NodeId) -> Option<NodeRef<'a>> {
        let i = self.s.idx(id)?;
        Some(NodeRef { s: self.s, id, i })
    }

    /// [`SceneRef::get`] by ABI handle.
    pub fn get_raw(self, h: Handle) -> Option<NodeRef<'a>> {
        self.get(NodeId::from_raw(h)?)
    }

    /// Live nodes (O(1)).
    pub fn len(self) -> usize {
        self.s.len()
    }

    pub fn is_empty(self) -> bool {
        self.s.is_empty()
    }

    /// The current tick (the last mutation batch's).
    pub fn tick(self) -> Tick {
        Tick(self.s.tick)
    }

    /// The roots and what they are.
    pub fn roots(self) -> impl Iterator<Item = (RootIx, TreeRoot)> + 'a {
        self.s.roots.iter().map(|(r, e)| (r, e.kind))
    }

    /// The top node of root `r`.
    pub fn root_top(self, r: RootIx) -> Option<NodeRef<'a>> {
        self.get(self.s.roots.get(r)?.top)
    }

    /// Every live node, in slot order (O(slots)).
    pub fn nodes(self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.s.ids().map(move |id| NodeRef { s: self.s, id, i: id.index() as usize })
    }

    /// The live nodes of element tag `tag`, in slot order (O(slots) until SG3's kind index).
    pub fn of_tag(self, tag: &'a str) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        self.nodes().filter(move |n| n.tag() == tag)
    }

    pub fn stats(self) -> SceneStats {
        self.s.stats()
    }

    /// The scene's invariants (ARCH-SCENE §11).
    pub fn validate(self) -> Result<(), String> {
        self.s.validate()
    }

    /// A stable text dump of `top`'s subtree.
    pub fn dump(self, top: NodeId, opts: DumpOpts) -> String {
        self.s.dump(top, opts)
    }
}

/// Read access to one live node.
#[derive(Copy, Clone)]
pub struct NodeRef<'a> {
    s: &'a Scene,
    id: NodeId,
    i: usize,
}

impl<'a> NodeRef<'a> {
    pub fn id(self) -> NodeId {
        self.id
    }
    pub fn kind(self) -> KindId {
        KindId(kinds::kind_id(self.kind_index()))
    }
    pub(crate) fn kind_index(self) -> usize {
        self.s.core[self.i].kind as usize
    }
    /// The element tag (`label`).
    pub fn tag(self) -> &'static str {
        self.s.tag_at(self.i)
    }
    pub fn owner(self) -> Owner {
        self.s.core[self.i].owner
    }
    pub fn root(self) -> RootIx {
        self.s.core[self.i].root
    }
    /// The light parent.
    pub fn parent(self) -> Option<NodeRef<'a>> {
        self.s.core[self.i].parent.map(|p| NodeRef { s: self.s, id: p, i: p.index() as usize })
    }
    pub fn children(self) -> Children<'a> {
        Children { s: self.s, it: self.s.children[self.i].iter() }
    }
    /// The subtree in pre-order, this node first.
    pub fn descendants(self) -> Descendants<'a> {
        Descendants { s: self.s, stack: vec![self.id], last: None }
    }
    /// The parent chain, nearest first.
    pub fn ancestors(self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        std::iter::successors(self.parent(), |n| n.parent())
    }
    pub fn egui_id(self) -> egui::Id {
        self.s.core[self.i].egui_id
    }
    /// Root-relative rect of the last layout.
    pub fn rect(self) -> egui::Rect {
        self.s.core[self.i].rect
    }
    /// The screen rect drawn last frame, if drawn.
    pub fn drawn(self) -> Option<egui::Rect> {
        let r = self.s.core[self.i].drawn;
        (r != egui::Rect::NOTHING).then_some(r)
    }
    pub fn state(self) -> StateBits {
        self.s.core[self.i].state
    }
    pub fn flags(self) -> NodeFlags {
        self.s.core[self.i].flags
    }
    /// Unstable (ARCH-SCENE §16): what the node still needs.
    pub fn dirty(self) -> Dirty {
        self.s.core[self.i].dirty
    }
    /// True if a layout boundary: content changes inside it re-lay out it alone.
    pub fn is_layout_boundary(self) -> bool {
        self.s.is_boundary(self.i)
    }
    /// The `visible` prop of this node and its ancestors.
    pub fn is_visible(self) -> bool {
        self.s.common(self.i).visible && self.ancestors().all(|a| a.s.common(a.i).visible)
    }
    /// The tick of this node's last change.
    pub fn changed(self) -> Tick {
        self.s.core[self.i].changed
    }
    pub fn changed_since(self, t: Tick) -> bool {
        self.s.core[self.i].changed > t
    }
    /// Typed prop read, zero-copy: `n.get(&keys::label::TEXT)`. `None` if the kind has no such prop.
    pub fn get<T: 'static>(self, key: &PropKey<T>) -> Option<&'a T> {
        kinds::find_prop(self.kind_index(), key.name)?;
        key.read(self.s.common(self.i), self.s.w(self.i))
    }
    /// Untyped read by id, as the host reads (`get_prop`).
    pub fn value(self, prop: PropId) -> Result<Value, PropError> {
        let def = kinds::prop_def(self.kind_index(), prop.0).ok_or_else(|| PropError::Unknown(format!("unknown prop id {} on <{}>", prop.0, self.tag())))?;
        props::get_prop(&self.view(), def).map_err(PropError::Invalid)
    }
    /// Untyped read by prop name.
    pub fn value_of(self, name: &str) -> Result<Value, PropError> {
        let (_, def) = kinds::find_prop(self.kind_index(), name).ok_or_else(|| PropError::Unknown(format!("<{}> has no prop {name}", self.tag())))?;
        props::get_prop(&self.view(), def).map_err(PropError::Invalid)
    }
    /// The prop id of `name` on this node's kind.
    pub fn prop_id(self, name: &str) -> Option<PropId> {
        kinds::find_prop(self.kind_index(), name).map(|(id, _)| PropId(id))
    }
    /// The text it shows (a window: its title).
    pub fn text(self) -> &'a str {
        super::node::text_of(self.s.w(self.i))
    }
    /// The built-in widget fields (unstable: internal representation, §16).
    pub fn widget(self) -> &'a W {
        self.s.w(self.i)
    }
    pub fn common(self) -> &'a Common {
        self.s.common(self.i)
    }
    /// True if a runtime listens to signal id `signal`.
    pub fn listens(self, signal: u32) -> bool {
        self.s.listens(self.i, signal)
    }
    /// True if a runtime observes prop `prop` (R1).
    pub fn observes(self, prop: PropId) -> bool {
        self.s.observes(self.i, prop.0)
    }
    fn view(self) -> super::Node<'a> {
        self.s.get(self.id.to_raw()).expect("live")
    }
}

impl std::fmt::Debug for NodeRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} {}>", self.tag(), self.id)
    }
}

/// The light children of a node, in order.
pub struct Children<'a> {
    s: &'a Scene,
    it: std::slice::Iter<'a, NodeId>,
}

impl<'a> Iterator for Children<'a> {
    type Item = NodeRef<'a>;
    fn next(&mut self) -> Option<NodeRef<'a>> {
        let s = self.s;
        self.it.next().map(|&id| NodeRef { s, id, i: id.index() as usize })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl DoubleEndedIterator for Children<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let s = self.s;
        self.it.next_back().map(|&id| NodeRef { s, id, i: id.index() as usize })
    }
}

impl ExactSizeIterator for Children<'_> {}

/// A subtree in pre-order.
pub struct Descendants<'a> {
    s: &'a Scene,
    stack: Vec<NodeId>,
    last: Option<usize>,
}

impl Descendants<'_> {
    /// Skips the children of the node yielded last.
    pub fn skip_subtree(&mut self) {
        if let Some(n) = self.last.take() {
            self.stack.truncate(self.stack.len() - n);
        }
    }
}

impl<'a> Iterator for Descendants<'a> {
    type Item = NodeRef<'a>;
    fn next(&mut self) -> Option<NodeRef<'a>> {
        let id = self.stack.pop()?;
        let i = id.index() as usize;
        let kids = &self.s.children[i];
        self.stack.extend(kids.iter().rev().copied());
        self.last = Some(kids.len());
        Some(NodeRef { s: self.s, id, i })
    }
}

// ---- mutation ----

/// Mutation access (in `SceneHandle::write`, `defer`, and the crate's own op paths).
pub struct SceneMut<'a> {
    pub(crate) s: &'a mut Scene,
    pub(crate) origin: Origin,
}

impl<'a> SceneMut<'a> {
    pub fn read(&self) -> SceneRef<'_> {
        SceneRef { s: self.s }
    }

    pub fn origin(&self) -> Origin {
        self.origin
    }

    fn owner(&self) -> Owner {
        match self.origin {
            Origin::Js => Owner::Js,
            Origin::Rust(p) => Owner::Rust(p),
            _ => Owner::Host,
        }
    }

    /// Creates a detached node owned by this origin.
    pub fn create(&mut self, kind: KindId) -> Result<NodeId, SceneError> {
        let k = kinds::kind_of_id(kind.0).ok_or_else(|| SceneError::Invalid(format!("unknown kind id {}", kind.0)))?;
        let owner = self.owner();
        self.s.create(k, owner)
    }

    /// [`SceneMut::create`] by element tag.
    pub fn create_tag(&mut self, tag: &str) -> Result<NodeId, SceneError> {
        let k = kinds::tag_index(tag).ok_or_else(|| SceneError::Invalid(format!("unknown element <{tag}>")))?;
        let owner = self.owner();
        self.s.create(k, owner)
    }

    /// Moves `child` under `parent` before `before` (last when `None`). Both must be Rust- or
    /// Host-owned (S3), as must `child`'s old parent.
    pub fn insert(&mut self, parent: NodeId, child: NodeId, before: Option<NodeId>) -> Result<(), SceneError> {
        self.s.insert(parent, child, before, self.origin).map(|_| ())
    }

    /// Detaches `child` from its parent (it stays alive).
    pub fn remove(&mut self, child: NodeId) -> Result<(), SceneError> {
        let ci = self.s.idx(child).ok_or(SceneError::Stale(child.to_raw()))?;
        match self.s.core[ci].parent {
            Some(p) => self.s.remove(p, child, self.origin),
            None => Ok(()),
        }
    }

    /// Destroys `id` and its whole subtree; every node in it must be Rust- or Host-owned.
    pub fn destroy(&mut self, id: NodeId) -> Result<(), SceneError> {
        let i = self.s.idx(id).ok_or(SceneError::Stale(id.to_raw()))?;
        if !matches!(self.origin, Origin::Js) {
            for n in SceneRef::new(self.s).get(id).expect("live").descendants() {
                if n.owner() == Owner::Js {
                    return Err(SceneError::NotOwner { node: n.id(), owner: Owner::Js });
                }
            }
            if let Some(p) = self.s.core[i].parent
                && self.s.core[self.s.ix(p)].owner == Owner::Js
            {
                return Err(SceneError::NotOwner { node: p, owner: Owner::Js });
            }
        }
        self.s.free_subtree(id);
        Ok(())
    }

    /// Replaces `remove` children of `parent` from index `at` with `insert` (in order).
    pub fn splice(&mut self, parent: NodeId, at: usize, remove: usize, insert: &[NodeId]) -> Result<(), SceneError> {
        let pi = self.s.idx(parent).ok_or(SceneError::Stale(parent.to_raw()))?;
        let kids = &self.s.children[pi];
        if at > kids.len() || at + remove > kids.len() {
            return Err(SceneError::Invalid(format!("splice {at}+{remove} out of {} children", kids.len())));
        }
        let gone: Vec<NodeId> = kids[at..at + remove].to_vec();
        for c in gone {
            self.s.remove(parent, c, self.origin)?;
        }
        for &c in insert.iter().rev() {
            let before = self.s.children[pi].get(at).copied();
            self.s.insert(parent, c, before, self.origin)?;
        }
        Ok(())
    }

    /// Mutation access to one live node.
    pub fn node(&mut self, id: NodeId) -> Option<NodeMut<'_>> {
        let i = self.s.idx(id)?;
        Some(NodeMut { s: self.s, id, i, origin: self.origin })
    }

    /// Groups writes into one tick (SG2: one journal batch). A `write` is already one batch.
    pub fn batch<R>(&mut self, _label: &'static str, f: impl FnOnce(&mut Self) -> R) -> R {
        let was = self.s.in_batch;
        if !was {
            self.s.tick += 1;
            self.s.in_batch = true;
        }
        let r = f(self);
        self.s.in_batch = was;
        r
    }
}

/// Mutation access to one node.
pub struct NodeMut<'a> {
    s: &'a mut Scene,
    id: NodeId,
    i: usize,
    origin: Origin,
}

impl NodeMut<'_> {
    pub fn read(&self) -> NodeRef<'_> {
        NodeRef { s: self.s, id: self.id, i: self.i }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Typed write through the exact path (R18: announced when observed, no events).
    pub fn set<T: PropValue>(&mut self, key: &PropKey<T>, v: T) -> Result<(), PropError> {
        let (id, _) = kinds::find_prop(self.s.core[self.i].kind as usize, key.name).ok_or_else(|| PropError::Unknown(format!("<{}> has no prop {}", self.s.tag_at(self.i), key.name)))?;
        self.s.set_value(self.id, id, &v.into_value(), self.origin)
    }

    /// Untyped write by id.
    pub fn set_value(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.s.set_value(self.id, prop.0, v, self.origin)
    }

    /// Untyped write by prop name.
    pub fn set_by_name(&mut self, name: &str, v: &Value) -> Result<(), PropError> {
        let (id, _) = kinds::find_prop(self.s.core[self.i].kind as usize, name).ok_or_else(|| PropError::Unknown(format!("<{}> has no prop {name}", self.s.tag_at(self.i))))?;
        self.s.set_value(self.id, id, v, self.origin)
    }

    pub fn set_state(&mut self, bits: StateBits, on: bool) {
        let before = self.s.core[self.i].state;
        self.s.core[self.i].state.set(bits, on);
        if self.s.core[self.i].state != before {
            let t = self.s.stamp();
            self.s.core[self.i].changed = t;
            self.s.core[self.i].dirty.insert(Dirty::PAINT);
        }
    }

    /// Sets the public flags ([`NodeFlags::PUBLIC`]: `LAYOUT_BOUNDARY`, `HIT_TRANSPARENT`,
    /// `SKIP_OFFSCREEN`); others are ignored.
    pub fn set_flags(&mut self, flags: NodeFlags, on: bool) {
        let f = flags & NodeFlags::PUBLIC;
        self.s.core[self.i].flags.set(f, on);
        let t = self.s.stamp();
        self.s.core[self.i].changed = t;
    }

    /// Asks for work the scene cannot see (a provider's own state changed): MEASURE, LAYOUT
    /// (content), PAINT.
    pub fn invalidate(&mut self, what: Dirty) {
        if what.contains(Dirty::MEASURE) {
            self.s.mark_measure(self.id);
        }
        if what.contains(Dirty::LAYOUT) {
            self.s.invalidate_content(self.id);
        }
        self.s.core[self.i].dirty.insert(what & (Dirty::PAINT | Dirty::A11Y));
    }
}

// ---- the one write path ----

impl Scene {
    /// Writes prop `prop` of `id` from `origin`: stores it, marks the dirty bits (restyle and
    /// invalidation for layout props, remeasure for content), sets the `explicit` bit for a
    /// runtime's write, and queues `prop_changed` for an observed prop written by Rust (R18).
    pub(crate) fn set_value(&mut self, id: NodeId, prop: u32, v: &Value, origin: Origin) -> Result<(), PropError> {
        let i = self.idx(id).ok_or(PropError::Stale(id.to_raw()))?;
        let kind = self.core[i].kind as usize;
        let tag = KINDS[kind].tag.unwrap_or("?");
        let def = kinds::prop_def(kind, prop).ok_or_else(|| PropError::Unknown(format!("unknown prop id {prop} on <{tag}>")))?;
        let announce = !matches!(origin, Origin::Js) && self.observes(i, prop);
        let before = if announce { props::get_prop(&self.get(id.to_raw()).expect("live"), def).ok() } else { None };
        let d = {
            let w = match &mut self.payload[i] {
                super::node::Payload::Builtin(w) => w,
                super::node::Payload::Vacant => return Err(PropError::Stale(id.to_raw())),
            };
            if props::is_common(def.key) {
                let c = &mut self.cold[i].get_or_insert_with(|| Box::new(super::node::NodeCold::DEFAULT)).common;
                props::set_prop(c, w, def, v)
            } else {
                props::set_prop(&mut Common::default(), w, def, v)
            }
        }
        .map_err(|e| PropError::Invalid(format!("<{tag}> {}: {e}", def.name)))?;
        let t = self.stamp();
        let core = &mut self.core[i];
        core.changed = t;
        core.dirty.insert(Dirty::PAINT);
        if matches!(origin, Origin::Js)
            && let Some(b) = member_bit(kind, prop, Member::Prop)
            && b < 64
        {
            core.explicit |= 1 << b;
        }
        if d.contains(Dirty::LAYOUT) {
            self.restyle(i);
            self.invalidate_style(id);
        }
        if d.contains(Dirty::MEASURE) {
            self.mark_measure(id);
        }
        if announce {
            let after = props::get_prop(&self.get(id.to_raw()).expect("live"), def).ok();
            if after != before {
                self.announce.push((id, prop));
            }
        }
        Ok(())
    }

    /// Rebuilds node `i`'s taffy style from its props.
    pub(crate) fn restyle(&mut self, i: usize) {
        self.layout[i].style = crate::layout::style_of(self.common(i), self.w(i), &self.vis(i));
    }

    /// Sets the TR1 visual channels; a changed clip restyles. False if nothing changed.
    pub(crate) fn set_visual(&mut self, id: NodeId, v: super::Visual) -> bool {
        let i = self.ix(id);
        let before = self.vis(i);
        if v == before {
            return false;
        }
        self.cold_mut(i).vis = v;
        self.core[i].flags.set(NodeFlags::HAS_VISUAL, !v.is_identity());
        self.core[i].dirty.insert(Dirty::PAINT);
        let t = self.stamp();
        self.core[i].changed = t;
        if v.clip != before.clip {
            self.restyle(i);
            self.invalidate_style(id);
        }
        true
    }

    /// R1 observation counts (nested on/off).
    pub(crate) fn observe(&mut self, i: usize, prop: u32, on: bool) {
        let v = &mut self.cold_mut(i).observe;
        match v.iter().position(|&(p, _)| p == prop) {
            Some(k) => {
                if on {
                    v[k].1 += 1;
                } else {
                    v[k].1 = v[k].1.saturating_sub(1);
                    if v[k].1 == 0 {
                        v.swap_remove(k);
                    }
                }
            }
            None if on => v.push((prop, 1)),
            None => {}
        }
    }
}

// ---- the backend's entry points ----

impl EguiBackend {
    /// The scene (ARCH-SCENE §6.1): read, write, defer.
    pub fn scene(&self) -> SceneHandle {
        self.0.scene.clone()
    }

    /// Reads the scene (tests, devtools; pre-SG1). Prefer [`EguiBackend::scene`].
    pub fn with_scene<R>(&self, f: impl FnOnce(&Scene) -> R) -> R {
        f(&self.0.scene.borrow())
    }

    /// A prop of a live node by name, as the host would read it.
    pub fn prop(&self, h: Handle, name: &str) -> Result<Value, String> {
        let scene = self.0.scene.borrow();
        let n = SceneRef::new(&scene).get_raw(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        n.value_of(name).map_err(|e| e.to_string())
    }

    /// The live handles of kind `tag` in creation order of their slots (tests).
    pub fn handles_of(&self, tag: &str) -> Vec<Handle> {
        let scene = self.0.scene.borrow();
        let mut v: Vec<Handle> = SceneRef::new(&scene).of_tag(tag).map(|n| n.id().to_raw()).collect();
        v.sort();
        v
    }
}
