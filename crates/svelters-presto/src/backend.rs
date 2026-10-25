//! [`PrestoBackend`]: the backend ABI on presto's tree (`plans/architecture/backend-abi.md`; the egui
//! backend's `ops.rs` is the template). Every op is a call on the one [`Document`] with typed, by-id
//! props; handles are this backend's own, allocated in order so a template's nodes are `base + k`
//! (compiler-and-js §3.2).
//!
//! The toolkit loop is not here: whoever runs presto (the `presto` facade's app, a test harness)
//! installs a [`Driver`], and [`Backend::run`] hands it the loop. JS-facing events come back through
//! [`PrestoBackend::deliver`] (`deliver.rs`).

use crate::ids::What;
use crate::shim::{Document, KindId, KindRegistry, KindSpec, ListenFlags};
use crate::value;
use crate::wire::Wire;
use presto_base::ids::NodeId;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use svelters_abi::*;

/// The ABI's `listen_ex` flag for a passive listener (compiler-and-js §3.5; lane COMPILER emits it).
pub const SVL_LISTEN_PASSIVE: u32 = 4;

/// Who runs the loop: the presto app runner or a test harness. Main thread only.
pub trait Driver {
    /// Runs the loop until the app quits ([`Backend::run`]).
    fn run(&self, backend: &PrestoBackend) -> StrResult<()>;
    /// The app asked to quit.
    fn quit(&self);
    /// The one-shot host timer moved (`None`: cancelled). The driver calls
    /// [`PrestoBackend::fire_timer`] at the deadline.
    fn timer_changed(&self, deadline: Option<Instant>);
    /// JS asked for a frame (`requestAnimationFrame`); the driver calls [`PrestoBackend::frame`] at
    /// the next one.
    fn frame_requested(&self);
}

/// Cross-thread wake: a flag plus the platform's waker, both `Send`.
#[derive(Default)]
pub struct WakeState {
    pending: AtomicBool,
    notify: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl WakeState {
    /// Takes the pending flag (the driver runs a host checkpoint when it was set).
    pub fn take(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// Installs the platform's waker (presto-platform's `Waker::wake`).
    pub fn set_notify(&self, f: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self.notify.lock().unwrap_or_else(|e| e.into_inner()) = f;
    }

    fn wake(&self) {
        self.pending.store(true, Ordering::Release);
        if let Some(f) = self.notify.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f();
        }
    }
}

/// Handle ↔ node. Handles count up from 1 and are never reused, so a template's nodes are
/// contiguous and a stale handle never names a new node.
#[derive(Default)]
pub(crate) struct Handles {
    next: svl_handle,
    nodes: HashMap<svl_handle, NodeId>,
    handles: HashMap<NodeId, svl_handle>,
}

impl Handles {
    /// Reserves `n` contiguous handles; returns the first.
    pub(crate) fn reserve(&mut self, n: u64) -> svl_handle {
        let base = self.next.max(1);
        self.next = base + n;
        base
    }

    pub(crate) fn bind(&mut self, h: svl_handle, node: NodeId) {
        self.nodes.insert(h, node);
        self.handles.insert(node, h);
    }

    pub(crate) fn node(&self, h: svl_handle) -> Option<NodeId> {
        self.nodes.get(&h).copied()
    }

    pub(crate) fn handle(&self, n: NodeId) -> Option<svl_handle> {
        self.handles.get(&n).copied()
    }

    pub(crate) fn unbind(&mut self, h: svl_handle) -> Option<NodeId> {
        let n = self.nodes.remove(&h)?;
        self.handles.remove(&n);
        Some(n)
    }
}

pub struct Inner {
    pub(crate) doc: RefCell<Document>,
    pub(crate) registry: Arc<KindRegistry>,
    pub(crate) wire: Wire,
    pub(crate) handles: RefCell<Handles>,
    pub(crate) host: Cell<*const svl_host>,
    /// Presented windows, in order.
    pub(crate) windows: RefCell<Vec<svl_handle>>,
    /// `close` calls the driver has not applied yet.
    pub(crate) closes: RefCell<Vec<svl_handle>>,
    /// Nodes the embedder owns (slots): JS mounts into them but cannot destroy them.
    pub(crate) borrowed: RefCell<HashSet<svl_handle>>,
    /// Observe counts per (handle, wire prop id).
    pub(crate) observed: RefCell<HashMap<(svl_handle, svl_id), u32>>,
    pub(crate) wake: Arc<WakeState>,
    pub(crate) deadline: Cell<Option<Instant>>,
    pub(crate) frame_pending: Cell<bool>,
    pub(crate) quitting: Cell<bool>,
    pub(crate) driver: RefCell<Option<Rc<dyn Driver>>>,
}

/// The svelters backend on presto. Cheap to clone (shared state).
#[derive(Clone)]
pub struct PrestoBackend(pub(crate) Rc<Inner>);

impl PrestoBackend {
    /// A backend over a fresh [`Document`] of `registry`'s kinds, with wire ids from the checked-in
    /// table.
    pub fn new(registry: Arc<KindRegistry>) -> PrestoBackend {
        let wire = Wire::new(&registry, crate::ids::Ids::builtin());
        PrestoBackend(Rc::new(Inner {
            doc: RefCell::new(Document::new(registry.clone())),
            registry,
            wire,
            handles: RefCell::new(Handles::default()),
            host: Cell::new(std::ptr::null()),
            windows: RefCell::new(Vec::new()),
            closes: RefCell::new(Vec::new()),
            borrowed: RefCell::new(HashSet::new()),
            observed: RefCell::new(HashMap::new()),
            wake: Arc::new(WakeState::default()),
            deadline: Cell::new(None),
            frame_pending: Cell::new(false),
            quitting: Cell::new(false),
            driver: RefCell::new(None),
        }))
    }

    /// A backend of the built-in kinds.
    pub fn builtin() -> PrestoBackend {
        PrestoBackend::new(Arc::new(crate::builtin_registry()))
    }

    /// Installs who runs the loop ([`Driver`]).
    pub fn set_driver(&self, d: Option<Rc<dyn Driver>>) {
        *self.0.driver.borrow_mut() = d;
    }

    fn driver(&self) -> Option<Rc<dyn Driver>> {
        self.0.driver.borrow().clone()
    }

    pub fn wake_state(&self) -> &Arc<WakeState> {
        &self.0.wake
    }

    /// The document, for the runner (style, layout, frame) and tests.
    pub fn with_doc<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
        f(&mut self.0.doc.borrow_mut())
    }

    pub fn registry(&self) -> &Arc<KindRegistry> {
        &self.0.registry
    }

    pub fn wire(&self) -> &Wire {
        &self.0.wire
    }

    /// The node behind a live handle.
    pub fn node(&self, h: svl_handle) -> Option<NodeId> {
        self.0.handles.borrow().node(h)
    }

    /// The handle of a node JS knows; `None` for nodes JS never saw.
    pub fn handle(&self, n: NodeId) -> Option<svl_handle> {
        self.0.handles.borrow().handle(n)
    }

    /// The attached host, while one is.
    pub fn host(&self) -> Option<HostRef<'_>> {
        let p = self.0.host.get();
        // SAFETY: `run`/`start` store the host's callback table, which outlives the attachment;
        // `start(None)` clears it before the host goes.
        (!p.is_null()).then(|| unsafe { HostRef::from_raw(p) })
    }

    /// Presented windows, in order.
    pub fn windows(&self) -> Vec<svl_handle> {
        self.0.windows.borrow().clone()
    }

    /// `close` calls since the last take: the driver closes those windows and reports them with
    /// [`PrestoBackend::window_closed`].
    pub fn take_closes(&self) -> Vec<svl_handle> {
        std::mem::take(&mut self.0.closes.borrow_mut())
    }

    /// The driver closed window `h` (by `close` or by the user after `close-request`).
    pub fn window_closed(&self, h: svl_handle) {
        self.0.windows.borrow_mut().retain(|&w| w != h);
        if let Some(host) = self.host() {
            host.on_destroyed(h, SVL_GONE_CLOSED);
        }
    }

    pub fn quitting(&self) -> bool {
        self.0.quitting.get()
    }

    /// The host timer's deadline, for drivers that poll.
    pub fn deadline(&self) -> Option<Instant> {
        self.0.deadline.get()
    }

    /// The driver's timer reached the deadline: runs the host checkpoint it was for.
    pub fn fire_timer(&self) {
        self.0.deadline.set(None);
        if let Some(h) = self.host() {
            h.checkpoint();
        }
    }

    /// The driver's frame: runs `requestAnimationFrame` callbacks when one was asked for.
    pub fn frame(&self) {
        if self.0.frame_pending.replace(false)
            && let Some(h) = self.host()
        {
            h.frame();
        }
    }

    /// The driver saw a cross-thread wake: runs the checkpoint it asked for.
    pub fn poll_wake(&self) {
        if self.0.wake.take()
            && let Some(h) = self.host()
        {
            h.checkpoint();
        }
    }

    /// Makes `kind` a node the embedder owns (an embed slot, `embed`): a handle JS can mount into.
    pub fn create_borrowed(&self, kind: &str) -> StrResult<svl_handle> {
        let h = self.create(self.resolve(NameKind::Kind, 0, kind))?;
        self.0.borrowed.borrow_mut().insert(h);
        Ok(h)
    }

    /// Frees an embedder-owned node.
    pub fn destroy_borrowed(&self, h: svl_handle) {
        self.0.borrowed.borrow_mut().remove(&h);
        self.destroy(h);
    }

    pub(crate) fn live(&self, h: svl_handle) -> StrResult<(NodeId, KindId, &'static KindSpec)> {
        let n = self.node(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let doc = self.0.doc.borrow();
        let k = doc.kind(n).ok_or_else(|| format!("stale handle {h}"))?;
        let spec = self.0.registry.spec(k).ok_or_else(|| format!("handle {h} has an unregistered kind"))?;
        Ok((n, k, spec))
    }

    fn tag(spec: &KindSpec) -> &'static str {
        spec.tag.unwrap_or(spec.name)
    }

    /// Creates a node of wire kind `kind` at handle `h` (reserved by the caller).
    pub(crate) fn create_at(&self, h: svl_handle, kind: svl_id) -> StrResult<NodeId> {
        let k = self.0.wire.kind(kind).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let spec = self.0.registry.spec(k).ok_or_else(|| format!("unknown kind id {kind}"))?;
        if spec.tag.is_none() {
            return Err(format!("{} is abstract: no element creates it", spec.name));
        }
        let n = self.0.doc.borrow_mut().create(k);
        self.0.handles.borrow_mut().bind(h, n);
        Ok(n)
    }

    /// The prop write behind `set_prop`, shared with templates.
    pub(crate) fn write_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        let (n, k, spec) = self.live(h)?;
        let (pid, ps) = self.0.wire.core_prop(&self.0.registry, k, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", Self::tag(spec)))?;
        let v = value::from_abi(v, ps.ty, ps.nullable).map_err(|e| format!("<{}> {}: {e}", Self::tag(spec), ps.name))?;
        self.0.doc.borrow_mut().set_prop(n, pid, v).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// The listener registration behind `listen`/`listen_ex`, shared with templates.
    pub(crate) fn write_listen(&self, h: svl_handle, signal: svl_id, on: bool, flags: u32) -> StrResult<()> {
        let (n, k, spec) = self.live(h)?;
        let (sid, ss) = self.0.wire.core_signal(&self.0.registry, k, signal).ok_or_else(|| format!("unknown signal id {signal} on <{}>", Self::tag(spec)))?;
        if flags & !(SVL_LISTEN_CAPTURE | SVL_LISTEN_PASSIVE) != 0 {
            return Err(format!("`{}` of <{}>: presto has no listen flags {flags:#x} (capture and passive only)", ss.name, Self::tag(spec)));
        }
        if flags & SVL_LISTEN_CAPTURE != 0 && !ss.bubbles {
            return Err(format!("`{}` of <{}> has no capture phase: only the DOM-style events propagate", ss.name, Self::tag(spec)));
        }
        let f = on.then_some(ListenFlags { capture: flags & SVL_LISTEN_CAPTURE != 0, passive: flags & SVL_LISTEN_PASSIVE != 0 || ss.passive && !ss.cancelable });
        self.0.doc.borrow_mut().listen(n, sid, f);
        Ok(())
    }

    /// The insert behind `insert`, shared with templates.
    pub(crate) fn write_insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        let (p, _, pspec) = self.live(parent)?;
        let (c, _, _) = self.live(child)?;
        let b = before.map(|b| self.node(b).ok_or_else(|| format!("handle #{b} is not a child of #{parent}"))).transpose()?;
        let s = self.0.wire.slot(pspec, slot).ok_or_else(|| format!("unknown slot id {slot} on <{}>", Self::tag(pspec)))?;
        self.0.doc.borrow_mut().insert(p, c, b, Some(s)).map_err(|e| e.to_string())
    }

    fn forget(&self, h: svl_handle) {
        self.0.observed.borrow_mut().retain(|&(x, _), _| x != h);
        self.0.windows.borrow_mut().retain(|&w| w != h);
        self.0.closes.borrow_mut().retain(|&w| w != h);
    }

    /// Frees the node behind `h` and its handle.
    pub(crate) fn free(&self, h: svl_handle) {
        let Some(n) = self.0.handles.borrow_mut().unbind(h) else { return };
        self.0.doc.borrow_mut().destroy(n);
        self.forget(h);
    }
}

impl Backend for PrestoBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        let reg = &self.0.registry;
        if what == NameKind::Kind {
            return reg.by_tag(name).map(|k| self.0.wire.kind_id(k)).unwrap_or(0);
        }
        let Some(spec) = self.0.wire.kind(kind).and_then(|k| reg.spec(k)) else { return 0 };
        let w = match what {
            NameKind::Prop => What::Prop,
            NameKind::Signal => What::Signal,
            NameKind::Method => What::Method,
            NameKind::Slot => What::Slot,
            _ => return 0,
        };
        self.0.wire.resolve(spec, w, name)
    }

    fn create(&self, kind: svl_id) -> StrResult<svl_handle> {
        let h = self.0.handles.borrow_mut().reserve(1);
        self.create_at(h, kind)?;
        Ok(h)
    }

    fn insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        self.write_insert(parent, child, before, slot)
    }

    fn remove(&self, parent: svl_handle, child: svl_handle) -> StrResult<()> {
        let (p, c) = (self.node(parent), self.node(child));
        let (Some(p), Some(c)) = (p, c) else { return Err("stale or foreign handle".into()) };
        self.0.doc.borrow_mut().remove(p, c).map_err(|e| e.to_string())
    }

    fn destroy(&self, h: svl_handle) {
        // Children still attached are orphaned: the host destroys them itself.
        if self.0.borrowed.borrow().contains(&h) {
            return;
        }
        self.free(h);
    }

    fn set_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        self.write_prop(h, prop, v)
    }

    fn get_prop(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        let (n, k, spec) = self.live(h)?;
        let (pid, ps) = self.0.wire.core_prop(&self.0.registry, k, prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", Self::tag(spec)))?;
        let v = self.0.doc.borrow().get_prop(n, pid).ok_or_else(|| format!("stale handle {h}"))?;
        Ok(value::to_abi(&v, ps.ty))
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let (_, _, spec) = self.live(h)?;
        let m = self.0.wire.method(spec, method).ok_or_else(|| format!("unknown method id {method} on <{}>", Self::tag(spec)))?;
        if !args.is_empty() && m.spec.params.is_empty() {
            return Err(format!("{} takes no arguments", m.spec.name));
        }
        match (m.decl.name, m.spec.name) {
            ("Window", "present") => {
                let mut ws = self.0.windows.borrow_mut();
                if !ws.contains(&h) {
                    ws.push(h);
                }
            }
            ("Window", "close") => self.0.closes.borrow_mut().push(h),
            // focus (`request-focus`) and scrolling (`scroll-into-view`) are presto-core's focus and
            // scroll models, not in the tree API this crate builds on yet (shim)
            (_, other) => return Err(format!("<{}>.{other}() is not implemented on presto yet", Self::tag(spec))),
        }
        Ok(Value::Null)
    }

    fn listen(&self, h: svl_handle, signal: svl_id, on: bool) -> StrResult<()> {
        self.write_listen(h, signal, on, 0)
    }

    /// EV1 capture on the DOM-style events, and `LISTEN_PASSIVE` (compiler-and-js §3.5).
    fn listen_ex(&self, h: svl_handle, signal: svl_id, on: bool, flags: u32) -> StrResult<()> {
        self.write_listen(h, signal, on, flags)
    }

    fn handle_info(&self, h: svl_handle) -> StrResult<svl_handle_info> {
        let (_, k, spec) = self.live(h)?;
        Ok(svl_handle_info {
            kind: self.0.wire.kind_id(k),
            lifetime: if self.0.borrowed.borrow().contains(&h) {
                SVL_LIFE_BORROWED
            } else if spec.chain().any(|s| s.toplevel) {
                SVL_LIFE_TOPLEVEL
            } else {
                SVL_LIFE_OWNED
            },
            flags: 0,
        })
    }

    fn run(&self, host: &HostRef<'_>) -> StrResult<()> {
        self.0.host.set(host.as_raw());
        self.0.quitting.set(false);
        let d = self.driver().ok_or("svelters-presto: no presto driver installed (run an app through svelters-presto-run, or PrestoBackend::set_driver)")?;
        let r = d.run(self);
        self.0.host.set(std::ptr::null());
        r
    }

    fn waker(&self) -> Waker {
        let w = self.0.wake.clone();
        Arc::new(move || w.wake())
    }

    fn quit(&self) {
        self.0.quitting.set(true);
        if let Some(d) = self.driver() {
            d.quit();
        }
    }

    fn schedule_wake(&self, ms: i64) {
        let at = (ms >= 0).then(|| Instant::now() + Duration::from_millis(ms as u64));
        self.0.deadline.set(at);
        if let Some(d) = self.driver() {
            d.timer_changed(at);
        }
    }

    fn request_frame(&self) {
        if !self.0.frame_pending.replace(true)
            && let Some(d) = self.driver()
        {
            d.frame_requested();
        }
    }

    fn style(&self, op: u32, h: svl_handle, text: Option<&str>) -> StrResult<i32> {
        self.style_op(op, h, text)
    }

    fn supports_observe(&self) -> bool {
        true
    }

    fn observe(&self, h: svl_handle, prop: svl_id, on: bool) -> Result<(), ObserveError> {
        let (_, k, spec) = self.live(h).map_err(ObserveError::Failed)?;
        self.0.wire.core_prop(&self.0.registry, k, prop).ok_or_else(|| ObserveError::Failed(format!("unknown prop id {prop} on <{}>", Self::tag(spec))))?;
        // Every prop is exact: the tree is the only place a value changes, and a native change is
        // announced by the kind that makes it (`Dispatched::changed`).
        let mut o = self.0.observed.borrow_mut();
        let c = o.entry((h, prop)).or_insert(0);
        if on {
            *c += 1;
        } else {
            *c = c.saturating_sub(1);
            if *c == 0 {
                o.remove(&(h, prop));
            }
        }
        Ok(())
    }

    fn supports_start(&self) -> bool {
        true
    }

    fn start(&self, host: Option<&HostRef<'_>>) -> StrResult<()> {
        // presto needs no global init here: the driver (or embedder) owns the platform and drives the
        // frames; attaching is all.
        self.0.host.set(host.map(|h| h.as_raw()).unwrap_or(std::ptr::null()));
        Ok(())
    }

    fn describe(&self, what: NameKind, kind: svl_id, member: svl_id) -> Option<MemberDesc> {
        if what != NameKind::Prop {
            return None;
        }
        let spec = self.0.registry.spec(self.0.wire.kind(kind)?)?;
        let p = self.0.wire.prop(spec, member)?.spec;
        let mut slot = SlotDesc::new(value::tag_of(p.ty)).named(p.name);
        if p.nullable {
            slot = slot.flags(SVL_D_NULLABLE);
        }
        if p.ty == crate::shim::ValueType::I64 {
            slot = slot.flags(SVL_D_INTEGRAL);
        }
        Some(MemberDesc { name: p.name.to_string(), params: vec![slot.clone()], ret: slot, flags: SVL_M_OBSERVABLE })
    }
}
