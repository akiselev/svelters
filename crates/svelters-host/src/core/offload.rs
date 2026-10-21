//! Offloaded export calls (E1, `docs/notes/egui-rust-exports.md` §6): the op table on the JS thread.
//!
//! - **Start** (`host.call`/`host.callStatic` of an offloaded member): the arguments are encoded and
//!   converted to owned values on the JS thread, the generated start thunk builds the [`Job`], and the
//!   engine makes the Promise (`JsPromises::new_op`, which also watches `options.signal`). A JS-thread
//!   future is polled once at once (eager); pool jobs and tokio tasks are spawned.
//! - **Finish**: jobs push their outcome to the [`Completions`] queue (one backend `wake` per burst).
//!   [`drain`] runs at the start of every checkpoint: it polls woken JS-thread futures, takes the
//!   latest progress values and the finished jobs (at most [`MAX_PER_DRAIN`] or [`DRAIN_BUDGET`]),
//!   converts the results with the object table at hand, and settles the whole batch in one engine
//!   entry (`JsEngine::settle_ops`).
//! - **Cancel**: an abort rejects the Promise in JS at once and fires the op's token here; a job that
//!   is still queued never runs, a JS-thread future is dropped, a tokio task is aborted, and a running
//!   pool job finishes as a "zombie" whose result is dropped. Superseded calls (`#[js(supersede)]`),
//!   calls on a disposed owner and calls of a reloaded runtime are cancelled the same way.
//! - **Concurrency caps** (`#[js(max_in_flight = n)]`): per (member, owner), counted until the job
//!   really ends (zombies included), with a FIFO of waiting calls.

use super::*;
use crate::convert::{self, Arena, Cx, JsPromises, JsReader, JsWriter};
use crate::engine::{OpOutcome, OpSettle};
use crate::exports::offload::{self, Completions, Job, Label, LocalFuture, LocalSettle, LocalWake, OpShared, Outcome};
use crate::exports::{self, CallCx, MemberSchema, schema};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::{Poll, Waker};
use svelters_abi::{NativeError, SVL_D_OPTIONAL, SVL_NULL, SlotDesc};

/// Most completions settled per checkpoint; the rest wait for the next one (a re-wake).
pub(crate) const MAX_PER_DRAIN: usize = 1024;
/// Time budget of one drain (completions only; progress and woken futures always run).
pub(crate) const DRAIN_BUDGET: Duration = Duration::from_millis(4);
/// Debug builds warn once per member when one poll of a JS-thread future takes longer than this.
const LONG_POLL: Duration = Duration::from_millis(20);

/// Counters of the offload path (see [`crate::Host::offload_stats`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct OffloadStats {
    /// Offloaded calls started (jobs spawned or futures polled for the first time).
    pub started: u64,
    /// Promises settled by a checkpoint drain (resolved or rejected).
    pub settled: u64,
    /// Promises settled within the call (a JS-thread future ready at once, a failed start).
    pub settled_eagerly: u64,
    /// Late results dropped: their call was aborted, superseded, disposed or reloaded.
    pub dropped: u64,
    /// Progress values delivered.
    pub progress: u64,
    /// Drains that had work, and the time they took (conversion + one engine entry each).
    pub drains: u64,
    pub drain_ns: u64,
    /// The part of `drain_ns` spent in the engine entry (`JsEngine::settle_ops`: JS values built and
    /// promises settled); the rest is the queue, the op table and the Rust-side conversion.
    pub engine_ns: u64,
}

type Key = (svl_id, &'static str, svl_handle);

enum State {
    Waiting(Job),
    Running,
    Local(Option<LocalFuture>, Waker),
}

struct Op {
    generation: u64,
    owner: Option<svl_handle>,
    kind: svl_id,
    member: &'static MemberSchema,
    key: Key,
    shared: Arc<OpShared>,
    /// The Promise is settled (aborted, superseded, disposed, retired): a result is dropped.
    settled: bool,
    state: State,
}

#[derive(Default)]
struct KeyState {
    running: u32,
    waiting: VecDeque<u64>,
}

pub(crate) enum Done {
    Send(Outcome),
    Local(Result<LocalSettle, NativeError>),
}

pub(crate) struct OpTable {
    pub(crate) queue: Arc<Completions>,
    next: Cell<u64>,
    generation: Cell<u64>,
    ops: RefCell<HashMap<u64, Op>>,
    keys: RefCell<HashMap<Key, KeyState>>,
    /// Host-side rejections (owner disposed) settled by the next drain.
    rejects: RefCell<Vec<(u64, u64, NativeError)>>,
    /// A cap freed a slot: the next drain starts waiting calls.
    kick: Cell<bool>,
    warned: RefCell<HashSet<String>>,
    stats: Cell<OffloadStats>,
}

impl OpTable {
    pub(crate) fn new(queue: Arc<Completions>) -> OpTable {
        OpTable {
            queue,
            next: Cell::new(0),
            generation: Cell::new(1),
            ops: RefCell::default(),
            keys: RefCell::default(),
            rejects: RefCell::default(),
            kick: Cell::new(false),
            warned: RefCell::default(),
            stats: Cell::new(OffloadStats::default()),
        }
    }

    fn stat(&self, f: impl FnOnce(&mut OffloadStats)) {
        let mut s = self.stats.get();
        f(&mut s);
        self.stats.set(s);
    }
}

fn cancelled() -> NativeError {
    exports::host_error("cancelled", "The operation was aborted")
}

fn what(m: &'static MemberSchema, kind: svl_id) -> Label {
    Label(kind, m.name.as_str())
}

static NULL_SLOT: std::sync::LazyLock<SlotDesc> = std::sync::LazyLock::new(|| SlotDesc::new(SVL_NULL));

impl Core {
    pub(crate) fn offload_stats(&self) -> OffloadStats {
        self.ops.stats.get()
    }

    pub(crate) fn ops_in_flight(&self) -> usize {
        self.ops.ops.borrow().len()
    }

    /// `host.call`/`host.callStatic` of an offloaded member: starts the call and returns its Promise.
    /// `args` holds the declared parameters, then optionally `{ signal, onProgress }`.
    pub(crate) fn app_start_op<E>(
        &self,
        e: &E,
        this: Option<svl_handle>,
        kind: svl_id,
        m: &'static MemberSchema,
        prefix: &dyn Fn() -> String,
        args: &<E as JsReader>::V,
    ) -> Result<<E as JsReader>::V, OpError>
    where
        E: JsReader + JsWriter<V = <E as JsReader>::V> + JsPromises,
    {
        let off = m.offload.expect("offloaded member");
        let type_err = |msg: String| OpError { kind: OpKind::Type, msg, native: None };
        let slots = &m.desc.params;
        let mut given = match JsReader::kind(e, args) {
            convert::JsKind::Undefined | convert::JsKind::Null => 0,
            convert::JsKind::Array => JsReader::len(e, args)?,
            _ => return Err(type_err(format!("{}: arguments must be an array", prefix()))),
        };
        let mut options = None;
        if given == slots.len() + 1 {
            let o = JsReader::index(e, args, given - 1)?;
            match JsReader::kind(e, &o) {
                convert::JsKind::Object => options = Some(o),
                convert::JsKind::Undefined | convert::JsKind::Null => {}
                _ => return Err(type_err(format!("{}: the options must be an object ({{ signal, onProgress }})", prefix()))),
            }
            given -= 1;
        }
        let required = slots.iter().filter(|p| !p.has(SVL_D_OPTIONAL)).count();
        if given < required || given > slots.len() {
            let n = if required == slots.len() { required.to_string() } else { format!("{required} to {}", slots.len()) };
            return Err(type_err(format!("{}: expected {n} argument(s), got {given}", prefix())));
        }
        let mut ar = Arena::new();
        let cx = Cx { types: self, utf16: false, callbacks: None, owner: 0 };
        let mut values = Vec::with_capacity(slots.len());
        for (i, slot) in slots.iter().enumerate() {
            if i >= given {
                values.push(Value::Null);
                continue;
            }
            let v = JsReader::index(e, args, i)?;
            let raw = convert::encode(e, &v, slot, &cx, &mut ar)
                .map_err(|c| self.conv_err(&format!("{}({})", prefix(), slot.name.as_deref().unwrap_or("arg")), c))?;
            values.push(unsafe { Value::from_raw(&raw) });
        }
        drop(ar);
        let aborted = match &options {
            Some(o) if JsReader::has_field(e, o, "signal")? => {
                let s = JsReader::field(e, o, "signal")?;
                matches!(JsReader::kind(e, &s), convert::JsKind::Object)
                    && JsReader::has_field(e, &s, "aborted")?
                    && {
                        let a = JsReader::field(e, &s, "aborted")?;
                        matches!(JsReader::kind(e, &a), convert::JsKind::Bool) && JsReader::boolean(e, &a)
                    }
            }
            _ => false,
        };

        let kname = || schema().kind_name(kind).unwrap_or_default();
        match this {
            Some(h) => self.note_op(|| format!("call #a{} {}.{} {}", h & !exports::APP_HANDLE_BIT, kname(), m.name, super::apps::json_args(&values))),
            None => self.note_op(|| format!("static {}.{} {}", kname(), m.name, super::apps::json_args(&values))),
        }
        if let Some(h) = this {
            self.app_check_live(h)?;
        }

        let t = &self.ops;
        let id = t.next.get() + 1;
        t.next.set(id);
        let promise = e.new_op(id, options.as_ref())?;
        let soon = |r: Result<Value, NativeError>, slot: &'static SlotDesc| {
            t.stat(|s| s.settled_eagerly += 1);
            let outcome = match r {
                Ok(v) => OpOutcome::Resolve(v, slot),
                Err(n) => OpOutcome::Reject(n),
            };
            e.settle_op_soon(&OpSettle { id, outcome }).map_err(OpError::from)
        };
        if aborted {
            soon(Err(cancelled()), &m.desc.ret)?;
            return Ok(promise);
        }

        let generation = t.generation.get();
        let shared = OpShared::new(id, generation, t.queue.clone());
        let module = schema().module_of(kind).map(|x| x.name.as_str()).unwrap_or("");
        let ccx = CallCx { objects: &self.apps, schema: schema(), this, module };
        let job = match catch_unwind(AssertUnwindSafe(|| (off.start)(&ccx, values, &shared))) {
            Ok(Ok(j)) => j,
            Ok(Err(n)) => {
                soon(Err(n), &m.desc.ret)?;
                return Ok(promise);
            }
            Err(p) => {
                soon(Err(offload::panic_error(p, &what(m, kind))), &m.desc.ret)?;
                return Ok(promise);
            }
        };

        let key: Key = (kind, m.name.as_str(), this.unwrap_or(0));
        if off.supersede {
            let older: Vec<u64> = t.ops.borrow().iter().filter(|(_, o)| o.key == key && !o.settled).map(|(i, _)| *i).collect();
            for old in older {
                if self.op_cancel(old) {
                    t.stat(|s| s.settled_eagerly += 1);
                    let err = exports::host_error("superseded", format!("{} was called again (#[js(supersede)])", what(m, kind)));
                    e.settle_op_soon(&OpSettle { id: old, outcome: OpOutcome::Reject(err) }).map_err(OpError::from)?;
                }
            }
        }

        t.ops.borrow_mut().insert(id, Op { generation, owner: this, kind, member: m, key, shared, settled: false, state: State::Waiting(job) });
        let run_now = {
            let mut keys = t.keys.borrow_mut();
            let ks = keys.entry(key).or_default();
            if off.max_in_flight == 0 || ks.running < off.max_in_flight {
                ks.running += 1;
                true
            } else {
                ks.waiting.push_back(id);
                false
            }
        };
        if run_now {
            if let Some(s) = self.op_run(id) {
                t.stat(|x| x.settled_eagerly += 1);
                e.settle_op_soon(&s).map_err(OpError::from)?;
            }
        }
        if self.ops.kick.get() {
            self.ops.queue.rewake();
        }
        Ok(promise)
    }

    /// Starts op `id` (its key slot is taken): spawns a pool job or tokio task, or polls a JS-thread
    /// future once. Returns the settlement when the future finished at once.
    fn op_run(&self, id: u64) -> Option<OpSettle> {
        let (job, shared, label) = {
            let mut ops = self.ops.ops.borrow_mut();
            let op = ops.get_mut(&id)?;
            let State::Waiting(_) = &op.state else { return None };
            let State::Waiting(job) = std::mem::replace(&mut op.state, State::Running) else { unreachable!() };
            (job, op.shared.clone(), what(op.member, op.kind))
        };
        self.ops.stat(|s| s.started += 1);
        match job {
            Job::Pool(exec, f) => {
                offload::spawn_pool(exec, f, shared, label);
                None
            }
            Job::Tokio(fut) => {
                offload::spawn_tokio(fut, shared, label);
                None
            }
            Job::Local(fut) => {
                let waker = Waker::from(Arc::new(LocalWake { id, queue: self.ops.queue.clone() }));
                if let Some(op) = self.ops.ops.borrow_mut().get_mut(&id) {
                    op.state = State::Local(Some(fut), waker);
                }
                self.op_poll(id)
            }
        }
    }

    /// Polls the JS-thread future of op `id` (eager start, or its waker fired).
    fn op_poll(&self, id: u64) -> Option<OpSettle> {
        let (mut fut, waker, generation, label) = {
            let mut ops = self.ops.ops.borrow_mut();
            let op = ops.get_mut(&id)?;
            let label = what(op.member, op.kind);
            match &mut op.state {
                State::Local(f, w) => (f.take()?, w.clone(), op.generation, label),
                _ => return None,
            }
        };
        let t0 = Instant::now();
        let r = offload::poll_local(&mut fut, &waker, label);
        let took = t0.elapsed();
        if cfg!(debug_assertions) && took > LONG_POLL && self.ops.warned.borrow_mut().insert(label.to_string()) {
            self.console(
                "warn",
                format!(
                    "svelters: `{label}` is an async fn on the JS thread and blocked it for {} ms in one poll; \
                     move blocking work to #[js(worker)], #[js(blocking)], svelters::exports::worker(...) or #[js(tokio)]",
                    took.as_millis()
                ),
            );
        }
        match r {
            Poll::Ready(done) => {
                drop(fut);
                self.op_complete(id, generation, Done::Local(done))
            }
            Poll::Pending => {
                let mut ops = self.ops.ops.borrow_mut();
                match ops.get_mut(&id) {
                    Some(Op { state: State::Local(slot, _), .. }) => *slot = Some(fut),
                    _ => {
                        drop(ops);
                        drop(fut);
                    }
                }
                None
            }
        }
    }

    /// Op `id` ended: frees its slot and converts the result, unless the call was settled already
    /// (then the result is dropped here, on the JS thread).
    fn op_complete(&self, id: u64, generation: u64, done: Done) -> Option<OpSettle> {
        let op = self.ops.ops.borrow_mut().remove(&id);
        let Some(op) = op else {
            drop(done);
            self.ops.stat(|s| s.dropped += 1);
            return None;
        };
        self.release_slot(&op.key);
        // The generation tag: a result from a runtime that was reloaded since is never settled.
        if op.settled || op.generation != generation || generation != self.ops.generation.get() {
            drop(done);
            drop(op);
            self.ops.stat(|s| s.dropped += 1);
            return None;
        }
        let module = schema().module_of(op.kind).map(|x| x.name.as_str()).unwrap_or("");
        let cx = CallCx { objects: &self.apps, schema: schema(), this: op.owner, module };
        let r = match done {
            Done::Send(Ok(f)) => catch_unwind(AssertUnwindSafe(|| f(&cx))).unwrap_or_else(|p| Err(offload::panic_error(p, &what(op.member, op.kind)))),
            Done::Local(Ok(f)) => catch_unwind(AssertUnwindSafe(|| f(&cx))).unwrap_or_else(|p| Err(offload::panic_error(p, &what(op.member, op.kind)))),
            Done::Send(Err(e)) | Done::Local(Err(e)) => Err(e),
        };
        let outcome = match r {
            Ok(v) => OpOutcome::Resolve(v, &op.member.desc.ret),
            Err(n) => OpOutcome::Reject(n),
        };
        Some(OpSettle { id, outcome })
    }

    /// The latest progress value of op `id`, if its call is still pending.
    fn op_progress(&self, id: u64) -> Option<OpSettle> {
        let (f, owner, kind, member) = {
            let ops = self.ops.ops.borrow();
            let op = ops.get(&id).filter(|o| !o.settled)?;
            (op.shared.take_progress()?, op.owner, op.kind, op.member)
        };
        let module = schema().module_of(kind).map(|x| x.name.as_str()).unwrap_or("");
        let cx = CallCx { objects: &self.apps, schema: schema(), this: owner, module };
        let v = catch_unwind(AssertUnwindSafe(|| f(&cx))).ok()?.ok()?;
        let slot: &'static SlotDesc = member.sig.progress.as_ref().map_or(&*NULL_SLOT, |t| &t.slot);
        self.ops.stat(|s| s.progress += 1);
        Some(OpSettle { id, outcome: OpOutcome::Progress(v, slot) })
    }

    /// A running call of `key` ended: frees its slot; waiting calls start at the next drain.
    fn release_slot(&self, key: &Key) {
        let mut keys = self.ops.keys.borrow_mut();
        if let Some(ks) = keys.get_mut(key) {
            ks.running = ks.running.saturating_sub(1);
            if !ks.waiting.is_empty() {
                self.ops.kick.set(true);
            } else if ks.running == 0 {
                keys.remove(key);
            }
        }
    }

    /// Starts waiting calls whose cap has room.
    fn op_start_waiting(&self, out: &mut Vec<OpSettle>) {
        loop {
            let next = {
                let ops = self.ops.ops.borrow();
                let mut keys = self.ops.keys.borrow_mut();
                let mut found = None;
                for ks in keys.values_mut() {
                    while let Some(&id) = ks.waiting.front() {
                        let Some(op) = ops.get(&id) else {
                            ks.waiting.pop_front();
                            continue;
                        };
                        let max = op.member.offload.map_or(0, |o| o.max_in_flight);
                        if max == 0 || ks.running < max {
                            ks.waiting.pop_front();
                            ks.running += 1;
                            found = Some(id);
                        }
                        break;
                    }
                    if found.is_some() {
                        break;
                    }
                }
                keys.retain(|_, ks| ks.running > 0 || !ks.waiting.is_empty());
                found
            };
            let Some(id) = next else { return };
            if let Some(s) = self.op_run(id) {
                self.ops.stat(|x| x.settled += 1);
                out.push(s);
            }
        }
    }

    /// Cancels op `id`: fires its token, drops a waiting job or a JS-thread future, keeps a running
    /// pool job as a zombie until it ends. True if the call was not settled yet (the caller settles
    /// its Promise, unless JS did already).
    pub(crate) fn op_cancel(&self, id: u64) -> bool {
        let (shared, removed) = {
            let mut ops = self.ops.ops.borrow_mut();
            let Some(op) = ops.get_mut(&id) else { return false };
            if op.settled {
                return false;
            }
            op.settled = true;
            let shared = op.shared.clone();
            let removed = if matches!(op.state, State::Running) { None } else { ops.remove(&id) };
            (shared, removed)
        };
        shared.cancel();
        if let Some(op) = removed {
            match op.state {
                State::Waiting(job) => {
                    if let Some(ks) = self.ops.keys.borrow_mut().get_mut(&op.key) {
                        ks.waiting.retain(|x| *x != id);
                    }
                    drop(job);
                }
                State::Local(fut, _) => {
                    drop(fut);
                    self.release_slot(&op.key);
                    self.ops.stat(|s| s.dropped += 1);
                }
                State::Running => {}
            }
        }
        if self.ops.kick.get() {
            self.ops.queue.rewake();
        }
        true
    }

    /// `native.cancelOp(id)`: the call's `signal` aborted; JS rejected its Promise already.
    pub(crate) fn op_cancel_from_js(&self, id: u64) {
        self.op_cancel(id);
    }

    /// The owner of in-flight calls was disposed: they reject with `disposed` at the next checkpoint.
    pub(crate) fn ops_owner_gone(&self, h: svl_handle) {
        let ids: Vec<(u64, u64)> =
            self.ops.ops.borrow().iter().filter(|(_, o)| o.owner == Some(h) && !o.settled).map(|(i, o)| (*i, o.generation)).collect();
        if ids.is_empty() {
            return;
        }
        for (id, generation) in ids {
            if self.op_cancel(id) {
                let msg = format!("app object #a{} was disposed while a call was in flight", h & !exports::APP_HANDLE_BIT);
                self.ops.rejects.borrow_mut().push((id, generation, exports::host_error("disposed", msg)));
            }
        }
        self.ops.queue.rewake();
    }

    /// A reload starts: calls made from now on belong to the new runtime's generation.
    pub(crate) fn ops_begin_generation(&self) -> u64 {
        let old = self.ops.generation.get();
        self.ops.generation.set(old + 1);
        old
    }

    /// Retires the calls of a runtime that is gone (cancelled, results dropped): the old one after a
    /// successful reload (`keep` = the new generation), or the failed one (then `restore` it).
    pub(crate) fn ops_end_generation(&self, old: u64, ok: bool) {
        let new = old + 1;
        let ids: Vec<u64> = self
            .ops
            .ops
            .borrow()
            .iter()
            .filter(|(_, o)| !o.settled && if ok { o.generation != new } else { o.generation == new })
            .map(|(i, _)| *i)
            .collect();
        for id in ids {
            self.op_cancel(id);
        }
        if !ok {
            self.ops.generation.set(old);
        }
        // Pending host rejections of retired calls have no runtime to go to.
        let current = self.ops.generation.get();
        self.ops.rejects.borrow_mut().retain(|(_, g, _)| *g == current);
    }

    /// The host is going away: no more wakes, every token fired, every job and future dropped.
    pub(crate) fn ops_shutdown(&self) {
        self.ops.queue.detach();
        let ids: Vec<u64> = self.ops.ops.borrow().keys().copied().collect();
        for id in ids {
            self.op_cancel(id);
        }
        let ops: Vec<Op> = self.ops.ops.borrow_mut().drain().map(|(_, o)| o).collect();
        drop(ops);
        self.ops.keys.borrow_mut().clear();
        self.ops.rejects.borrow_mut().clear();
    }

    fn ops_have_work(&self) -> bool {
        self.ops.queue.pending() || self.ops.kick.get() || !self.ops.rejects.borrow().is_empty()
    }
}

/// Checkpoint step 1 (§6.2): woken JS-thread futures, progress, finished jobs, host rejections, all
/// settled in one engine entry. Runs no jobs: the reactions run in the checkpoint's job drain.
pub(crate) fn drain(core: &Core, eng: &mut Box<dyn JsEngine>) {
    if !core.ops_have_work() {
        return;
    }
    let t0 = Instant::now();
    let t = &core.ops;
    let mut out: Vec<OpSettle> = Vec::new();
    let mut settled = 0u64;
    for (id, _, e) in std::mem::take(&mut *t.rejects.borrow_mut()) {
        out.push(OpSettle { id, outcome: OpOutcome::Reject(e) });
        settled += 1;
    }
    if t.kick.replace(false) {
        core.op_start_waiting(&mut out);
    }
    let (done, progress, ready, more) = t.queue.take(MAX_PER_DRAIN);
    for id in ready {
        if let Some(s) = core.op_poll(id) {
            out.push(s);
            settled += 1;
        }
    }
    for id in progress {
        if let Some(s) = core.op_progress(id) {
            out.push(s);
        }
    }
    let mut rest = done.into_iter();
    for (id, generation, outcome) in rest.by_ref() {
        if let Some(s) = core.op_complete(id, generation, Done::Send(outcome)) {
            out.push(s);
            settled += 1;
        }
        if t0.elapsed() > DRAIN_BUDGET {
            break;
        }
    }
    let rest: Vec<_> = rest.collect();
    let leftover = !rest.is_empty();
    t.queue.requeue(rest);
    if more && !leftover {
        t.queue.rewake();
    }
    if t.kick.replace(false) {
        core.op_start_waiting(&mut out);
    }
    let mut engine_ns = 0;
    if !out.is_empty() {
        let t1 = Instant::now();
        if let Err(e) = eng.settle_ops(&out) {
            core.report(e);
        }
        engine_ns = t1.elapsed().as_nanos() as u64;
        core.report_all(eng.take_errors());
    }
    let ns = t0.elapsed().as_nanos() as u64;
    t.stat(|s| {
        s.settled += settled;
        s.drains += 1;
        s.drain_ns += ns;
        s.engine_ns += engine_ns;
    });
}
