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

use crate::engine::{ErrorKind, JsEngine, JsError};
use crate::sourcemap::SourceMaps;
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use svelters_abi::{BackendRef, NameKind, Value, WakeHandle, svl_handle, svl_id};

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

struct Event {
    handle: svl_handle,
    signal: svl_id,
    args: Vec<Value>,
}

/// An event with its signal name resolved, ready for `__svelters_dispatch`.
pub(crate) struct DispatchEvent {
    pub handle: svl_handle,
    pub signal: String,
    pub args: Vec<Value>,
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

struct SchedState {
    deadline: Option<Instant>,
    gen_: u64,
    fired_gen: u64,
    shutdown: bool,
}

/// Timer thread state. The thread sleeps until the earliest timer deadline and calls the
/// backend's thread-safe `wake` once per deadline update, so the host never polls.
pub(crate) struct Sched {
    m: Mutex<SchedState>,
    cv: Condvar,
}

impl Sched {
    pub(crate) fn new() -> Arc<Sched> {
        Arc::new(Sched {
            m: Mutex::new(SchedState {
                deadline: None,
                gen_: 0,
                fired_gen: 0,
                shutdown: false,
            }),
            cv: Condvar::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, SchedState> {
        self.m.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, deadline: Option<Instant>, force: bool) {
        let mut g = self.lock();
        if force || g.deadline != deadline {
            g.deadline = deadline;
            g.gen_ += 1;
            self.cv.notify_all();
        }
    }

    pub(crate) fn shutdown(&self) {
        self.lock().shutdown = true;
        self.cv.notify_all();
    }

    pub(crate) fn run(&self, wake: WakeHandle) {
        let mut g = self.lock();
        loop {
            if g.shutdown {
                return;
            }
            match g.deadline {
                Some(d) if g.fired_gen != g.gen_ => {
                    let now = Instant::now();
                    if now >= d {
                        g.fired_gen = g.gen_;
                        drop(g);
                        wake.wake();
                        g = self.lock();
                    } else {
                        g = self
                            .cv
                            .wait_timeout(g, d - now)
                            .unwrap_or_else(|e| e.into_inner())
                            .0;
                    }
                }
                _ => g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }
}

#[derive(Default)]
struct CoreState {
    /// handle → kind id
    handles: HashMap<svl_handle, svl_id>,
    /// child -> parent for handles currently inserted somewhere; a live handle that is not a key
    /// is a root (a toplevel, or not attached yet). Used by reload teardown.
    tree: HashMap<svl_handle, svl_handle>,
    kinds: HashMap<String, svl_id>,
    names: HashMap<(u32, svl_id, String), svl_id>,
    /// (kind id, signal id) → name, filled by `listen`
    signal_names: HashMap<(svl_id, svl_id), String>,
    events: VecDeque<Event>,
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
    pub(crate) sched: Arc<Sched>,
    pub(crate) interrupt: Arc<AtomicBool>,
    epoch: Instant,
    vnow: Cell<Duration>,
    timing: Cell<Timing>,
    checkpoints: Cell<u64>,
    maps: RefCell<SourceMaps>,
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
    pub(crate) fn new(backend: BackendRef<'static>, cfg: HostConfig, sched: Arc<Sched>) -> Core {
        Core {
            backend,
            cfg,
            st: RefCell::new(CoreState::default()),
            busy: Cell::new(0),
            deferred: Cell::new(false),
            sched,
            interrupt: Arc::new(AtomicBool::new(false)),
            epoch: Instant::now(),
            vnow: Cell::new(Duration::ZERO),
            timing: Cell::new(Timing::default()),
            checkpoints: Cell::new(0),
            maps: RefCell::default(),
        }
    }

    pub(crate) fn take_timing(&self) -> Timing {
        self.timing.replace(Timing::default())
    }

    pub(crate) fn checkpoint_count(&self) -> u64 {
        self.checkpoints.get()
    }

    /// Runs one host op, adding its duration to the timing counters when enabled.
    fn timed<R>(&self, f: impl FnOnce() -> R) -> R {
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
        if self.cfg.echo_console {
            eprintln!("[{level}] {text}");
        }
        self.st.borrow_mut().console.push(ConsoleLine {
            level: level.to_string(),
            text,
        });
    }

    pub(crate) fn take_errors(&self) -> Vec<JsError> {
        std::mem::take(&mut self.st.borrow_mut().errors)
    }

    pub(crate) fn take_console(&self) -> Vec<ConsoleLine> {
        std::mem::take(&mut self.st.borrow_mut().console)
    }

    // ---- name resolution ----

    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> Result<svl_id, String> {
        let key = (what as u32, kind, name.to_string());
        if let Some(id) = self.st.borrow().names.get(&key) {
            return Ok(*id);
        }
        let id = self.backend.resolve(what, kind, name);
        if id == 0 {
            return Err(format!("unknown {what:?} '{name}'"));
        }
        self.st.borrow_mut().names.insert(key, id);
        Ok(id)
    }

    fn kind_of(&self, h: svl_handle) -> Result<svl_id, String> {
        self.st
            .borrow()
            .handles
            .get(&h)
            .copied()
            .ok_or_else(|| format!("stale or foreign handle {h}"))
    }

    // ---- JS host ops (called by the engine's `__svelters_host`) ----

    pub(crate) fn op_create(&self, kind: &str) -> Result<svl_handle, String> {
        self.timed(|| self.op_create_raw(kind))
    }

    pub(crate) fn op_insert(
        &self,
        parent: svl_handle,
        child: svl_handle,
        before: Option<svl_handle>,
        slot: Option<&str>,
    ) -> Result<(), String> {
        self.timed(|| self.op_insert_raw(parent, child, before, slot))
    }

    pub(crate) fn op_remove(&self, parent: svl_handle, child: svl_handle) -> Result<(), String> {
        self.timed(|| self.op_remove_raw(parent, child))
    }

    pub(crate) fn op_destroy(&self, h: svl_handle) -> Result<(), String> {
        self.timed(|| self.op_destroy_raw(h))
    }

    pub(crate) fn op_set_prop(&self, h: svl_handle, prop: &str, v: &Value) -> Result<(), String> {
        self.timed(|| self.op_set_prop_raw(h, prop, v))
    }

    pub(crate) fn op_get_prop(&self, h: svl_handle, prop: &str) -> Result<Value, String> {
        self.timed(|| self.op_get_prop_raw(h, prop))
    }

    pub(crate) fn op_call(
        &self,
        h: svl_handle,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        self.timed(|| self.op_call_raw(h, method, args))
    }

    pub(crate) fn op_listen(&self, h: svl_handle, signal: &str, on: bool) -> Result<(), String> {
        self.timed(|| self.op_listen_raw(h, signal, on))
    }


    fn op_create_raw(&self, kind: &str) -> Result<svl_handle, String> {
        let cached = self.st.borrow().kinds.get(kind).copied();
        let kid = match cached {
            Some(k) => k,
            None => {
                let k = self.backend.resolve(NameKind::Kind, 0, kind);
                if k == 0 {
                    return Err(format!("unknown kind '{kind}'"));
                }
                self.st.borrow_mut().kinds.insert(kind.to_string(), k);
                k
            }
        };
        let h = self.backend.create(kid).map_err(|e| e.message)?;
        self.st.borrow_mut().handles.insert(h, kid);
        Ok(h)
    }

    fn op_insert_raw(
        &self,
        parent: svl_handle,
        child: svl_handle,
        before: Option<svl_handle>,
        slot: Option<&str>,
    ) -> Result<(), String> {
        let pk = self.kind_of(parent)?;
        self.kind_of(child)?;
        if let Some(b) = before {
            self.kind_of(b)?;
        }
        let slot = match slot {
            Some(s) => self.resolve(NameKind::Slot, pk, s)?,
            None => 0,
        };
        self.backend
            .insert(parent, child, before, slot)
            .map_err(|e| e.message)?;
        self.st.borrow_mut().tree.insert(child, parent);
        Ok(())
    }

    fn op_remove_raw(&self, parent: svl_handle, child: svl_handle) -> Result<(), String> {
        self.kind_of(parent)?;
        self.kind_of(child)?;
        self.backend.remove(parent, child).map_err(|e| e.message)?;
        self.st.borrow_mut().tree.remove(&child);
        Ok(())
    }

    fn op_destroy_raw(&self, h: svl_handle) -> Result<(), String> {
        self.kind_of(h)?;
        {
            let mut st = self.st.borrow_mut();
            st.handles.remove(&h);
            st.tree.remove(&h);
        }
        self.backend.destroy(h);
        Ok(())
    }

    fn op_set_prop_raw(&self, h: svl_handle, prop: &str, v: &Value) -> Result<(), String> {
        let k = self.kind_of(h)?;
        let p = self.resolve(NameKind::Prop, k, prop)?;
        self.backend.set_prop(h, p, v).map_err(|e| e.message)
    }

    fn op_get_prop_raw(&self, h: svl_handle, prop: &str) -> Result<Value, String> {
        let k = self.kind_of(h)?;
        let p = self.resolve(NameKind::Prop, k, prop)?;
        self.backend.get_prop(h, p).map_err(|e| e.message)
    }

    fn op_call_raw(
        &self,
        h: svl_handle,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        let k = self.kind_of(h)?;
        let m = self.resolve(NameKind::Method, k, method)?;
        self.backend.call(h, m, args).map_err(|e| e.message)
    }

    fn op_listen_raw(&self, h: svl_handle, signal: &str, on: bool) -> Result<(), String> {
        let k = self.kind_of(h)?;
        let s = self.resolve(NameKind::Signal, k, signal)?;
        self.st
            .borrow_mut()
            .signal_names
            .insert((k, s), signal.to_string());
        self.backend.listen(h, s, on).map_err(|e| e.message)
    }

    // ---- events ----

    pub(crate) fn push_event(&self, handle: svl_handle, signal: svl_id, args: Vec<Value>) {
        self.st.borrow_mut().events.push_back(Event {
            handle,
            signal,
            args,
        });
    }

    pub(crate) fn has_events(&self) -> bool {
        !self.st.borrow().events.is_empty()
    }

    /// Drains the queue, resolving signal ids to names. Events that cannot be mapped are
    /// reported and dropped.
    pub(crate) fn take_events(&self) -> Vec<DispatchEvent> {
        let events: Vec<Event> = self.st.borrow_mut().events.drain(..).collect();
        let mut out = Vec::new();
        for e in events {
            let name = {
                let st = self.st.borrow();
                st.handles
                    .get(&e.handle)
                    .and_then(|k| st.signal_names.get(&(*k, e.signal)))
                    .cloned()
            };
            match name {
                Some(signal) => out.push(DispatchEvent {
                    handle: e.handle,
                    signal,
                    args: e.args,
                }),
                None => self.report(JsError::new(
                    ErrorKind::Host,
                    format!(
                        "dropped event for handle {} signal id {}: no listener or stale handle",
                        e.handle, e.signal
                    ),
                )),
            }
        }
        out
    }

    /// The backend reports that a native object went away.
    pub(crate) fn forget_handle(&self, h: svl_handle) {
        let mut st = self.st.borrow_mut();
        st.handles.remove(&h);
        st.tree.remove(&h);
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

    fn arm_sched(&self, deadline: Option<Instant>, force: bool) {
        if !self.cfg.scripted {
            self.sched.set(deadline, force);
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
/// 1. deliver queued events to JS, draining jobs after each;
/// 2. fire due timers, draining jobs after each;
/// 3. drain remaining jobs, up to `max_jobs_per_checkpoint` in total;
/// 4. `wake` if jobs or events remain, and re-arm the timer thread.
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
    if core.cfg.scripted {
        checkpoint_scripted(core, &mut eng);
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

    for ev in core.take_events() {
        if let Err(e) = eng.dispatch(ev.handle, &ev.signal, &ev.args) {
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

    core.sync_sched();
    if pending || core.has_events() {
        core.backend.wake();
    }
}

/// Scripted-mode checkpoint; see [`HostConfig::scripted`].
fn checkpoint_scripted(core: &Core, eng: &mut Box<dyn JsEngine>) {
    const MAX_DRAIN_ROUNDS: usize = 100;
    let events = core.take_events();
    if !events.is_empty() {
        for ev in events {
            if let Err(e) = eng.dispatch(ev.handle, &ev.signal, &ev.args) {
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

/// Loads the entry module under the re-entry guard, then drains jobs.
pub(crate) fn load_bundle(
    core: &Core,
    engine: &RefCell<Box<dyn JsEngine>>,
    entry: &std::path::Path,
) -> Result<(), JsError> {
    let Some(_guard) = core.enter() else {
        return Err(JsError::new(
            ErrorKind::Host,
            "load_bundle called while JS is running",
        ));
    };
    core.maps.borrow_mut().set_entry(entry);
    let mut eng = engine.borrow_mut();
    let res = eng.load_module(entry);
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
                let _ = self.op_destroy_raw(h);
            }
        }
    }

    pub(crate) fn clear_events(&self) {
        self.st.borrow_mut().events.clear();
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
