//! Provider passes over the scene (ARCH-SCENE §7.2, §7.3, §7.5; SG4, with R16, R21 and R23):
//! [`StateProvider`] (bulk state from Rust in `logic`, step 6), [`ScenePass`] (overlays and layout
//! hooks per root or per viewport), and [`PrepareCx`] (`Caps::PREPARE`, before a root's layout).
//!
//! ```ignore
//! // a state provider: updates many nodes in one batch, only when it wants to
//! let pick = embed.register_state_provider(PickProvider::default());   // ProviderHandle<PickProvider>
//! pick.with_mut(|p| p.want = Some(ty));          // marks it wanting and asks for a frame (R23)
//! // next logic (step 6): PickProvider::update(&mut SceneMut, &ProviderCx), one batch, one tick;
//! // the roots it changed repaint once
//!
//! // a scene pass: paints over every pane of each window, after berth drew them (R21)
//! let overlay = embed.register_pass(PickOverlay::default());           // PassHandle<PickOverlay>
//! // App::ui (each viewport, root and deferred): … berth draws panes … embed.show_viewport_passes(ui)
//! ```
//!
//! - A state provider runs in `logic` after the checkpoint and animation frames, when its `wants()`
//!   is true or its handle's `with_mut` ran since its last update. Its writes are one batch,
//!   journaled with `Origin::Rust(ProviderId::named(name()))`, and the roots they touch are
//!   repainted. While it wants, `Embed::settled` reports `Busy::PROVIDER` (R16), and logic asks for
//!   another frame. A panic in `update` disables that provider (reported once); the scene's batch
//!   is closed and nothing else is affected.
//! - A scene pass with `PassScope::Root(r)` runs right after root `r`'s walk in `show_slot`
//!   (step e); `Viewport(vp)`/`AllViewports` passes run from `Embed::show_viewport_passes(ui)`,
//!   which the host calls at the end of each viewport's ui (or `embed::show_viewport_passes(ui)`
//!   from a deferred viewport's closure). `overlay` paints in a `Ui` on its own layer
//!   ([`ScenePass::order`]), click-through; `after_layout` sees each layout of the roots in scope.
//!
//! Writer lane: SCENE (SG4).

use crate::scene::access::SceneCell;
use crate::scene::{Damage, NodeId, NodeRef, Origin, ProviderId, RootIx, Scene, SceneHandle, SceneMut, SceneRef};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

// ---- state providers ----

/// Rust code that owns some provider state bits or ext columns and updates them in bulk in `logic`
/// (ARCH-SCENE §7.3, step 6 of §7.2). Register it with `Embed::register_state_provider` or
/// `SceneHandle::register_state_provider`.
pub trait StateProvider: 'static {
    /// Names the provider: its writes are journaled as `Origin::Rust(ProviderId::named(name))`.
    fn name(&self) -> &'static str;
    /// Called once per logic pass when [`StateProvider::wants`] is true or the handle's `with_mut`
    /// ran since the last call (never when idle). All writes are one batch.
    fn update(&mut self, scene: &mut SceneMut<'_>, cx: &ProviderCx<'_>);
    /// Pending work: the next logic calls `update`. While true, the embed is not settled
    /// (`Busy::PROVIDER`, R16) and logic asks for another frame.
    fn wants(&self) -> bool;
    /// Work in flight outside the scene (counts toward `settled`, R8's provider bit).
    fn busy(&self) -> bool {
        false
    }
}

/// What [`StateProvider::update`] gets besides the scene.
pub struct ProviderCx<'a> {
    ctx: Option<&'a egui::Context>,
    kicked: bool,
}

impl ProviderCx<'_> {
    /// The egui context of the logic pass, if the embed has one yet.
    pub fn ctx(&self) -> Option<&egui::Context> {
        self.ctx
    }
    /// True if this call is due to the handle's `with_mut` (not, or not only, to `wants()`).
    pub fn kicked(&self) -> bool {
        self.kicked
    }
}

struct ProviderEntry {
    id: u64,
    p: Rc<RefCell<dyn StateProvider>>,
    kick: Rc<Cell<bool>>,
    faulted: bool,
}

/// A registered [`StateProvider`] (R23). Cloning shares the provider.
pub struct ProviderHandle<P> {
    id: u64,
    p: Rc<RefCell<P>>,
    kick: Rc<Cell<bool>>,
    cell: Weak<SceneCell>,
}

impl<P> Clone for ProviderHandle<P> {
    fn clone(&self) -> Self {
        ProviderHandle { id: self.id, p: self.p.clone(), kick: self.kick.clone(), cell: self.cell.clone() }
    }
}

impl<P: StateProvider> ProviderHandle<P> {
    /// Reads the provider (panics inside its own `update`).
    pub fn with<R>(&self, f: impl FnOnce(&P) -> R) -> R {
        f(&self.p.borrow())
    }
    /// Changes the provider, marks it wanting (its `update` runs at the next logic whatever
    /// `wants()` says) and asks for that logic (R23, R16). Any phase on the UI thread: during the
    /// walk the update still waits for the next logic. Panics inside the provider's own `update`.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut P) -> R) -> R {
        let r = f(&mut self.p.borrow_mut());
        self.kick.set(true);
        if let Some(c) = self.cell.upgrade() {
            SceneHandle::from_cell(c).wake();
        }
        r
    }
    /// Marked wanting by `with_mut` and not updated yet.
    pub fn is_kicked(&self) -> bool {
        self.kick.get()
    }
    /// Removes the provider: it is never called again. The handle still reads it.
    pub fn unregister(&self) {
        if let Some(c) = self.cell.upgrade() {
            c.passes.borrow_mut().providers.retain(|e| e.id != self.id);
        }
    }
}

// ---- scene passes ----

/// Where a [`ScenePass`] runs (ARCH-SCENE §7.5, R21).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PassScope {
    /// After root `r`'s walk, in its `show_slot`.
    Root(RootIx),
    /// From `show_viewport_passes` in that viewport.
    Viewport(egui::ViewportId),
    /// From `show_viewport_passes` in every viewport.
    AllViewports,
}

impl PassScope {
    fn covers_root(self, r: RootIx, vp: egui::ViewportId) -> bool {
        match self {
            PassScope::Root(x) => x == r,
            PassScope::Viewport(v) => v == vp,
            PassScope::AllViewports => true,
        }
    }
}

/// A pass over the scene registered by a provider: overlays painted over a root or a whole window,
/// and a hook after layout (ARCH-SCENE §7.5).
pub trait ScenePass: 'static {
    fn scope(&self) -> PassScope;
    /// The layer `overlay` paints on.
    fn order(&self) -> egui::Order {
        egui::Order::Foreground
    }
    /// Paints over the scope: a root's drawn rect, or the viewport's ui. Read-only scene (last
    /// pass's hit list is complete for this viewport when the viewport's slots are drawn).
    fn overlay(&mut self, _ui: &mut egui::Ui, _scene: SceneRef<'_>, _cx: &mut PassCx<'_>) {}
    /// A root in scope was laid out: `damage.relayout` lists the boundaries (or the root's top
    /// for a whole-root layout). Runs at the root's next draw, before its walk's passes.
    fn after_layout(&mut self, _scene: SceneRef<'_>, _damage: &Damage) {}
}

/// What [`ScenePass::overlay`] gets.
pub struct PassCx<'a> {
    ctx: &'a egui::Context,
    viewport: egui::ViewportId,
    root: Option<RootIx>,
}

impl PassCx<'_> {
    pub fn viewport(&self) -> egui::ViewportId {
        self.viewport
    }
    /// The root, for `PassScope::Root`.
    pub fn root(&self) -> Option<RootIx> {
        self.root
    }
    /// Another pass of this viewport only.
    pub fn request_repaint(&self) {
        self.ctx.request_repaint_of(self.viewport);
    }
}

struct PassEntry {
    id: u64,
    p: Rc<RefCell<dyn ScenePass>>,
}

/// A registered [`ScenePass`]. Cloning shares the pass.
pub struct PassHandle<P> {
    id: u64,
    p: Rc<RefCell<P>>,
    cell: Weak<SceneCell>,
}

impl<P> Clone for PassHandle<P> {
    fn clone(&self) -> Self {
        PassHandle { id: self.id, p: self.p.clone(), cell: self.cell.clone() }
    }
}

impl<P: ScenePass> PassHandle<P> {
    pub fn with<R>(&self, f: impl FnOnce(&P) -> R) -> R {
        f(&self.p.borrow())
    }
    /// Changes the pass (its next run sees it; ask the viewport for a repaint if it must show now).
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut P) -> R) -> R {
        f(&mut self.p.borrow_mut())
    }
    pub fn unregister(&self) {
        if let Some(c) = self.cell.upgrade() {
            c.passes.borrow_mut().passes.retain(|e| e.id != self.id);
        }
    }
}

// ---- the registry (in the scene's cell) ----

#[derive(Default)]
pub(crate) struct Registry {
    next: u64,
    providers: Vec<ProviderEntry>,
    passes: Vec<PassEntry>,
    /// EV1: the pass each viewport last ran its viewport passes in (they run once per pass, from
    /// the windows' walk or the host's `show_viewport_passes`, whichever comes first).
    vp_ran: Vec<(egui::ViewportId, u64)>,
}

impl SceneHandle {
    /// Registers a [`StateProvider`] (R23). It runs at each logic that it wants.
    pub fn register_state_provider<P: StateProvider>(&self, p: P) -> ProviderHandle<P> {
        let p = Rc::new(RefCell::new(p));
        let kick = Rc::new(Cell::new(false));
        let mut r = self.cell().passes.borrow_mut();
        r.next += 1;
        let id = r.next;
        r.providers.push(ProviderEntry { id, p: p.clone(), kick: kick.clone(), faulted: false });
        ProviderHandle { id, p, kick, cell: Rc::downgrade(self.cell()) }
    }

    /// Registers a [`ScenePass`].
    pub fn register_pass<P: ScenePass>(&self, p: P) -> PassHandle<P> {
        let p = Rc::new(RefCell::new(p));
        let mut r = self.cell().passes.borrow_mut();
        r.next += 1;
        let id = r.next;
        r.passes.push(PassEntry { id, p: p.clone() });
        PassHandle { id, p, cell: Rc::downgrade(self.cell()) }
    }

    /// Registered state providers and scene passes.
    pub fn pass_counts(&self) -> (usize, usize) {
        let r = self.cell().passes.borrow();
        (r.providers.len(), r.passes.len())
    }
}

/// Logic step 6: runs every state provider that wants (or was kicked), each as one batch. Returns
/// whether any changed the scene.
pub(crate) fn run_state_providers(h: &SceneHandle, ctx: Option<&egui::Context>) -> bool {
    let list: Vec<(u64, Rc<RefCell<dyn StateProvider>>, Rc<Cell<bool>>)> =
        h.cell().passes.borrow().providers.iter().filter(|e| !e.faulted).map(|e| (e.id, e.p.clone(), e.kick.clone())).collect();
    let mut changed = false;
    for (id, p, kick) in list {
        let Ok(mut pb) = p.try_borrow_mut() else { continue };
        let kicked = kick.get();
        if !kicked && !pb.wants() {
            continue;
        }
        kick.set(false);
        let name = pb.name();
        let before = h.borrow().tick;
        let cx = ProviderCx { ctx, kicked };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.write_now(Origin::Rust(ProviderId::named(name)), name, |m| pb.update(m, &cx))));
        let mut s = h.borrow_mut();
        s.counters.provider_updates += 1;
        match r {
            Ok(Ok(())) => {}
            // the scene is borrowed (cannot happen in logic): try again next time
            Ok(Err(_)) => kick.set(true),
            Err(p) => {
                s.abort_batch();
                drop(s);
                let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into());
                eprintln!("[svelters-egui] state provider `{name}` panicked in update and is disabled: {msg}");
                for e in h.cell().passes.borrow_mut().providers.iter_mut().filter(|e| e.id == id) {
                    e.faulted = true;
                }
                changed = true;
                continue;
            }
        }
        changed |= s.tick != before;
    }
    changed
}

/// R16: a provider with pending work (wanting, kicked or busy).
pub(crate) fn providers_busy(h: &SceneHandle) -> bool {
    h.cell().passes.borrow().providers.iter().any(|e| !e.faulted && (e.kick.get() || e.p.try_borrow().map(|p| p.wants() || p.busy()).unwrap_or(true)))
}

/// R16: a state provider still wants after its update (logic asks for another frame).
pub(crate) fn providers_wanting(h: &SceneHandle) -> bool {
    h.cell().passes.borrow().providers.iter().any(|e| !e.faulted && (e.kick.get() || e.p.try_borrow().map(|p| p.wants()).unwrap_or(false)))
}

/// R8: a provider widget reports work in flight (`Widget::busy`).
pub(crate) fn widgets_busy(s: &Scene) -> bool {
    let reg = crate::kinds::KindRegistry::global();
    reg.indexes().filter(|&k| reg.provider(k).is_some()).any(|k| {
        s.index.of_kind(k).iter().any(|&id| match s.idx(id).map(|i| &s.payload[i]) {
            Some(crate::scene::node::Payload::Builtin(crate::scene::W::Ext(x))) => x.0.try_borrow().is_ok_and(|w| w.busy()),
            _ => false,
        })
    })
}

fn passes_for(h: &SceneHandle, f: impl Fn(PassScope) -> bool) -> Vec<(Rc<RefCell<dyn ScenePass>>, egui::Order)> {
    h.cell()
        .passes
        .borrow()
        .passes
        .iter()
        .filter_map(|e| {
            let p = e.p.try_borrow().ok()?;
            f(p.scope()).then(|| (e.p.clone(), p.order()))
        })
        .collect()
}

/// Runs `pass.overlay` in a child of `ui` over `rect`, on the pass's own layer.
fn overlay_in(h: &SceneHandle, ui: &mut egui::Ui, rect: egui::Rect, k: usize, p: &RefCell<dyn ScenePass>, order: egui::Order, root: Option<RootIx>) {
    let vp = ui.ctx().viewport_id();
    let layer = egui::LayerId::new(order, egui::Id::unique(("svelters-scene-pass", k, root.map(|r| r.0), vp)));
    let mut child = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(rect));
    let ctx = ui.ctx().clone();
    let mut cx = PassCx { ctx: &ctx, viewport: vp, root };
    let Ok(mut pb) = p.try_borrow_mut() else { return };
    let Ok(s) = h.try_borrow() else { return };
    pb.overlay(&mut child, SceneRef::new(&s), &mut cx);
}

/// Step e of §7.2: the `Root(r)` passes after root `r`'s walk (and `after_layout` of every pass whose
/// scope covers `r`, if `r` was laid out), in `rect` (the slot's drawn rect).
pub(crate) fn run_root_passes(h: &SceneHandle, ui: &mut egui::Ui, r: RootIx, rect: egui::Rect) {
    let vp = ui.ctx().viewport_id();
    let laid = h.try_borrow_mut().ok().and_then(|mut s| s.roots.get_mut(r).map(|e| std::mem::take(&mut e.laid))).unwrap_or_default();
    if !laid.is_empty() {
        let damage = Damage { relayout: laid, ..Damage::default() };
        for (p, _) in passes_for(h, |sc| sc.covers_root(r, vp)) {
            if let (Ok(mut pb), Ok(s)) = (p.try_borrow_mut(), h.try_borrow()) {
                pb.after_layout(SceneRef::new(&s), &damage);
            }
        }
    }
    for (k, (p, order)) in passes_for(h, |sc| sc == PassScope::Root(r)).into_iter().enumerate() {
        overlay_in(h, ui, rect, k, &p, order, Some(r));
    }
}

/// R21: the `Viewport(vp)` and `AllViewports` passes over `ui`'s max rect.
pub(crate) fn run_viewport_passes(h: &SceneHandle, ui: &mut egui::Ui) -> usize {
    let vp = ui.ctx().viewport_id();
    {
        let pass = ui.ctx().cumulative_pass_nr();
        let mut r = h.cell().passes.borrow_mut();
        match r.vp_ran.iter_mut().find(|(v, _)| *v == vp) {
            Some((_, p)) if *p == pass => return 0,
            Some((_, p)) => *p = pass,
            None => r.vp_ran.push((vp, pass)),
        }
    }
    let list = passes_for(h, |sc| matches!(sc, PassScope::AllViewports) || sc == PassScope::Viewport(vp));
    let rect = ui.max_rect();
    for (k, (p, order)) in list.iter().enumerate() {
        overlay_in(h, ui, rect, k, p, *order, None);
    }
    list.len()
}

// ---- prepare (Caps::PREPARE) ----

/// What [`Widget::prepare`](crate::provider::Widget::prepare) gets (ARCH-SCENE §8.2): the host node,
/// last frame's rects and the scene to build or rebind its shadow children in. Writes are journaled
/// as `Origin::Rust(ProviderId::named(<kind id>))`; nodes it creates are owned by that provider.
/// The phase is `Prepare`: by convention it changes only its own shadow subtree (not enforced).
pub struct PrepareCx<'a> {
    s: &'a mut Scene,
    node: NodeId,
    origin: Origin,
}

impl PrepareCx<'_> {
    pub fn host(&self) -> NodeId {
        self.node
    }
    /// The host (its payload is out while `prepare` runs: `widget()` is `None`).
    pub fn node(&self) -> NodeRef<'_> {
        NodeRef::at(self.s, self.node)
    }
    /// Last frame's drawn rect of the host on screen, intersected with nothing (`None` before its
    /// first draw).
    pub fn visible_rect(&self) -> Option<egui::Rect> {
        self.node().drawn()
    }
    /// The host's root-relative rect from the last layout.
    pub fn rect(&self) -> egui::Rect {
        self.node().rect()
    }
    pub fn read(&self) -> SceneRef<'_> {
        SceneRef::new(self.s)
    }
    /// Mutation access (create nodes, `insert_shadow` under the host, props, state, ext).
    pub fn scene(&mut self) -> SceneMut<'_> {
        SceneMut { s: self.s, origin: self.origin }
    }
}

/// Step b of §7.2: every visible `Caps::PREPARE` provider node of root `r`, before its layout.
pub(crate) fn prepare_root(s: &mut Scene, r: RootIx) {
    let reg = crate::kinds::KindRegistry::global();
    let kinds: Vec<(usize, &'static str)> = reg
        .indexes()
        .filter_map(|k| reg.provider(k).filter(|p| p.entry.caps.contains(crate::provider::Caps::PREPARE)).map(|p| (k, p.entry.id)))
        .collect();
    for (k, kid) in kinds {
        let nodes: Vec<NodeId> = s.index.of_kind(k).iter().copied().filter(|&id| s.idx(id).is_some_and(|i| s.core[i].root == r)).collect();
        for node in nodes {
            let Some(i) = s.idx(node) else { continue };
            if !NodeRef::at(s, node).is_visible() {
                continue;
            }
            let origin = Origin::Rust(ProviderId::named(kid));
            crate::provider::with_taken(s, i, |w, s| {
                let outer = s.begin_batch("prepare");
                w.prepare(&mut PrepareCx { s: &mut *s, node, origin });
                s.end_batch(outer);
            });
        }
    }
}
