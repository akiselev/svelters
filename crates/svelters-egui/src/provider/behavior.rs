//! Native behaviours behind `use:` (ARCH-EMBED §8.5, ARCH-SCENE §4.4 and §7.3; EV2b, SV2).
//!
//! A behaviour is an **attached kind**: a Rust struct with `#[prop]` params and `#[event]` events,
//! like a widget, but implementing [`Behavior`] instead of `Widget`. The scene keeps its node in its
//! host's attached list (`NodeCold::attached`, `NodeFlags::ATTACHED`), never among the light
//! children: it is not laid out or painted. The walk runs [`Behavior::after_ui`] right after the host
//! is drawn, every frame, with the host's response, in Rust: no JS entry per frame.
//!
//! ```ignore
//! /// Makes its host pickable for a running pick.
//! #[svelters::behavior(namespace = "cad", name = "pick")]
//! pub struct Pick {
//!     #[prop] accepts: Vec<String>,
//!     #[event] picked: Event<PickRef>,
//! }
//! impl Behavior for Pick {
//!     fn sense(&self) -> egui::Sense { egui::Sense::click() }
//!     fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
//!         cx.set_host_state(&cad::PICK_HOVER, resp.hovered());
//!         if resp.clicked() { cx.emit(&self.picked, PickRef { .. }); }
//!     }
//! }
//! ```
//!
//! ```svelte
//! <script>import { pick } from 'svelters:behaviors/cad';</script>
//! <label use:pick={{ accepts: ['mcad.face'], onpicked: (r) => … }}>Face</label>
//! ```
//!
//! The dump writes `svelters:behaviors/<namespace>` (`behaviors/<ns>.js`, one
//! `nativeBehavior(..)` action per behaviour) and its typings (`behaviors.d.ts`); see `spec.rs`. The
//! action creates the attached node, sets the params as props (only the ones that changed on an
//! update: one `set_prop` per changed param) and listens to the `on<event>` callbacks of the params.
//!
//! Hooks: `sense`, `before_ui` and `paint_over` (SG4), `after_ui`, `detached`. ARCH-SCENE §7.3's
//! `filter` is EV1's.
//!
//! Writer lane: EVENTS (EV2b).

use super::{
    ErasedWidget, Event, IntoStateBits, KindDecl, KindEntry, StateBits, flush_writes, queue_emit,
};
use crate::kinds::KindRegistry;
use crate::scene::node::Payload;
use crate::scene::{
    Change, NodeFlags, NodeId, NodeMut, NodeRef, Origin, RootIx, Scene, SceneError, SceneMut,
    SceneRef, W,
};
use crate::walk::Out;
use std::any::Any;
use std::marker::PhantomData;
use svelters_host::exports::{CallCx, IntoJs, NativeError, Value};

/// What a behaviour does to its host (ARCH-SCENE §7.3). Implemented by the struct
/// `#[svelters::behavior]` declares.
pub trait Behavior: 'static {
    /// What the host must sense for this behaviour, merged into the host's interaction: a behaviour
    /// that wants clicks returns `Sense::click()` and a label host becomes clickable. Read every frame
    /// before the hook (a change applies from the next frame on, as egui hit-tests last frame's rects).
    fn sense(&self) -> egui::Sense {
        egui::Sense::hover()
    }

    /// Runs after the host's `ui` (its widget, or for a container its background response under
    /// the children), every frame the host is drawn, on the UI thread. Never runs JS: writes to the
    /// host (state bits, ext data) and the behaviour's own events go through `cx`
    /// and are applied after the hook.
    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>);

    /// SG4 (ARCH-SCENE §7.3): runs before the host's `ui`, every frame the host is drawn, with the
    /// host's layout rect (`cx.rect()`, screen) and its egui id. State bits set here apply before
    /// the host draws (a tutorial gate, a disabled look); other writes are applied after the hook.
    fn before_ui(&mut self, _host: NodeRef<'_>, _cx: &mut BehaviorCx<'_>) {}

    /// SG4 (ARCH-SCENE §7.3): paints over the host (decorations, badges, pick highlights) after its
    /// `ui` (for a container, after its children) and the `after_ui` hooks, on the host's layer.
    fn paint_over(&mut self, _host: NodeRef<'_>, _painter: &egui::Painter, _cx: &mut BehaviorCx<'_>) {}

    /// The behaviour leaves its host (the `use:` action's destroy, or a move): undo what it set on
    /// the host (its state bits, its ext entries). Writes are journaled as `Origin::Widget`.
    fn detached(&mut self, _host: &mut NodeMut<'_>) {}
}

/// What [`Behavior::after_ui`] gets: the host's id, rect and state, and the queued writes. (A
/// behaviour reports values with events; its params have no `bind:`.)
pub struct BehaviorCx<'a> {
    node: NodeId,
    host: NodeId,
    id: egui::Id,
    rect: egui::Rect,
    state: StateBits,
    entry: &'static KindEntry,
    emits: Vec<(usize, Value)>,
    writes: Vec<HostWrite>,
    errors: Vec<String>,
    repaint: bool,
    _a: PhantomData<&'a mut ()>,
}

type HostWrite = Box<dyn FnOnce(&mut NodeMut<'_>)>;

impl BehaviorCx<'_> {
    /// The behaviour's own node.
    pub fn node(&self) -> NodeId {
        self.node
    }
    /// The host node.
    pub fn host(&self) -> NodeId {
        self.host
    }
    /// The host's egui id (its response's id).
    pub fn id(&self) -> egui::Id {
        self.id
    }
    /// The rect the host took this frame (screen coordinates).
    pub fn rect(&self) -> egui::Rect {
        self.rect
    }
    /// Whether every bit of `bits` is set on the host (with this hook's writes).
    pub fn host_state(&self, bits: impl IntoStateBits) -> bool {
        self.state.contains(bits.state_bits())
    }
    /// Sets or clears host state bits (provider bits such as `cad::PICK_TARGET`, built-ins). Applied
    /// after the hook and journaled only when the host's state actually changed.
    pub fn set_host_state(&mut self, bits: impl IntoStateBits, on: bool) {
        self.state.set(bits.state_bits(), on);
    }
    /// Any other host write (ext data, flags, props through the exact path), applied after the hook
    /// with `Origin::Widget`: `cx.with_host(|n| n.ext_mut(&PRESENTS).set(r))`.
    pub fn with_host(&mut self, f: impl FnOnce(&mut NodeMut<'_>) + 'static) {
        self.writes.push(Box::new(f));
    }
    /// Queues `payload` for the event's JS listener (the `on<event>` callback of the params),
    /// delivered after the pass.
    pub fn emit<T: IntoJs>(&mut self, event: &Event<T>, payload: T) {
        queue_emit(
            self.entry,
            event,
            payload,
            &mut self.emits,
            &mut self.errors,
        );
    }
    /// Asks for another frame (an animation the behaviour drives).
    pub fn request_repaint(&mut self) {
        self.repaint = true;
    }
}

/// The payload of a behaviour node: the behaviour struct behind the scene's provider payload
/// (`W::Ext`), so props, `NodeRef::widget::<B>` and the registry treat it as any provider kind.
pub struct BehaviorBox<B>(pub B);

impl<B: Behavior + KindDecl> ErasedWidget for BehaviorBox<B> {
    fn entry(&self) -> &'static KindEntry {
        B::KIND
    }
    fn set_prop(&mut self, prop: usize, v: Value, cx: &CallCx<'_>) -> Result<(), NativeError> {
        self.0.__svelters_set(prop, v, cx)
    }
    fn reset_prop(&mut self, prop: usize) {
        self.0.__svelters_reset(prop)
    }
    fn get_prop(&self, prop: usize, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        self.0.__svelters_get(prop, cx)
    }
    fn measure(
        &mut self,
        _cx: &mut super::MeasureCx<'_>,
        _avail: super::AvailableSpace,
    ) -> egui::Vec2 {
        egui::Vec2::ZERO
    }
    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut super::WidgetCx<'_>) -> egui::Response {
        // never called: attached nodes are not among the painted children
        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
    }
    fn a11y(&self, _cx: &super::A11yCx<'_>, _node: &mut egui::accesskit::Node) {}
    fn busy(&self) -> bool {
        false
    }
    fn hit_test(&self, _cx: &super::HitCx, _local: egui::Pos2) -> super::Hit {
        // attached nodes are not painted, so they never own a point: it goes to the host
        super::Hit::Miss
    }
    // never called: attached nodes are not laid out
    fn layout(&mut self, cx: &mut super::LayoutCx<'_>, inputs: crate::taffy::LayoutInput) -> crate::taffy::LayoutOutput {
        cx.default_layout(inputs)
    }
    fn prepare(&mut self, _cx: &mut super::PrepareCx<'_>) {}
    fn as_any(&self) -> &dyn Any {
        &self.0
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.0
    }
    fn base(&self) -> usize {
        &self.0 as *const B as usize
    }
    fn behavior(&mut self) -> Option<&mut dyn Behavior> {
        Some(&mut self.0)
    }
    fn behavior_ref(&self) -> Option<&dyn Behavior> {
        Some(&self.0)
    }
}

/// A fresh behaviour payload (what `#[svelters::behavior]`'s `KindEntry::new` returns).
#[doc(hidden)]
pub fn __new_behavior<B: Behavior + KindDecl>() -> Box<dyn ErasedWidget> {
    Box::new(BehaviorBox(B::__svelters_new()))
}

// ---- the walk's hook ----

/// The union of the senses of the behaviours attached to slot `i`.
pub(crate) fn sense(arena: &Scene, i: usize) -> egui::Sense {
    let mut s = egui::Sense::hover();
    for &b in &arena.cold(i).attached {
        let Some(bi) = arena.idx(b) else { continue };
        if let Payload::Builtin(W::Ext(x)) = &arena.payload[bi]
            && let Ok(w) = x.0.try_borrow()
            && let Some(bh) = w.behavior_ref()
        {
            s |= bh.sense();
        }
    }
    s
}

/// Runs around the host in slot `i` (a host with `NodeFlags::HAS_ATTACHED`): `paint` draws the host
/// and returns its response; a container (`container`) instead gets a response of its own at `ui`'s
/// max rect (its rect), registered before its children so they stay on top. Then each behaviour's
/// `after_ui` with that response; a leaf's response first takes the behaviours' senses.
pub(crate) fn around(
    arena: &mut Scene,
    i: usize,
    ui: &mut egui::Ui,
    out: &mut Vec<Out>,
    paint: impl FnOnce(&mut Scene, &mut Vec<Out>) -> Option<egui::Response>,
    container: bool,
) {
    let want = sense(arena, i);
    // SG4: before_ui, with the host's rect and id
    let (id, rect) = (arena.core[i].egui_id, ui.max_rect());
    hooks(arena, i, id, rect, ui.ctx(), out, &mut |bh, host, cx| bh.before_ui(host, cx));
    let bg = container.then(|| ui.interact(ui.max_rect(), arena.core[i].egui_id, want));
    let own = paint(arena, out);
    let resp = match (bg, own) {
        (Some(b), _) => b,
        (None, Some(r)) if !r.sense.contains(want) => r.interact(want),
        (None, Some(r)) => r,
        (None, None) => return,
    };
    hooks(arena, i, resp.id, resp.rect, &resp.ctx, out, &mut |bh, host, cx| bh.after_ui(host, &resp, cx));
    // SG4: paint_over, on top of the host (and a container's children)
    let painter = ui.painter().clone();
    hooks(arena, i, resp.id, resp.rect, &resp.ctx, out, &mut |bh, host, cx| bh.paint_over(host, &painter, cx));
}

/// Runs one hook (`call`) of each behaviour attached to slot `i`, with the host's id and rect, and
/// applies what each one wrote (host state, host writes, events) after it.
fn hooks(
    arena: &mut Scene,
    i: usize,
    id: egui::Id,
    rect: egui::Rect,
    ctx: &egui::Context,
    out: &mut Vec<Out>,
    call: &mut dyn FnMut(&mut dyn Behavior, NodeRef<'_>, &mut BehaviorCx<'_>),
) {
    let host = arena.slots.id_at(i).expect("live host");
    let mut k = 0;
    // the attached list cannot change during the hooks (host writes are not structural)
    while k < arena.cold(i).attached.len() {
        let b = arena.cold(i).attached[k];
        k += 1;
        let Some(bi) = arena.idx(b) else { continue };
        let ran = {
            let scene: &Scene = arena;
            let Payload::Builtin(W::Ext(x)) = &scene.payload[bi] else {
                continue;
            };
            let Ok(mut wb) = x.0.try_borrow_mut() else {
                continue;
            };
            let entry = wb.entry();
            let Some(bh) = wb.behavior() else { continue };
            let mut cx = BehaviorCx {
                node: b,
                host,
                id,
                rect,
                state: scene.core[i].state,
                entry,
                emits: Vec::new(),
                writes: Vec::new(),
                errors: Vec::new(),
                repaint: false,
                _a: PhantomData,
            };
            let host_ref = SceneRef::new(scene).get(host).expect("live host");
            call(bh, host_ref, &mut cx);
            cx
        };
        let BehaviorCx {
            state,
            emits,
            writes,
            errors,
            repaint,
            ..
        } = ran;
        if state != arena.core[i].state {
            let before = arena.core[i].state;
            arena.core[i].state = state;
            let t = arena.stamp();
            arena.core[i].changed = t;
            arena.core[i].dirty.insert(crate::scene::Dirty::PAINT);
            arena.record_state(i, before, Origin::Widget);
        }
        if !writes.is_empty() {
            let mut sm = SceneMut {
                s: arena,
                origin: Origin::Widget,
            };
            if let Some(mut n) = sm.node(host) {
                for w in writes {
                    w(&mut n);
                }
            }
        }
        flush_writes(arena, bi, Vec::new(), emits, errors, out);
        if repaint {
            ctx.request_repaint();
        }
    }
}

// ---- the scene's attached lists (called from `scene/tree.rs`) ----

impl Scene {
    /// Attaches the behaviour node `child` to `host` (no slot or position). `Ok(false)`: already
    /// attached there.
    pub(crate) fn insert_attached(
        &mut self,
        host: NodeId,
        child: NodeId,
        origin: Origin,
    ) -> Result<bool, SceneError> {
        let (hi, ci) = (self.ix(host), self.ix(child));
        let reg = KindRegistry::global();
        let (hk, ck) = (self.core[hi].kind as usize, self.core[ci].kind as usize);
        let to = reg
            .provider(ck)
            .and_then(|p| p.entry.attaches_to)
            .unwrap_or("Widget");
        if !reg.chain_iter(hk).any(|k| reg.kind(k).id == to) {
            return Err(SceneError::Invalid(format!(
                "<{}> attaches to a {to}, not to <{}>",
                self.tag_at(ci),
                self.tag_at(hi)
            )));
        }
        if host == child {
            return Err(SceneError::Invalid(
                "cannot attach a behaviour to itself".into(),
            ));
        }
        if self.core[ci].parent == Some(host) {
            return Ok(false);
        }
        self.may_edit(origin, &[host, child])?;
        if self.core[ci].parent.is_some() {
            self.unlink_attached(child, false, origin);
        }
        self.cold_mut(hi).attached.push(child);
        self.core[hi].flags.insert(NodeFlags::HAS_ATTACHED);
        self.core[ci].parent = Some(host);
        self.core[ci].flags.insert(NodeFlags::ATTACHED);
        let t = self.stamp();
        self.core[hi].changed = t;
        self.core[ci].changed = t;
        let r = self.core[hi].root;
        if self.core[ci].root != r {
            self.set_root_subtree(child, r);
        }
        self.record(
            Change::Inserted {
                parent: host,
                node: child,
                before: None,
            },
            origin,
        );
        Ok(true)
    }

    /// Takes the attached node `child` off its host, after its [`Behavior::detached`] hook.
    pub(crate) fn unlink_attached(&mut self, child: NodeId, detach_root: bool, origin: Origin) {
        let ci = self.ix(child);
        let Some(host) = self.core[ci].parent else {
            return;
        };
        self.run_detached(ci, host);
        let hi = self.ix(host);
        let cold = self.cold_mut(hi);
        cold.attached.retain(|&c| c != child);
        if cold.attached.is_empty() {
            self.core[hi].flags.remove(NodeFlags::HAS_ATTACHED);
        }
        self.core[ci].parent = None;
        self.core[ci].flags.remove(NodeFlags::ATTACHED);
        let t = self.stamp();
        self.core[hi].changed = t;
        self.core[ci].changed = t;
        self.record(
            Change::Removed {
                parent: host,
                node: child,
            },
            origin,
        );
        if detach_root && !self.core[ci].root.is_detached() {
            self.set_root_subtree(child, RootIx::DETACHED);
        }
    }

    /// The host in slot `i` is being destroyed: its attached nodes stay alive, detached (a runtime
    /// destroys them itself), without their `detached` hooks (the host is gone).
    pub(crate) fn orphan_attached(&mut self, i: usize, origin: Origin) {
        if !self.core[i].flags.contains(NodeFlags::HAS_ATTACHED) {
            return;
        }
        let host = self.slots.id_at(i).expect("live host");
        let list = std::mem::take(&mut self.cold_mut(i).attached);
        self.core[i].flags.remove(NodeFlags::HAS_ATTACHED);
        for c in list {
            self.record(
                Change::Removed {
                    parent: host,
                    node: c,
                },
                origin,
            );
            let ci = self.ix(c);
            self.core[ci].parent = None;
            self.core[ci].flags.remove(NodeFlags::ATTACHED);
            if !self.core[ci].root.is_detached() {
                self.set_root_subtree(c, RootIx::DETACHED);
            }
        }
    }

    /// Runs the `detached` hook of the behaviour in slot `ci` against its live host.
    fn run_detached(&mut self, ci: usize, host: NodeId) {
        // the payload leaves its slot while the hook has the scene
        let payload = std::mem::replace(&mut self.payload[ci], Payload::Vacant);
        if let Payload::Builtin(W::Ext(x)) = &payload
            && let Some(bh) = x.0.borrow_mut().behavior()
        {
            let mut sm = SceneMut {
                s: self,
                origin: Origin::Widget,
            };
            if let Some(mut n) = sm.node(host) {
                bh.detached(&mut n);
            }
        }
        self.payload[ci] = payload;
    }

    /// The nodes attached to slot `i` (behaviours), in attach order.
    pub(crate) fn attached_of(&self, i: usize) -> &[NodeId] {
        &self.cold(i).attached
    }
}

impl<'a> NodeRef<'a> {
    /// The nodes attached to this one (its behaviours, EV2b), in attach order. Not among
    /// [`NodeRef::children`].
    pub fn attached(self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let s = self.s;
        s.attached_of(self.i)
            .iter()
            .filter_map(move |&c| SceneRef::new(s).get(c))
    }
}
