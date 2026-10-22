//! Engine-agnostic host core: handle table, name→ID caches, event queue, timer heap, error and
//! console sinks, op forwarding to the backend, and the checkpoint algorithm.
//!
//! Nothing here names an engine type. An engine reaches the core through the `op_*`, `timer_*`,
//! `console` and `report` methods and is driven through [`JsEngine`].
//!
//! # Re-entrancy
//!
//! `checkpoint` can be called while JS is already running: a backend op issued from JS may open a
//! modal dialog whose nested toolkit loop calls `host.checkpoint`. The core guards re-entry
//! instead of nesting. Every entry into JS (`checkpoint`, bundle load) holds a [`JsGuard`]; a
//! checkpoint requested while a guard is held does no work, sets `deferred`, and returns. When the
//! outermost guard drops it calls `backend.wake()` so the deferred work runs as an ordinary
//! checkpoint from the toolkit loop. No `RefCell` is borrowed across a call into the backend or
//! the engine, so backend callbacks (`post_event`, `on_destroyed`, nested `checkpoint`) cannot
//! double-borrow.

use crate::engine::{ChangedProp, ErrorKind, HostBusy, JsEngine, JsError, RootMessage};
use crate::sourcemap::SourceMaps;
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::convert::TypeInfo;
use svelters_abi::{
    BackendRef, MemberDesc, NameKind, SVL_CB_ASYNC, SVL_CB_CALL, SVL_CB_FOREVER, SVL_CB_NOTIFIED, SVL_GONE_CLOSED,
    SVL_GONE_OWNER, SVL_GONE_PARENT, SVL_GONE_REPLACED, SVL_GONE_SHUTDOWN, SVL_LIFE_TOPLEVEL, SyncError, Value, gone_name, svl_handle,
    svl_id,
};

mod apps;
mod asyncs;
mod callbacks;
mod offload;
mod ops;
mod reactive;
mod statics;
pub use offload::OffloadStats;
pub use reactive::ReactiveStats;
pub(crate) use callbacks::{CallbackEngine, CallbackRegistry, CbTable};
pub(crate) use ops::{Name, OpError, OpKind};

#[derive(Clone, Debug)]
pub struct HostConfig {
    /// QuickJS heap limit in bytes. 0 means unlimited.
    pub memory_limit: usize,
    /// JS stack limit in bytes.
    pub max_stack_size: usize,
    /// Wall-clock budget for one entry into JS (bundle load, one checkpoint step). The interrupt
    /// handler stops scripts that exceed it. `None` disables the time limit.
    pub turn_budget: Option<Duration>,
    /// Jobs (promise reactions) run per checkpoint before the host yields and calls `wake`.
    pub max_jobs_per_checkpoint: usize,
    /// Echo `console.*` and reported errors to stderr.
    pub echo_console: bool,
    pub echo_errors: bool,
    /// Scripted mode for op-log fixtures, matching the Node TestHost's stepping model:
    /// - timers run on a virtual clock and never wake the backend;
    /// - a checkpoint with queued events only delivers them (no job drain, no timers);
    /// - a checkpoint with no queued events drains jobs and fires the earliest timer, repeating
    ///   until both are empty (at most `max_virtual_timer_fires` timer firings);
    /// - loading a bundle drains nothing.
    pub scripted: bool,
    pub max_virtual_timer_fires: usize,
    /// Accumulate [`Timing`] counters (two clock reads per host op and per entry into JS).
    pub timing: bool,
    /// Ask the backend to quit once the last toplevel handle is gone (after the first one was
    /// created). The host, not the toolkit, decides when the app ends. Ignored in scripted mode.
    pub quit_on_last_window: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        HostConfig {
            memory_limit: 256 * 1024 * 1024,
            max_stack_size: 1024 * 1024,
            turn_budget: Some(Duration::from_secs(10)),
            max_jobs_per_checkpoint: 10_000,
            echo_console: true,
            echo_errors: true,
            scripted: false,
            max_virtual_timer_fires: 10_000,
            timing: false,
            quit_on_last_window: true,
        }
    }
}

/// Time split counters, enabled by [`HostConfig::timing`].
///
/// `js_ns` is the wall time spent inside entries into JS (bundle load, checkpoints). `ops_ns` is
/// the part of that spent inside host ops (marshalling, handle table, name resolution and the
/// backend call itself), so JS-only time is `js_ns - ops_ns`; subtract the backend's own
/// toolkit time from `ops_ns` to get host/ABI time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub js_ns: u64,
    pub ops_ns: u64,
    pub ops: u64,
    pub checkpoints: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLine {
    pub level: String,
    pub text: String,
}

enum Event {
    Signal {
        handle: svl_handle,
        signal: svl_id,
        args: Vec<Value>,
        /// A sync signal that found JS running (`call_sync` answered busy): delivered late, so
        /// its handlers get an event object whose result is ignored.
        deferred: bool,
        /// `SVL_LISTEN_*` flags of the connection that fired (ABI v2.2).
        flags: u32,
    },
    /// The native object behind `handle` died (`reason` is an `SVL_GONE_*` code).
    Gone { handle: svl_handle, reason: u32 },
}

/// A queued event ready for JS: a signal with its name resolved (`__svelters_dispatch`), or a
/// native death (`__svelters_gone`).
pub(crate) enum DispatchEvent {
    Signal {
        handle: svl_handle,
        signal: String,
        args: Vec<Value>,
        deferred: bool,
        flags: u32,
    },
    Gone {
        handle: svl_handle,
        reason: &'static str,
    },
}

/// One observed (handle, prop): see `op_observe` and REACTIVITY.md §5.2.
pub(crate) struct Observed {
    /// JS-side observers (`host.observe(on)` calls not yet matched by an `off`).
    pub count: u32,
    /// The prop name as JS first asked for it: what `__svelters_changed` reports.
    pub name: String,
    /// `SVL_M_PROPORTIONAL`: never read or compared, delivered as an invalidation.
    pub proportional: bool,
    /// The last value delivered to JS or read back after our own `set_prop`; `None` = unknown
    /// (the next notification is delivered whatever the value). Never holds a `BOXED` value.
    pub last: Option<Value>,
    /// Deliver at the next checkpoint even if equal to `last`: our own write was normalized by the
    /// toolkit (the read-back differs from what JS wrote), so JS must see the value once.
    pub force: bool,
}

/// Why a handle is no longer live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Death {
    /// The app called `destroy`.
    Destroyed,
    /// The backend reported it (`SVL_GONE_*`).
    Gone(u32),
}

impl Death {
    fn describe(self) -> &'static str {
        match self {
            Death::Destroyed => "destroyed by the app",
            Death::Gone(SVL_GONE_CLOSED) => "its window was closed",
            Death::Gone(SVL_GONE_PARENT) => "its parent was destroyed",
            Death::Gone(SVL_GONE_OWNER) => "its owner deleted it",
            Death::Gone(SVL_GONE_REPLACED) => "it was replaced",
            Death::Gone(SVL_GONE_SHUTDOWN) => "the app is shutting down",
            Death::Gone(_) => "destroyed natively",
        }
    }
}

/// How many dead handles are remembered for error messages.
const DEAD_RING: usize = 4096;
/// Handles must stay exactly representable as JS numbers.
const MAX_HANDLE: svl_handle = 1 << 53;

/// Bounded memory of dead handles. Handles are never reused, so an entry can only ever describe
/// the one object that held that handle.
#[derive(Default)]
struct DeadRing {
    order: VecDeque<svl_handle>,
    why: HashMap<svl_handle, Death>,
}

impl DeadRing {
    fn push(&mut self, h: svl_handle, d: Death) {
        if self.why.insert(h, d).is_none() {
            self.order.push_back(h);
            if self.order.len() > DEAD_RING {
                if let Some(old) = self.order.pop_front() {
                    self.why.remove(&old);
                }
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Timers {
    heap: BinaryHeap<Reverse<(Instant, u64, u32)>>,
    live: HashMap<u32, (u64, Option<Duration>)>,
    seq: u64,
}

impl Timers {
    fn set(&mut self, id: u32, delay: Duration, repeat: bool, now: Instant) {
        self.seq += 1;
        let interval = repeat.then_some(delay);
        self.live.insert(id, (self.seq, interval));
        self.heap.push(Reverse((now + delay, self.seq, id)));
    }

    fn next_deadline(&mut self) -> Option<Instant> {
        while let Some(Reverse((d, seq, id))) = self.heap.peek().copied() {
            if self.live.get(&id).map(|l| l.0) == Some(seq) {
                return Some(d);
            }
            self.heap.pop();
        }
        None
    }

    fn take_due(&mut self, now: Instant) -> Vec<(u32, u64)> {
        let mut due = Vec::new();
        while let Some(Reverse((d, seq, id))) = self.heap.peek().copied() {
            if d > now {
                break;
            }
            self.heap.pop();
            if self.live.get(&id).map(|l| l.0) == Some(seq) {
                due.push((id, seq));
            }
        }
        due
    }

    /// Marks `(id, seq)` as firing. Returns false if the timer was cleared or replaced since
    /// `take_due`. Intervals are rescheduled; one-shots are dropped.
    fn begin_fire(&mut self, id: u32, seq: u64, now: Instant) -> bool {
        match self.live.get(&id).copied() {
            Some((s, interval)) if s == seq => {
                match interval {
                    Some(iv) => {
                        self.seq += 1;
                        self.live.insert(id, (self.seq, interval));
                        self.heap.push(Reverse((now + iv, self.seq, id)));
                    }
                    None => {
                        self.live.remove(&id);
                    }
                }
                true
            }
            _ => false,
        }
    }
}

#[derive(Default)]
struct CoreState {
    /// live handle → kind id
    handles: HashMap<svl_handle, svl_id>,
    /// live handles of the toplevel lifetime class
    toplevels: HashSet<svl_handle>,
    top_seen: bool,
    quit_sent: bool,
    dead: DeadRing,
    /// child -> parent for handles currently inserted somewhere; a live handle that is not a key
    /// is a root (a toplevel, or not attached yet). Used by reload teardown.
    tree: HashMap<svl_handle, svl_handle>,
    kinds: HashMap<String, svl_id>,
    /// kind id → name, for error messages
    kind_names: HashMap<svl_id, String>,
    /// (name kind, scope kind) → name → id
    names: HashMap<(u32, svl_id), HashMap<Box<str>, svl_id>>,
    /// Descriptor caches: `None` records that the backend has no descriptor.
    members: HashMap<(u32, svl_id, svl_id), Option<Rc<MemberDesc>>>,
    types: HashMap<svl_id, Option<Rc<TypeInfo>>>,
    /// Live boxed-value handles, and ones the JS side dropped (released at the next checkpoint).
    /// Value handles also remember their kind: VALUE handles take method calls (ABI v2.5).
    boxed: HashMap<svl_handle, svl_id>,
    release_queue: Vec<svl_handle>,
    callbacks: CbTable,
    /// (kind id, signal id) → name, filled by `listen`
    signal_names: HashMap<(svl_id, svl_id), String>,
    /// ABI v2.6 observations: (handle, prop id) → refcount, name, cost class, last value.
    observed: HashMap<(svl_handle, svl_id), Observed>,
    /// Observed (handle, prop) pairs announced by `prop_changed` since the last checkpoint, in
    /// first-announcement order, each once (`dirty_set` deduplicates).
    dirty: Vec<(svl_handle, svl_id)>,
    dirty_set: HashSet<(svl_handle, svl_id)>,
    events: VecDeque<Event>,
    /// Handles carried in the arguments of queued events (a list item in a deferred `bind`, N3), with
    /// a count: `destroy` leaves a pinned handle alive, so a wrapper collected while its event waits
    /// cannot kill the handle the event is about to deliver. Unpinned when the event is delivered.
    pins: HashMap<svl_handle, u32>,
    timers: Timers,
    console: Vec<ConsoleLine>,
    errors: Vec<JsError>,
}

pub(crate) struct Core {
    backend: BackendRef<'static>,
    pub(crate) cfg: HostConfig,
    st: RefCell<CoreState>,
    busy: Cell<u32>,
    deferred: Cell<bool>,
    /// The deadline last passed to `backend.schedule_wake` (None: cancelled or never armed).
    armed: Cell<Option<Instant>>,
    /// `backend.request_frame` was called and `host.frame` has not arrived yet.
    frame_requested: Cell<bool>,
    pub(crate) interrupt: Arc<AtomicBool>,
    epoch: Instant,
    vnow: Cell<Duration>,
    timing: Cell<Timing>,
    checkpoints: Cell<u64>,
    cb_engine: RefCell<Option<Rc<dyn CallbackEngine>>>,
    /// Depth of native-callback invocations in progress (see `callbacks.rs`).
    cb_depth: Cell<u32>,
    pub(crate) maps: RefCell<SourceMaps>,
    /// DX1: the app's settings, UI state and paths (`Host::set_dx`).
    pub(crate) dx: RefCell<Option<Rc<crate::dx::Dx>>>,
    /// App exports (E0): this host's exported Rust objects.
    pub(crate) apps: crate::exports::objects::AppObjects,
    /// LD1: whether an `svelters:app/<module>` export is reachable by `construct` and `callStatic`,
    /// which need no import. Set by `Host::set_module_hooks` from the extension policy.
    pub(crate) app_gate: RefCell<Option<Rc<dyn Fn(&str) -> bool>>>,
    /// Where app-export calls are logged (the op log of a test harness), if anywhere.
    pub(crate) op_log: RefCell<Option<Rc<dyn Fn(&str)>>>,
    /// Offloaded app-export calls in flight (E1) and the completion queue workers post to.
    pub(crate) ops: offload::OpTable,
    /// Stores, events and persisted objects of app exports (E3).
    pub(crate) reactive: reactive::Reactive,
    /// The turn budget armed for the next entries into JS (starts as `cfg.turn_budget`; an embedder
    /// sets per-entry budgets, ARCH-EMBED §4.2 `Budgets`).
    pub(crate) budget: Cell<Option<Duration>>,
    /// EM1: what mounted roots reported (`__svelters_host.rootFault`/`rootInfo`/`rootEmit`).
    pub(crate) root_msgs: RefCell<Vec<RootMessage>>,
}

/// Held while JS runs. See the module docs on re-entrancy.
pub(crate) struct JsGuard<'a>(&'a Core, Option<Instant>);

impl Drop for JsGuard<'_> {
    fn drop(&mut self) {
        let c = self.0;
        if let Some(t0) = self.1 {
            let mut t = c.timing.get();
            t.js_ns += t0.elapsed().as_nanos() as u64;
            c.timing.set(t);
        }
        c.busy.set(c.busy.get() - 1);
        if c.busy.get() == 0 && c.deferred.replace(false) {
            c.backend.wake();
        }
    }
}

impl Core {
    pub(crate) fn new(backend: BackendRef<'static>, cfg: HostConfig) -> Core {
        let turn_budget = cfg.turn_budget;
        Core {
            backend,
            cfg,
            st: RefCell::new(CoreState::default()),
            busy: Cell::new(0),
            deferred: Cell::new(false),
            armed: Cell::new(None),
            frame_requested: Cell::new(false),
            interrupt: Arc::new(AtomicBool::new(false)),
            epoch: Instant::now(),
            vnow: Cell::new(Duration::ZERO),
            timing: Cell::new(Timing::default()),
            checkpoints: Cell::new(0),
            cb_engine: RefCell::new(None),
            cb_depth: Cell::new(0),
            maps: RefCell::default(),
            apps: Default::default(),
            app_gate: RefCell::new(None),
            dx: RefCell::new(None),
            op_log: RefCell::new(None),
            ops: offload::OpTable::new(crate::exports::offload::Completions::new(backend.wake_handle())),
            reactive: reactive::Reactive::new(crate::exports::reactive::Hub::new(backend.wake_handle())),
            budget: Cell::new(turn_budget),
            root_msgs: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn take_timing(&self) -> Timing {
        self.timing.replace(Timing::default())
    }

    pub(crate) fn checkpoint_count(&self) -> u64 {
        self.checkpoints.get()
    }

    /// Runs one host op, adding its duration to the timing counters when enabled.
    fn timed<R>(&self, name: &'static str, f: impl FnOnce() -> R) -> R {
        crate::crash::note_op(name);
        if !self.cfg.timing {
            return f();
        }
        let t0 = Instant::now();
        let r = f();
        let mut t = self.timing.get();
        t.ops_ns += t0.elapsed().as_nanos() as u64;
        t.ops += 1;
        self.timing.set(t);
        r
    }

    /// Every queue the core holds (ARCH-EMBED §5.4); `jobs` comes from the engine.
    pub(crate) fn busy(&self, jobs: bool) -> HostBusy {
        let next_timer = self.next_timer_deadline();
        let st = self.st.borrow();
        HostBusy {
            events: !st.events.is_empty(),
            changes: !st.dirty.is_empty(),
            jobs,
            ops_in_flight: self.ops_in_flight(),
            stores: self.reactive.hub.pending(),
            frame: self.frame_requested.get(),
            running: self.busy.get() > 0,
            next_timer,
        }
    }

    pub(crate) fn root_message(&self, m: RootMessage) {
        if let RootMessage::Fault { error, .. } = &m
            && self.cfg.echo_errors
        {
            eprintln!("[svelters] root fault: {error}");
        }
        self.root_msgs.borrow_mut().push(m);
    }

    pub(crate) fn take_root_messages(&self) -> Vec<RootMessage> {
        std::mem::take(&mut *self.root_msgs.borrow_mut())
    }

    /// True when nothing is waiting for the host: no queued events and no live timers.
    pub(crate) fn is_idle(&self) -> bool {
        let st = self.st.borrow();
        self.busy.get() == 0 && st.events.is_empty() && st.timers.live.is_empty()
    }

    /// The timer clock: wall time, or the virtual clock in scripted mode.
    pub(crate) fn now(&self) -> Instant {
        if self.cfg.scripted {
            self.epoch + self.vnow.get()
        } else {
            Instant::now()
        }
    }

    /// `performance.now()` in scripted mode: the virtual clock in milliseconds, so animations
    /// (TR1) advance with the virtual 16 ms frames instead of the wall clock and end after a fixed
    /// number of frames. `None` outside scripted mode (wall time).
    pub(crate) fn virtual_ms(&self) -> Option<f64> {
        self.cfg.scripted.then(|| self.vnow.get().as_secs_f64() * 1000.0)
    }

    pub(crate) fn backend(&self) -> BackendRef<'static> {
        self.backend
    }

    /// Enters JS. `None` if JS is already running (re-entry); the request is then recorded as
    /// deferred.
    pub(crate) fn enter(&self) -> Option<JsGuard<'_>> {
        if self.busy.get() > 0 {
            self.deferred.set(true);
            return None;
        }
        self.busy.set(1);
        Some(JsGuard(self, self.cfg.timing.then(Instant::now)))
    }

    // ---- sinks ----

    pub(crate) fn report(&self, err: JsError) {
        let err = self.map_error(err);
        if self.cfg.echo_errors {
            eprintln!("[svelters] {err}");
        }
        crate::crash::note_error(err.to_string());
        self.st.borrow_mut().errors.push(err);
    }

    pub(crate) fn report_all(&self, errs: Vec<JsError>) {
        for e in errs {
            self.report(e);
        }
    }

    pub(crate) fn console(&self, level: &str, text: String) {
        let text = if matches!(level, "error" | "warn" | "trace") {
            self.maps.borrow_mut().map_text(&text)
        } else {
            text
        };
        self.emit(level, None, text);
    }

    /// DX1: one JS log record (`console.*`, or `logger(category)`): to `tracing` (target `js`, the
    /// category a field), to stderr when echoing, and to the console buffer.
    pub(crate) fn emit(&self, level: &str, category: Option<&str>, text: String) {
        let cat = category.unwrap_or("");
        match level {
            "error" => tracing::error!(target: "js", category = cat, "{text}"),
            "warn" => tracing::warn!(target: "js", category = cat, "{text}"),
            "debug" => tracing::debug!(target: "js", category = cat, "{text}"),
            "trace" => tracing::trace!(target: "js", category = cat, "{text}"),
            _ => tracing::info!(target: "js", category = cat, "{text}"),
        }
        if self.cfg.echo_console {
            match category {
                Some(c) => eprintln!("[{level}] {c}: {text}"),
                None => eprintln!("[{level}] {text}"),
            }
        }
        self.st.borrow_mut().console.push(ConsoleLine {
            level: level.to_string(),
            text,
        });
    }

    /// DX1: the `__svelters_host.dx` op. Errors are `(code, message)`.
    pub(crate) fn dx_op(&self, op: &str, a: &str, b: &str) -> Result<String, (&'static str, String)> {
        if op == "log" {
            let (level, cat) = a.split_once('\n').unwrap_or((a, ""));
            let text = if matches!(level, "error" | "warn" | "trace") { self.maps.borrow_mut().map_text(b) } else { b.to_string() };
            self.emit(level, Some(cat), text);
            return Ok(String::new());
        }
        let dx = self.dx.borrow().clone();
        match dx {
            Some(dx) => dx.op(op, a, b),
            None => Err(crate::dx::unavailable()),
        }
    }

    pub(crate) fn take_errors(&self) -> Vec<JsError> {
        std::mem::take(&mut self.st.borrow_mut().errors)
    }

    pub(crate) fn take_console(&self) -> Vec<ConsoleLine> {
        std::mem::take(&mut self.st.borrow_mut().console)
    }

    // ---- events ----

    pub(crate) fn push_event(&self, handle: svl_handle, signal: svl_id, args: Vec<Value>) {
        let (signal, flags) = split_signal(signal);
        self.pin_args(&args, true);
        self.st.borrow_mut().events.push_back(Event::Signal {
            handle,
            signal,
            args,
            deferred: false,
            flags,
        });
    }

    /// Pins (or unpins) the handles in the arguments of a queued event.
    pub(crate) fn pin_args(&self, args: &[Value], on: bool) {
        let mut hs = Vec::new();
        for a in args {
            handles_in(a, &mut hs);
        }
        if hs.is_empty() {
            return;
        }
        let mut st = self.st.borrow_mut();
        for h in hs {
            if on {
                *st.pins.entry(h).or_insert(0) += 1;
            } else if let Some(n) = st.pins.get_mut(&h) {
                *n -= 1;
                if *n == 0 {
                    st.pins.remove(&h);
                }
            }
        }
    }

    pub(crate) fn pinned(&self, h: svl_handle) -> bool {
        self.st.borrow().pins.contains_key(&h)
    }

    /// The name of `signal` on `h` if `h` is live and a listener was registered for it.
    fn listened_signal(&self, h: svl_handle, signal: svl_id) -> Option<String> {
        let st = self.st.borrow();
        st.handles
            .get(&h)
            .and_then(|k| st.signal_names.get(&(*k, signal)))
            .cloned()
    }

    /// ABI v2.6 `host.prop_changed`: marks an observed (h, prop) dirty. Returns true for the first
    /// insert into an empty dirty set, when the caller wakes the backend (one wake per turn).
    /// Unobserved pairs (cross-notified props, a late native emission after `observe(off)`) and
    /// repeats are ignored. Never runs JS and never reads the property: that is the checkpoint's job.
    pub(crate) fn prop_changed(&self, h: svl_handle, prop: svl_id) -> bool {
        let Ok(mut st) = self.st.try_borrow_mut() else { return false };
        if !st.observed.contains_key(&(h, prop)) {
            return false;
        }
        mark_dirty(&mut st, (h, prop))
    }

    /// Queued events or dirty observed properties: work for the next checkpoint.
    pub(crate) fn has_events(&self) -> bool {
        let st = self.st.borrow();
        !st.events.is_empty() || !st.dirty.is_empty()
    }

    /// Drains the dirty set into a `__svelters_changed` batch (REACTIVITY.md §5.2): each pair that
    /// is still observed is read natively (no JS) unless it is proportional, compared with its
    /// `last` value and kept only if it differs (or a normalized write forces it). Never runs JS.
    pub(crate) fn take_changes(&self) -> Vec<ChangedProp> {
        let dirty = {
            let mut st = self.st.borrow_mut();
            st.dirty_set.clear();
            std::mem::take(&mut st.dirty)
        };
        let mut out = Vec::new();
        for key in dirty {
            let (name, proportional, force) = {
                let mut st = self.st.borrow_mut();
                let Some(o) = st.observed.get_mut(&key) else { continue };
                (o.name.clone(), o.proportional, std::mem::take(&mut o.force))
            };
            let app = crate::exports::is_app_handle(key.0);
            if proportional {
                if app {
                    self.note_store_change(true);
                }
                out.push(ChangedProp { handle: key.0, prop: name, value: None });
                continue;
            }
            // E3: a store field is read through its getter (a clone under the store's read lock).
            if app {
                let Some(v) = self.app_prop_value(key.0, key.1) else { continue };
                let changed = {
                    let mut st = self.st.borrow_mut();
                    let Some(o) = st.observed.get_mut(&key) else { continue };
                    let changed = force || !o.last.as_ref().is_some_and(|l| same_value(l, &v));
                    if changed {
                        o.last = Some(v.clone());
                    }
                    changed
                };
                self.note_store_change(changed);
                if changed {
                    out.push(ChangedProp { handle: key.0, prop: name, value: Some(v) });
                }
                continue;
            }
            // A failed read (the object died meanwhile) delivers nothing; `on_destroyed` follows.
            let Ok(v) = self.backend.get_prop(key.0, key.1) else { continue };
            self.adopt_handles(&v);
            let changed = {
                let mut st = self.st.borrow_mut();
                let Some(o) = st.observed.get_mut(&key) else { continue };
                let changed = force || !o.last.as_ref().is_some_and(|l| same_value(l, &v));
                if changed {
                    o.last = (!has_boxed(&v)).then(|| v.clone());
                }
                changed
            };
            if changed {
                out.push(ChangedProp { handle: key.0, prop: name, value: Some(v) });
            } else {
                // Dropped: a read that minted VALUE handles must not leak them.
                release_boxed_in(&self.backend, &v);
            }
        }
        out
    }

    /// Reads (h, prop) to become its `last` value after our own `set_prop` (the first observation
    /// reads nothing since FU3: `last` starts unknown). A value holding `BOXED` handles is released at once and leaves `last` unknown
    /// (each read mints new handles, so such a value always counts as changed).
    pub(crate) fn read_last(&self, h: svl_handle, p: svl_id) -> Option<Value> {
        if crate::exports::is_app_handle(h) {
            return self.app_prop_value(h, p);
        }
        let v = self.backend.get_prop(h, p).ok()?;
        if has_boxed(&v) {
            release_boxed_in(&self.backend, &v);
            return None;
        }
        self.adopt_handles(&v);
        Some(v)
    }

    /// The set path of REACTIVITY.md §5.2: after a successful `set_prop` on an observed, non-
    /// proportional (h, prop), read the value back once and store it as `last`, so the native echo
    /// of our write is dropped at the checkpoint. When the toolkit normalized the value (the read
    /// differs from `written`), the pair is forced dirty so JS gets the normalized value once.
    pub(crate) fn after_set(&self, h: svl_handle, p: svl_id, written: Option<Value>) {
        match self.st.borrow().observed.get(&(h, p)) {
            Some(o) if !o.proportional => {}
            _ => return,
        }
        let last = self.read_last(h, p);
        let normalized = match (&written, &last) {
            (Some(w), Some(l)) => !same_value(w, l),
            _ => false,
        };
        let wake = {
            let mut st = self.st.borrow_mut();
            let Some(o) = st.observed.get_mut(&(h, p)) else { return };
            o.last = last;
            if normalized {
                o.force = true;
                mark_dirty(&mut st, (h, p))
            } else {
                false
            }
        };
        if wake && !self.cfg.scripted {
            self.backend.wake();
        }
    }

    /// Drains the queue, resolving signal ids to names. Signals for handles that died are
    /// dropped; other events that cannot be mapped are reported and dropped.
    pub(crate) fn take_events(&self) -> Vec<DispatchEvent> {
        let events: Vec<Event> = self.st.borrow_mut().events.drain(..).collect();
        let mut out = Vec::new();
        for e in events {
            let (handle, signal, args, deferred, flags) = match e {
                Event::Gone { handle, reason } => {
                    out.push(DispatchEvent::Gone {
                        handle,
                        reason: gone_name(reason),
                    });
                    continue;
                }
                Event::Signal {
                    handle,
                    signal,
                    args,
                    deferred,
                    flags,
                } => (handle, signal, args, deferred, flags),
            };
            let app = crate::exports::is_app_handle(handle);
            let name = {
                let st = self.st.borrow();
                // E3: an app object released since its event was queued is not an error.
                if st.dead.why.contains_key(&handle) || (app && self.apps.kind(handle).is_none()) {
                    drop(st);
                    self.pin_args(&args, false);
                    continue;
                }
                let kind = if app { self.apps.kind(handle) } else { st.handles.get(&handle).copied() };
                kind.and_then(|k| st.signal_names.get(&(k, signal))).cloned()
            };
            // Objects the backend minted for the event (a GFileMonitor's file) join the handle table.
            if name.is_some() {
                for a in &args {
                    self.adopt_handles(a);
                }
            }
            match name {
                Some(signal) => out.push(DispatchEvent::Signal {
                    handle,
                    signal,
                    args,
                    deferred,
                    flags,
                }),
                None => {
                    self.pin_args(&args, false);
                    self.report(JsError::new(
                    ErrorKind::Host,
                    format!(
                        "dropped event for handle {handle} signal id {signal}: no listener or stale handle"
                    ),
                ))
                }
            }
        }
        out
    }

    /// The backend reports that a native object went away. Records the death and queues the
    /// notification for the next checkpoint; never runs JS. At most once per handle: a handle
    /// the app already destroyed, or one reported before, is ignored.
    pub(crate) fn native_gone(&self, h: svl_handle, reason: u32) {
        {
            let mut st = self.st.borrow_mut();
            if st.handles.remove(&h).is_none() {
                return;
            }
            st.toplevels.remove(&h);
            st.tree.remove(&h);
            st.observed.retain(|k, _| k.0 != h);
            st.dirty.retain(|k| k.0 != h);
            st.dead.push(h, Death::Gone(reason));
            st.events.push_back(Event::Gone { handle: h, reason });
        }
        // Only marks the owner's callbacks dead; their JS functions are dropped at the next checkpoint.
        self.cb_release_owner(h);
    }

    /// Asks the backend to quit once, when the last toplevel is gone.
    fn quit_if_last_window_gone(&self) {
        if self.cfg.scripted || !self.cfg.quit_on_last_window {
            return;
        }
        let quit = {
            let mut st = self.st.borrow_mut();
            let q = st.top_seen && st.toplevels.is_empty() && !st.quit_sent;
            st.quit_sent |= q;
            q
        };
        if quit {
            self.backend.quit();
        }
    }

    // ---- timers ----

    pub(crate) fn timer_set(&self, id: u32, delay: Duration, repeat: bool) {
        let now = self.now();
        // Wall-clock timers have a 1 ms floor; the virtual clock orders by exact due time.
        let delay = if self.cfg.scripted {
            delay
        } else {
            delay.max(Duration::from_millis(1))
        };
        let deadline = {
            let mut st = self.st.borrow_mut();
            st.timers.set(id, delay, repeat, now);
            st.timers.next_deadline()
        };
        self.arm_sched(deadline, false);
    }

    /// Arms the backend's one-shot wakeup for the earliest timer deadline. Scripted mode keeps
    /// timers on its virtual clock and never touches the backend. `force` re-arms even when the
    /// deadline is unchanged (the previous wakeup has fired).
    fn arm_sched(&self, deadline: Option<Instant>, force: bool) {
        if self.cfg.scripted {
            return;
        }
        if !force && self.armed.get() == deadline {
            return;
        }
        self.armed.set(deadline);
        let ms = match deadline {
            Some(d) => {
                let left = d.saturating_duration_since(Instant::now());
                let ms = left.as_nanos().div_ceil(1_000_000);
                i64::try_from(ms).unwrap_or(i64::MAX)
            }
            None => -1,
        };
        self.backend.schedule_wake(ms);
    }

    /// Cancels any pending backend wakeup. Called when the host goes away.
    pub(crate) fn disarm(&self) {
        if self.armed.replace(None).is_some() {
            self.backend.schedule_wake(-1);
        }
    }

    pub(crate) fn timer_clear(&self, id: u32) {
        let deadline = {
            let mut st = self.st.borrow_mut();
            st.timers.live.remove(&id);
            st.timers.next_deadline()
        };
        self.arm_sched(deadline, false);
    }

    fn timers_take_due(&self, now: Instant) -> Vec<(u32, u64)> {
        self.st.borrow_mut().timers.take_due(now)
    }

    fn timer_begin_fire(&self, id: u32, seq: u64, now: Instant) -> bool {
        self.st.borrow_mut().timers.begin_fire(id, seq, now)
    }

    fn sync_sched(&self) {
        let d = self.st.borrow_mut().timers.next_deadline();
        self.arm_sched(d, true);
    }

    fn next_timer_deadline(&self) -> Option<Instant> {
        self.st.borrow_mut().timers.next_deadline()
    }

    fn advance_clock_to(&self, t: Instant) {
        let off = t.saturating_duration_since(self.epoch);
        if off > self.vnow.get() {
            self.vnow.set(off);
        }
    }
}

/// Runs one checkpoint (see "Checkpoint semantics" in `backend-abi.md`):
/// 0. read, compare and deliver the dirty observed properties as one `__svelters_changed` batch
///    (ABI v2.6), with no drain before the first event;
/// 1. deliver queued events to JS, draining jobs after each;
/// 2. fire due timers, draining jobs after each;
/// 3. drain remaining jobs, up to `max_jobs_per_checkpoint` in total;
/// 4. `wake` if jobs or events remain, and arm the backend's one-shot wakeup for the earliest
///    timer deadline (`schedule_wake`).
///
/// Ops reach the backend directly while JS runs (no batching in P). Scripted mode has its own
/// stepping, see [`HostConfig::scripted`].
pub(crate) fn checkpoint(core: &Core, engine: &RefCell<Box<dyn JsEngine>>) {
    let Some(_guard) = core.enter() else { return };
    core.checkpoints.set(core.checkpoints.get() + 1);
    if core.cfg.timing {
        let mut t = core.timing.get();
        t.checkpoints += 1;
        core.timing.set(t);
    }
    let mut eng = engine.borrow_mut();
    // E1 (egui-rust-exports.md §6.2): offloaded export calls settle first, so the changes, events
    // and the one job drain below see their results in the same checkpoint.
    offload::drain(core, &mut eng);
    // E3 (§6.4): dirty store fields join the dirty set, queued `Emitter` events the event queue, so the
    // change batch and the events below carry them.
    reactive::collect(core);
    if core.cfg.scripted {
        checkpoint_scripted(core, &mut eng);
        core.process_releases();
        return;
    }
    let mut budget = core.cfg.max_jobs_per_checkpoint;
    let mut pending = false;

    let drain = |eng: &mut Box<dyn JsEngine>, budget: &mut usize| {
        let st = eng.run_jobs(*budget);
        *budget = budget.saturating_sub(st.ran);
        core.report_all(eng.take_errors());
        st.pending
    };

    // REACTIVITY.md §5.3: the batch of changed properties first, then the queued signal events,
    // with no job drain in between (handlers see the new state; Svelte flushes once after them).
    deliver_changes(core, &mut eng);
    for ev in core.take_events() {
        if let Err(e) = deliver(core, &mut eng, ev) {
            core.report(e);
        }
        pending |= drain(&mut eng, &mut budget);
    }

    let now = core.now();
    for (id, seq) in core.timers_take_due(now) {
        if !core.timer_begin_fire(id, seq, now) {
            continue;
        }
        if let Err(e) = eng.fire_timer(id) {
            core.report(e);
        }
        pending |= drain(&mut eng, &mut budget);
    }

    pending |= drain(&mut eng, &mut budget);
    drop(eng);

    // GC'd boxed values were only queued by the finalization callbacks: release them here,
    // outside any entry into JS.
    core.process_releases();
    core.sync_sched();
    if pending || core.has_events() {
        core.backend.wake();
    }
    core.quit_if_last_window_gone();
}

impl Core {
    /// `requestAnimationFrame` needs a frame: asks the backend for one unless a request is
    /// already pending. Returns false in scripted mode, where JS uses a virtual timer instead.
    pub(crate) fn frame_request(&self) -> bool {
        if self.cfg.scripted {
            return false;
        }
        if !self.frame_requested.replace(true) {
            self.backend.request_frame();
        }
        true
    }
}

/// `host.frame`: the backend's frame clock fired. Runs the animation-frame callbacks and then
/// the jobs they queued (same budget as a checkpoint). Callbacks that request another frame ask
/// the backend again, so the next frame comes from the clock, not from this call. A frame that
/// arrives while JS is running is requested again.
pub(crate) fn frame(core: &Core, engine: &RefCell<Box<dyn JsEngine>>) {
    core.frame_requested.set(false);
    let Some(_guard) = core.enter() else {
        core.frame_request();
        return;
    };
    let Ok(mut eng) = engine.try_borrow_mut() else {
        core.frame_request();
        return;
    };
    if let Err(e) = eng.run_frame() {
        core.report(e);
    }
    let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
    core.report_all(eng.take_errors());
    drop(eng);
    core.sync_sched();
    if st.pending || core.has_events() {
        core.backend.wake();
    }
    core.quit_if_last_window_gone();
}

/// `host.call_sync`: runs the JS listeners of `signal` on `h` now and returns the handler
/// result (`Null` = no decision). See "ABI v2.1" in `backend-abi.md`.
///
/// - JS already running: the event is queued for async delivery and the answer is
///   [`SyncError::Busy`]; the backend uses the signal's deferred result.
/// - The call holds the JS guard, so a checkpoint requested meanwhile (a nested loop) is
///   deferred, and the turn budget applies. No jobs run here; if some are pending afterwards
///   the host wakes the backend so the next checkpoint runs them.
/// - A throwing handler reports the error and answers `Null`.
pub(crate) fn call_sync(
    core: &Core,
    engine: &RefCell<Box<dyn JsEngine>>,
    h: svl_handle,
    signal: svl_id,
    args: &[Value],
) -> Result<Value, SyncError> {
    let (signal, flags) = split_signal(signal);
    let Some(name) = core.listened_signal(h, signal) else {
        return Err(SyncError::Failed(format!(
            "call_sync: no listener for signal id {signal} on handle {h}"
        )));
    };
    if core.js_running() || engine.try_borrow_mut().is_err() {
        core.pin_args(args, true);
        core.st.borrow_mut().events.push_back(Event::Signal {
            handle: h,
            signal,
            args: args.to_vec(),
            deferred: true,
            flags,
        });
        if !core.cfg.scripted {
            core.backend.wake();
        }
        return Err(SyncError::Busy);
    }
    let Some(_guard) = core.enter() else {
        return Err(SyncError::Busy);
    };
    // Objects the backend minted for the call (a list item in a virtual-list `bind`, N3) join the
    // handle table, as they do for queued events (`take_events`).
    for a in args {
        core.adopt_handles(a);
    }
    let mut eng = engine.borrow_mut();
    // REACTIVITY.md §5.2 "sync handlers": invalidate what changed before the handler runs (no
    // drain), so `$derived` values it reads are fresh. Stores included (E3).
    reactive::collect(core);
    deliver_changes(core, &mut eng);
    let result = match eng.dispatch_sync(h, &name, args, flags) {
        Ok(v) => v,
        Err(e) => {
            core.report(e.at_handle(h));
            Value::Null
        }
    };
    core.report_all(eng.take_errors());
    let pending = eng.run_jobs(0).pending;
    drop(eng);
    if pending && !core.cfg.scripted {
        core.backend.wake();
    }
    Ok(result)
}

/// The handles inside a value (lists, records and maps included).
fn handles_in(v: &Value, out: &mut Vec<svl_handle>) {
    match v {
        Value::Handle(h) => out.push(*h),
        Value::List(items) => items.iter().for_each(|i| handles_in(i, out)),
        Value::Record { fields, .. } => fields.iter().for_each(|i| handles_in(i, out)),
        Value::Map(entries) => entries.iter().for_each(|(_, v)| handles_in(v, out)),
        _ => {}
    }
}

/// Splits a signal id from a backend into the id and its `SVL_LISTEN_*` flags (ABI v2.2).
fn split_signal(signal: svl_id) -> (svl_id, u32) {
    (signal & svelters_abi::SVL_SIGNAL_ID_MASK, signal >> svelters_abi::SVL_SIGNAL_FLAGS_SHIFT)
}

fn deliver(core: &Core, eng: &mut Box<dyn JsEngine>, ev: DispatchEvent) -> Result<(), JsError> {
    match ev {
        DispatchEvent::Signal {
            handle,
            signal,
            args,
            deferred,
            flags,
        } => {
            let r = eng.dispatch(handle, &signal, &args, deferred, flags);
            core.pin_args(&args, false);
            r.map_err(|e| e.at_handle(handle))
        }
        DispatchEvent::Gone { handle, reason } => eng.dispatch_gone(handle, reason).map_err(|e| e.at_handle(handle)),
    }
}

/// Reads, compares and delivers the dirty set as one `__svelters_changed` batch (if not empty).
/// Returns whether a batch was delivered. Runs no jobs.
fn deliver_changes(core: &Core, eng: &mut Box<dyn JsEngine>) -> bool {
    let batch = core.take_changes();
    if batch.is_empty() {
        return false;
    }
    if batch.iter().any(|c| crate::exports::is_app_handle(c.handle)) {
        core.note_store_batch();
    }
    if let Err(e) = eng.dispatch_changed(&batch) {
        core.report(e);
    }
    core.report_all(eng.take_errors());
    true
}

/// Inserts `key` into the dirty set; true for the first insert into an empty set.
fn mark_dirty(st: &mut CoreState, key: (svl_handle, svl_id)) -> bool {
    if !st.dirty_set.insert(key) {
        return false;
    }
    st.dirty.push(key);
    st.dirty.len() == 1
}

/// The compare of REACTIVITY.md §5.2: handles by id, strings by content (UTF-8 and UTF-16 alike),
/// records, lists and maps deeply, f64 by bits (NaN equals NaN). `BOXED` values, callbacks and
/// errors never compare equal (each read mints a new handle).
pub(crate) fn same_value(a: &Value, b: &Value) -> bool {
    use Value as V;
    let f = |x: f64, y: f64| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan());
    match (a, b) {
        (V::Null, V::Null) => true,
        (V::Bool(x), V::Bool(y)) => x == y,
        (V::I64(x), V::I64(y)) => x == y,
        (V::U64(x), V::U64(y)) => x == y,
        (V::I64(x), V::U64(y)) | (V::U64(y), V::I64(x)) => u64::try_from(*x).is_ok_and(|x| x == *y),
        (V::F64(x), V::F64(y)) | (V::DateTime(x), V::DateTime(y)) => f(*x, *y),
        (V::Str(x), V::Str(y)) => x == y,
        (V::Str16(x), V::Str16(y)) => x == y,
        (V::Str(x), V::Str16(y)) | (V::Str16(y), V::Str(x)) => x.encode_utf16().eq(y.iter().copied()),
        (V::Handle(x), V::Handle(y)) => x == y,
        (V::List(x), V::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same_value(a, b)),
        (V::Enum { ty: t1, value: v1 }, V::Enum { ty: t2, value: v2 }) => t1 == t2 && v1 == v2,
        (V::Flags { ty: t1, mask: m1 }, V::Flags { ty: t2, mask: m2 }) => t1 == t2 && m1 == m2,
        (V::Record { ty: t1, fields: f1 }, V::Record { ty: t2, fields: f2 }) => {
            t1 == t2 && f1.len() == f2.len() && f1.iter().zip(f2).all(|(a, b)| same_value(a, b))
        }
        (V::Bytes(x), V::Bytes(y)) => x == y,
        (V::Map(x), V::Map(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|((k1, v1), (k2, v2))| k1 == k2 && same_value(v1, v2))
        }
        _ => false,
    }
}

/// Whether `v` holds a `BOXED` (VALUE) handle anywhere.
fn has_boxed(v: &Value) -> bool {
    match v {
        Value::Boxed { .. } => true,
        Value::List(items) => items.iter().any(has_boxed),
        Value::Record { fields, .. } => fields.iter().any(has_boxed),
        Value::Map(entries) => entries.iter().any(|(_, v)| has_boxed(v)),
        _ => false,
    }
}

/// Releases the `BOXED` handles a read minted and nobody will see.
fn release_boxed_in(backend: &BackendRef<'static>, v: &Value) {
    match v {
        Value::Boxed { handle, .. } => backend.destroy(*handle),
        Value::List(items) => items.iter().for_each(|i| release_boxed_in(backend, i)),
        Value::Record { fields, .. } => fields.iter().for_each(|i| release_boxed_in(backend, i)),
        Value::Map(entries) => entries.iter().for_each(|(_, v)| release_boxed_in(backend, v)),
        _ => {}
    }
}

/// Scripted-mode checkpoint; see [`HostConfig::scripted`].
fn checkpoint_scripted(core: &Core, eng: &mut Box<dyn JsEngine>) {
    const MAX_DRAIN_ROUNDS: usize = 100;
    // A batch of changed properties is delivered like an event: first, and without draining.
    let changed = deliver_changes(core, eng);
    let events = core.take_events();
    if changed || !events.is_empty() {
        for ev in events {
            if let Err(e) = deliver(core, eng, ev) {
                core.report(e);
            }
            core.report_all(eng.take_errors());
        }
        return;
    }

    let drain = |eng: &mut Box<dyn JsEngine>| {
        for _ in 0..MAX_DRAIN_ROUNDS {
            let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
            core.report_all(eng.take_errors());
            if !st.pending {
                return;
            }
        }
        core.report(JsError::new(ErrorKind::Host, "job queue did not drain"));
    };

    let mut fires = 0;
    drain(eng);
    while let Some(deadline) = core.next_timer_deadline() {
        core.advance_clock_to(deadline);
        let now = core.now();
        for (id, seq) in core.timers_take_due(now) {
            if !core.timer_begin_fire(id, seq, now) {
                continue;
            }
            fires += 1;
            if let Err(e) = eng.fire_timer(id) {
                core.report(e);
            }
            drain(eng);
        }
        if fires >= core.cfg.max_virtual_timer_fires {
            core.report(JsError::new(
                ErrorKind::Host,
                "virtual clock: timers did not quiesce (live interval?)",
            ));
            break;
        }
    }
}

/// Loads the provider's entry module under the re-entry guard, then drains jobs. The provider's
/// warnings (compiler warnings, skipped styles) go to the console.
pub(crate) fn load_bundle(
    core: &Core,
    engine: &RefCell<Box<dyn JsEngine>>,
    provider: &Rc<dyn crate::engine::ModuleProvider>,
) -> Result<(), JsError> {
    let Some(_guard) = core.enter() else {
        return Err(JsError::new(
            ErrorKind::Host,
            "load_bundle called while JS is running",
        ));
    };
    core.maps.borrow_mut().begin(provider.map_dir(), &provider.entry());
    let mut eng = engine.borrow_mut();
    let res = eng.load_module(provider.clone());
    for w in provider.take_warnings() {
        core.console("warn", w);
    }
    let mut pending = false;
    if res.is_ok() && !core.cfg.scripted {
        let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
        pending = st.pending;
    }
    core.report_all(eng.take_errors());
    drop(eng);
    core.sync_sched();
    if !core.cfg.scripted && (pending || core.has_events()) {
        core.backend.wake();
    }
    res
}

/// EM1: loads an entry and keeps its namespace (see [`load_bundle`]).
pub(crate) fn load_entry(
    core: &Core,
    engine: &RefCell<Box<dyn JsEngine>>,
    provider: &Rc<dyn crate::engine::ModuleProvider>,
) -> Result<u32, JsError> {
    let Some(_guard) = core.enter() else {
        return Err(JsError::new(ErrorKind::Host, "load_entry called while JS is running"));
    };
    core.maps.borrow_mut().begin(provider.map_dir(), &provider.entry());
    let mut eng = engine.borrow_mut();
    let res = eng.load_entry(provider.clone());
    for w in provider.take_warnings() {
        core.console("warn", w);
    }
    let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
    core.report_all(eng.take_errors());
    drop(eng);
    core.sync_sched();
    if st.pending || core.has_events() {
        core.backend.wake();
    }
    res
}

/// EM1: one call into a mounted root (`__svelters_embed.*`) as its own entry into JS, under the
/// turn budget, followed by a job drain (the microtasks it queued, Svelte's flush among them).
/// An error thrown by the call itself is returned (not reported); errors of the jobs are reported.
pub(crate) fn root_call<R>(
    core: &Core,
    engine: &RefCell<Box<dyn JsEngine>>,
    target: Option<svl_handle>,
    f: impl FnOnce(&mut Box<dyn JsEngine>) -> Result<R, JsError>,
) -> Result<R, JsError> {
    let Some(_guard) = core.enter() else {
        return Err(JsError::new(ErrorKind::Host, "a root call while JS is running"));
    };
    if let Some(t) = target {
        // The mount target (an embedder's slot) was created natively, not by JS: adopt it.
        core.adopt_handles(&Value::Handle(t));
    }
    let mut eng = engine.borrow_mut();
    // Changes and store fields first, as a checkpoint would, so the call sees current state.
    reactive::collect(core);
    deliver_changes(core, &mut eng);
    let res = f(&mut eng);
    let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
    core.report_all(eng.take_errors());
    drop(eng);
    core.process_releases();
    core.sync_sched();
    if st.pending || core.has_events() {
        core.backend.wake();
    }
    res
}

/// Runs queued jobs to exhaustion (no timers, no events). For harnesses that need a final
/// flush after the backend loop returns.
pub(crate) fn drain_jobs(core: &Core, engine: &RefCell<Box<dyn JsEngine>>) {
    let Some(_guard) = core.enter() else { return };
    let mut eng = engine.borrow_mut();
    for _ in 0..100 {
        let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
        core.report_all(eng.take_errors());
        if !st.pending {
            return;
        }
    }
    core.report(JsError::new(ErrorKind::Host, "job queue did not drain"));
}

// ---- reload support (see reload.rs) ----

impl Core {
    /// True while JS is running (an entry guard is held).
    pub(crate) fn js_running(&self) -> bool {
        self.busy.get() > 0
    }

    /// Live handles of the TOPLEVEL lifetime class (windows and dialogs, transient or not), in creation order.
    pub(crate) fn toplevel_handles(&self) -> Vec<svl_handle> {
        let mut v: Vec<svl_handle> = self.st.borrow().toplevels.iter().copied().collect();
        v.sort_unstable();
        v
    }

    /// Live handles that are not inserted under another handle, in creation order.
    pub(crate) fn roots(&self) -> Vec<svl_handle> {
        let st = self.st.borrow();
        let mut v: Vec<svl_handle> = st.handles.keys().filter(|h| !st.tree.contains_key(h)).copied().collect();
        v.sort_unstable();
        v
    }

    /// Destroys `root` and everything inserted below it through the ordinary destroy op,
    /// descendants first (the order the renderer itself uses). Handles the backend already
    /// reported dead are skipped.
    pub(crate) fn destroy_subtree(&self, root: svl_handle) {
        fn collect(children: &HashMap<svl_handle, Vec<svl_handle>>, h: svl_handle, out: &mut Vec<svl_handle>) {
            for c in children.get(&h).into_iter().flatten() {
                collect(children, *c, out);
            }
            out.push(h);
        }
        let mut children: HashMap<svl_handle, Vec<svl_handle>> = HashMap::new();
        {
            let st = self.st.borrow();
            for (c, p) in &st.tree {
                children.entry(*p).or_default().push(*c);
            }
        }
        for v in children.values_mut() {
            v.sort_unstable();
        }
        let mut order = Vec::new();
        collect(&children, root, &mut order);
        for h in order {
            if self.st.borrow().handles.contains_key(&h) {
                let _ = self.op_destroy(h);
            }
        }
    }

    pub(crate) fn clear_events(&self) {
        let mut st = self.st.borrow_mut();
        st.events.clear();
        st.dirty.clear();
        st.dirty_set.clear();
    }

    /// Replaces the timer heap (reload keeps the old runtime's timers aside while the new bundle
    /// loads, and puts them back if that fails). Returns the previous one.
    pub(crate) fn swap_timers(&self, t: Timers) -> Timers {
        let old = std::mem::replace(&mut self.st.borrow_mut().timers, t);
        self.sync_sched();
        old
    }

    pub(crate) fn map_error(&self, e: JsError) -> JsError {
        self.maps.borrow_mut().map_error(e)
    }
}
