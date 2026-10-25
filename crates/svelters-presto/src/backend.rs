//! [`PrestoBackend`]: the backend ABI on presto's tree (`plans/architecture/backend-abi.md`; the egui
//! backend's `ops.rs` is the template). Every op is a call on the one [`presto_core::Document`] with
//! typed, by-id props, through the member maps of [`crate::kinds`]; handles are this backend's own,
//! allocated in order so a template's nodes are `base + k` (compiler-and-js §3.2).
//!
//! What presto-core does not hold yet is kept here: surface props with no core prop
//! ([`PropTarget::Side`]), each node's style inputs (so one op can change classes without the inline
//! style), the TR1 visual channels and `prefers-color-scheme`, and the sheets by name.
//!
//! The toolkit loop is not here: whoever runs presto (the `presto` facade's app, a test harness)
//! installs a [`Driver`], and [`Backend::run`] hands it the loop. JS-facing events come back through
//! [`PrestoBackend::deliver`] (`deliver.rs`).

use crate::deliver::Output;
use crate::kinds::{KindMap, Kinds, PropTarget};
use crate::surface::{self, ColorScheme, Visual};
use crate::value;
use presto_base::geom::Size;
use presto_base::ids::{NodeId, Revision};
use presto_base::text::{TextMeasure, TextSpec};
use presto_core::{Document, ListenFlags, StyleInput};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
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

/// Measures no text: the backend's document until the runner installs presto-text's system.
pub struct NoText;

impl TextMeasure for NoText {
    fn measure(&mut self, _spec: &TextSpec<'_>, _max_width: Option<f32>) -> Size {
        Size::ZERO
    }
    fn content_widths(&mut self, _spec: &TextSpec<'_>) -> (f32, f32) {
        (0.0, 0.0)
    }
}

pub struct Inner {
    pub(crate) doc: RefCell<Document>,
    pub(crate) kinds: Arc<Kinds>,
    /// Surface props with no core prop, by (node, wire prop id).
    pub(crate) side: RefCell<HashMap<(NodeId, svl_id), surface::Value>>,
    /// Each styled node's style inputs (presto-core takes them whole).
    pub(crate) styles: RefCell<HashMap<NodeId, StyleInput>>,
    pub(crate) visuals: RefCell<HashMap<NodeId, Visual>>,
    pub(crate) scheme: Cell<Option<ColorScheme>>,
    /// Sheets by the runtime's id: presto-core's id and the text.
    pub(crate) sheets: RefCell<BTreeMap<String, (svelters_style::SheetId, String)>>,
    /// Native output queued outside the tree (an embedder's, a test's), delivered by the next pump.
    pub(crate) pending: RefCell<Output>,
    /// While events dispatched at this revision are delivered: JS writes are a JS task that saw it
    /// (native-owned props the kind changed since are not overwritten, architecture §4.5).
    pub(crate) seen: Cell<Option<Revision>>,
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
    /// A backend over a fresh [`Document`] of `kinds`, measuring no text (the runner gives it
    /// presto-text's system with [`PrestoBackend::with_text`]).
    pub fn new(kinds: Arc<Kinds>) -> PrestoBackend {
        PrestoBackend::with_text(kinds, Box::new(NoText))
    }

    pub fn with_text(kinds: Arc<Kinds>, text: Box<dyn TextMeasure>) -> PrestoBackend {
        PrestoBackend(Rc::new(Inner {
            doc: RefCell::new(Document::new(kinds.registry().clone(), text)),
            kinds,
            side: RefCell::new(HashMap::new()),
            styles: RefCell::new(HashMap::new()),
            visuals: RefCell::new(HashMap::new()),
            scheme: Cell::new(None),
            sheets: RefCell::new(BTreeMap::new()),
            pending: RefCell::new(Output::default()),
            seen: Cell::new(None),
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
        PrestoBackend::new(crate::builtin_kinds())
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

    pub fn kinds(&self) -> &Arc<Kinds> {
        &self.0.kinds
    }

    /// The style inputs JS gave `n` (classes, `#id`, inline style).
    pub fn style_input(&self, n: NodeId) -> Option<StyleInput> {
        self.0.styles.borrow().get(&n).cloned()
    }

    /// `n`'s TR1 visual channels.
    pub fn visual(&self, n: NodeId) -> Visual {
        self.0.visuals.borrow().get(&n).copied().unwrap_or_default()
    }

    /// The `prefers-color-scheme` JS asked for (`None`: the platform's).
    pub fn color_scheme(&self) -> Option<ColorScheme> {
        self.0.scheme.get()
    }

    /// The installed sheets by the runtime's id, with their text.
    pub fn sheets(&self) -> BTreeMap<String, String> {
        self.0.sheets.borrow().iter().map(|(k, (_, css))| (k.clone(), css.clone())).collect()
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

    /// The node behind a live handle and its kind's member map.
    pub(crate) fn live(&self, h: svl_handle) -> StrResult<(NodeId, &KindMap)> {
        let n = self.node(h).ok_or_else(|| format!("stale or foreign handle {h}"))?;
        let k = self.0.doc.borrow().kind_of(n).ok_or_else(|| format!("stale handle {h}"))?;
        let m = self.0.kinds.by_core(k).ok_or_else(|| format!("handle {h} has a kind JS does not know"))?;
        Ok((n, m))
    }

    /// Creates a node of wire kind `kind` at handle `h` (reserved by the caller).
    pub(crate) fn create_at(&self, h: svl_handle, kind: svl_id) -> StrResult<NodeId> {
        let m = self.0.kinds.by_wire(kind).ok_or_else(|| format!("unknown kind id {kind}"))?;
        let k = m.core.ok_or_else(|| format!("{} is abstract: no element creates it", m.surface.name))?;
        let n = self.0.doc.borrow_mut().create(k);
        self.0.handles.borrow_mut().bind(h, n);
        Ok(n)
    }

    /// The prop write behind `set_prop`, shared with templates.
    pub(crate) fn write_prop(&self, h: svl_handle, prop: svl_id, v: &Value) -> StrResult<()> {
        let (n, m) = self.live(h)?;
        let pm = m.prop(prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", m.tag()))?;
        let v = value::from_abi(v, pm.spec.ty, pm.spec.nullable).map_err(|e| format!("<{}> {}: {e}", m.tag(), pm.spec.name))?;
        let (pid, v) = match pm.target {
            PropTarget::Side => {
                let mut side = self.0.side.borrow_mut();
                match v {
                    surface::Value::Null => side.remove(&(n, prop)),
                    v => side.insert((n, prop), v),
                };
                return Ok(());
            }
            PropTarget::Native(pid) => (pid, value::to_core(v)),
            PropTarget::Inverted(pid) => (pid, value::to_core(v).map(|v| presto_core::Value::Bool(!v.as_bool().unwrap_or(false)))),
        };
        let write = |d: &mut Document| match v {
            Some(v) => d.set_prop(n, pid, v),
            None => d.reset_prop(n, pid),
        };
        let mut doc = self.0.doc.borrow_mut();
        let r = match self.0.seen.get() {
            Some(seen) => doc.js_transaction(seen, write),
            None => write(&mut doc),
        };
        match r {
            // the kind changed it after the event this task saw: the native value stands (§4.5)
            Ok(_) | Err(presto_core::PropError::Superseded { .. }) => Ok(()),
            Err(e) => Err(format!("<{}> {}: {e}", m.tag(), pm.spec.name)),
        }
    }

    /// The listener registration behind `listen`/`listen_ex`, shared with templates.
    pub(crate) fn write_listen(&self, h: svl_handle, signal: svl_id, on: bool, flags: u32) -> StrResult<()> {
        let (n, m) = self.live(h)?;
        let sm = m.signal(signal).ok_or_else(|| format!("unknown signal id {signal} on <{}>", m.tag()))?;
        let ss = sm.spec;
        if flags & !(SVL_LISTEN_CAPTURE | SVL_LISTEN_PASSIVE) != 0 {
            return Err(format!("`{}` of <{}>: presto has no listen flags {flags:#x} (capture and passive only)", ss.name, m.tag()));
        }
        let capture = flags & SVL_LISTEN_CAPTURE != 0;
        if capture && !ss.bubbles {
            return Err(format!("`{}` of <{}> has no capture phase: only the DOM-style events propagate", ss.name, m.tag()));
        }
        // a signal presto does not produce yet is accepted and never fires
        let Some(sid) = sm.native else { return Ok(()) };
        let mut doc = self.0.doc.borrow_mut();
        if on {
            let mut f = ListenFlags::EMPTY;
            if capture {
                f.insert(ListenFlags::CAPTURE);
            }
            if flags & SVL_LISTEN_PASSIVE != 0 || ss.passive && !ss.cancelable {
                f.insert(ListenFlags::PASSIVE);
            }
            if ss.sync {
                f.insert(ListenFlags::SYNC);
            }
            doc.listen(n, sid, f);
        } else {
            doc.unlisten(n, sid, capture);
        }
        Ok(())
    }

    /// Whether `h` listens to surface signal `name` in the tree (tests, the inspector).
    pub fn listening(&self, h: svl_handle, name: &str) -> bool {
        let Ok((n, m)) = self.live(h) else { return false };
        let Some(sid) = m.signal_named(name).and_then(|s| s.native) else { return false };
        self.0.doc.borrow().listeners(n).iter().any(|l| l.signal == sid)
    }

    /// The insert behind `insert`, shared with templates.
    pub(crate) fn write_insert(&self, parent: svl_handle, child: svl_handle, before: Option<svl_handle>, slot: svl_id) -> StrResult<()> {
        let (p, pm) = self.live(parent)?;
        let (c, _) = self.live(child)?;
        let b = before.map(|b| self.node(b).ok_or_else(|| format!("handle #{b} is not a child of #{parent}"))).transpose()?;
        let s = pm.slot(slot).ok_or_else(|| format!("unknown slot id {slot} on <{}>", pm.tag()))?;
        self.0.doc.borrow_mut().insert(p, c, b, s).map_err(|e| e.to_string())
    }

    fn forget(&self, h: svl_handle) {
        self.0.observed.borrow_mut().retain(|&(x, _), _| x != h);
        self.0.windows.borrow_mut().retain(|&w| w != h);
        self.0.closes.borrow_mut().retain(|&w| w != h);
    }

    /// Frees `n` and everything under it natively (an embed disposing what a root left behind);
    /// returns the freed handles JS knew, parents first.
    pub fn free_subtree(&self, n: NodeId) -> Vec<svl_handle> {
        let mut order = Vec::new();
        let mut stack = vec![n];
        while let Some(x) = stack.pop() {
            order.push(x);
            let kids = self.0.doc.borrow().children(x).to_vec();
            stack.extend(kids.into_iter().rev());
        }
        let mut freed = Vec::new();
        for x in order.iter().rev() {
            let h = self.0.handles.borrow_mut().handle(*x);
            match h {
                Some(h) => {
                    self.free(h);
                    freed.push(h);
                }
                None => self.0.doc.borrow_mut().destroy(*x),
            }
        }
        freed.reverse();
        freed
    }

    /// Frees the node behind `h` and its handle. Children JS knows are detached first, not freed:
    /// the ABI leaves them to the host (presto-core's `destroy` takes the subtree).
    pub(crate) fn free(&self, h: svl_handle) {
        let Some(n) = self.0.handles.borrow_mut().unbind(h) else { return };
        {
            let mut doc = self.0.doc.borrow_mut();
            let known: Vec<NodeId> = doc.children(n).iter().copied().filter(|&c| self.0.handles.borrow().handle(c).is_some()).collect();
            for c in known {
                let _ = doc.remove(n, c);
            }
            doc.destroy(n);
        }
        self.0.side.borrow_mut().retain(|&(x, _), _| x != n);
        self.0.styles.borrow_mut().remove(&n);
        self.0.visuals.borrow_mut().remove(&n);
        self.forget(h);
    }
}

impl Backend for PrestoBackend {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        let kinds = &self.0.kinds;
        if what == NameKind::Kind {
            return kinds.by_tag(name).map_or(0, |m| m.wire);
        }
        let Some(m) = kinds.by_wire(kind) else { return 0 };
        match what {
            NameKind::Prop => m.prop_named(name).map_or(0, |p| p.wire),
            NameKind::Signal => m.signal_named(name).map_or(0, |s| s.wire),
            NameKind::Method => m.methods.iter().find(|x| x.spec.name == name).map_or(0, |x| x.wire),
            NameKind::Slot => m.slots.iter().find(|x| x.spec.name == name).map_or(0, |x| x.wire),
            _ => 0,
        }
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
        let (n, km) = self.live(h)?;
        let pm = km.prop(prop).ok_or_else(|| format!("unknown prop id {prop} on <{}>", km.tag()))?;
        let stored = match pm.target {
            PropTarget::Side => self.0.side.borrow().get(&(n, prop)).cloned(),
            PropTarget::Native(pid) => self.0.doc.borrow().get_prop(n, pid).map(|v| value::from_core(&v)),
            PropTarget::Inverted(pid) => self.0.doc.borrow().get_prop(n, pid).and_then(|v| v.as_bool()).map(|b| surface::Value::Bool(!b)),
        };
        let v = stored.unwrap_or_else(|| pm.spec.default.value());
        Ok(value::to_abi(&v, pm.spec.ty))
    }

    fn call(&self, h: svl_handle, method: svl_id, args: &[Value]) -> StrResult<Value> {
        let (_, km) = self.live(h)?;
        let m = km.method(method).ok_or_else(|| format!("unknown method id {method} on <{}>", km.tag()))?;
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
            (_, "request-focus") => {
                let (n, _) = self.live(h)?;
                let mut doc = self.0.doc.borrow_mut();
                // a node gets a window when its tree is a window's (presto-core `attach_window`, Q4)
                if doc.window_of(n).is_none() {
                    return Err(format!("<{}>.request-focus(): the node is not in a window", km.tag()));
                }
                doc.focus(n, presto_core::FocusReason::Programmatic);
            }
            // scrolling a node into view needs presto-core's scroll model on a laid-out window
            (_, other) => return Err(format!("<{}>.{other}() is not implemented on presto yet", km.tag())),
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
        let (_, km) = self.live(h)?;
        Ok(svl_handle_info {
            kind: km.wire,
            lifetime: if self.0.borrowed.borrow().contains(&h) {
                SVL_LIFE_BORROWED
            } else if km.surface.chain().any(|s| s.toplevel) {
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
        let (_, km) = self.live(h).map_err(ObserveError::Failed)?;
        km.prop(prop).ok_or_else(|| ObserveError::Failed(format!("unknown prop id {prop} on <{}>", km.tag())))?;
        // Every prop is exact: the tree is the only place a value changes, and a native change is
        // in presto-core's change journal (`Change::Prop { native: true }`).
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
        let p = self.0.kinds.by_wire(kind)?.prop(member)?.spec;
        let mut slot = SlotDesc::new(value::tag_of(p.ty)).named(p.name);
        if p.nullable {
            slot = slot.flags(SVL_D_NULLABLE);
        }
        if p.ty == surface::ValueType::I64 {
            slot = slot.flags(SVL_D_INTEGRAL);
        }
        Some(MemberDesc { name: p.name.to_string(), params: vec![slot.clone()], ret: slot, flags: SVL_M_OBSERVABLE })
    }
}
