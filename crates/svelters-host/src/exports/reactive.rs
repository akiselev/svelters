//! Reactive exports (E3, `docs/notes/egui-rust-exports.md` §6.4, §6.6): stores and events.
//!
//! - **Stores** ([`Store<S>`], [`StoreStatic<S>`], `#[svelters::store]`): Rust state that Svelte reads like
//!   `$state`. Any thread writes; a write sets one dirty bit per field (decision 2026-10-22: the
//!   granularity is the field, see note §11 Q7) in every host that observes the store, and wakes that
//!   host once per burst. At the next checkpoint the host folds the dirty fields into R1's dirty set, so
//!   store changes, native property changes and events reach JS in one `__svelters_changed` batch with
//!   one job drain. A field is read (cloned under the read lock) only when it is delivered, and
//!   compare-with-last drops unchanged values, so [`Store::update`] may mark every field.
//!   `#[store(proportional)]` fields (a big `Vec`, a log `String`) are delivered as invalidations: JS
//!   re-reads them when it needs them. Store fields are `exact` by construction: every write goes through
//!   the store, so nothing changes them unannounced.
//! - **Events** ([`Emitter<E>`], `#[derive(svelters::Events)]`): any thread emits; events are queued in
//!   order per object and host, and delivered at the checkpoint as signal events to `obj.on(name, fn)`
//!   listeners, after the change batch. `#[event(latest)]` keeps only the last occurrence per checkpoint.
//!   An event nobody listens to is dropped at `emit`, before it is cloned or converted.
//!
//! Writers never call into the host: they set bits or queue payloads and wake the host's backend at most
//! once per burst ([`WakeHandle`] is the one thread-safe backend entry point).

use super::{AnyObj, CallCx, ExportObject, IntoJs, JsType, NativeError, SharedObject, TypeBuilder, TypeRef, Value};
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};
use svelters_abi::WakeHandle;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- the per-host hub writers post to ----

/// A queued event payload, converted on the JS thread (objects need the host's table).
pub(crate) type Payload = Box<dyn FnOnce(&CallCx<'_>) -> Result<Option<Value>, NativeError> + Send>;

pub(crate) enum HubEvent {
    /// An ordinary event: delivered in order.
    Event { handle: u64, index: usize, payload: Payload },
    /// A `#[event(latest)]` event: its latest payload waits in the subscription's slot.
    Latest { handle: u64, index: usize, sub: Arc<EmitSub> },
}

#[derive(Default)]
struct HubQueue {
    /// Store subscriptions (by handle) with dirty bits since the last take.
    stores: Vec<u64>,
    events: VecDeque<HubEvent>,
}

/// One host's inbox for store changes and events (E3): a mutex'd queue plus an atomic "wake pending",
/// so a burst of writes from any number of threads costs one backend `wake`.
pub(crate) struct Hub {
    q: Mutex<HubQueue>,
    wake_pending: AtomicBool,
    /// Cleared when the host goes away: later writes reach nobody (and wake nobody).
    waker: Mutex<Option<WakeHandle>>,
}

impl Hub {
    pub(crate) fn new(waker: WakeHandle) -> Arc<Hub> {
        Arc::new(Hub { q: Mutex::new(HubQueue::default()), wake_pending: AtomicBool::new(false), waker: Mutex::new(Some(waker)) })
    }

    fn wake(&self) {
        if !self.wake_pending.swap(true, Ordering::AcqRel) {
            if let Some(w) = &*lock(&self.waker) {
                w.wake();
            }
        }
    }

    fn push_store(&self, handle: u64) {
        lock(&self.q).stores.push(handle);
        self.wake();
    }

    fn push_event(&self, e: HubEvent) {
        lock(&self.q).events.push_back(e);
        self.wake();
    }

    /// Something to collect (cheap check at checkpoint start).
    pub(crate) fn pending(&self) -> bool {
        self.wake_pending.load(Ordering::Acquire)
    }

    /// Takes every store notification and event queued so far.
    pub(crate) fn take(&self) -> (Vec<u64>, VecDeque<HubEvent>) {
        self.wake_pending.store(false, Ordering::Release);
        let mut q = lock(&self.q);
        (std::mem::take(&mut q.stores), std::mem::take(&mut q.events))
    }

    /// The host is going away: no more wakes, queued payloads dropped.
    pub(crate) fn detach(&self) {
        *lock(&self.waker) = None;
        let q = std::mem::take(&mut *lock(&self.q));
        drop(q);
    }
}

// ---- stores ----

/// One field of a `#[svelters::store]` struct, as the macro describes it.
#[derive(Clone, Copy, Debug)]
pub struct StoreField {
    /// The JS name (camelCase, or `#[js(rename)]`).
    pub name: &'static str,
    pub rust: &'static str,
    /// `#[store(writable)]`: JS may assign it (`setProp` → `on_write` → read back).
    pub writable: bool,
    /// `#[store(proportional)]`: delivered as an invalidation, never read or compared by the host.
    pub proportional: bool,
    pub docs: &'static str,
}

/// The state struct of a store, implemented by `#[svelters::store(module = "m")]`. The generated
/// `<Name>Field` enum names its fields (for [`Store::write`] and [`StoreHooks::on_write`]).
pub trait StoreState: Send + Sync + Sized + 'static {
    const MODULE: &'static str;
    const NAME: &'static str;
    const DOCS: &'static str;
    const FIELDS: &'static [StoreField];
    /// The generated field enum.
    type Field: Copy + std::fmt::Debug + 'static;
    #[doc(hidden)]
    fn __index(f: Self::Field) -> usize;
    #[doc(hidden)]
    fn __field(i: usize) -> Self::Field;
    /// Clones field `i` into a JS value.
    #[doc(hidden)]
    fn __read(&self, i: usize, cx: &CallCx<'_>) -> Result<Value, NativeError>;
    /// Assigns field `i` from JS and runs [`StoreHooks::on_write`] if the type has hooks (restoring the old
    /// value when it fails). `Ok(true)`: hooks ran (they may have changed any field).
    #[doc(hidden)]
    fn __write(&mut self, i: usize, v: Value, cx: &CallCx<'_>) -> Result<bool, NativeError>;
    /// The typings of field `i`.
    #[doc(hidden)]
    fn __describe(i: usize, b: &mut TypeBuilder) -> TypeRef;
}

/// Validation of JS writes to `#[store(writable)]` fields. Optional: implement it for the state struct
/// and the generated setters call it after assigning the new value.
///
/// ```ignore
/// impl StoreHooks for Telemetry {
///     fn on_write(&mut self, field: TelemetryField) -> Result<(), NativeError> {
///         if let TelemetryField::Speed = field { self.speed = self.speed.clamp(0.1, 10.0); }  // normalize
///         Ok(())                                                                              // or Err: rejected, old value kept
///     }
/// }
/// ```
///
/// A normalized value (here, the clamp) is delivered to JS once (R1's set path: the host reads the field
/// back and compares it with what JS wrote); the echo of an accepted write is dropped. An `Err` throws in
/// JS (`stats.speed = v` throws the `NativeError`) and keeps the old value. The hook may change other
/// fields too: after a hooked write every field is marked, and the compare drops what did not change.
pub trait StoreHooks: StoreState {
    fn on_write(&mut self, field: Self::Field) -> Result<(), NativeError>;
}

#[doc(hidden)]
pub struct HookProbe<S>(PhantomData<S>);

impl<S> HookProbe<S> {
    pub fn new() -> Self {
        HookProbe(PhantomData)
    }
}

impl<S> Default for HookProbe<S> {
    fn default() -> Self {
        HookProbe::new()
    }
}

/// Autoref specialization: `(&HookProbe::<S>::new()).__svl_on_write(...)` picks [`StoreHooks`] when the
/// state implements it, [`HookViaDefault`] (no hooks) otherwise. Used by the generated `__write`.
#[doc(hidden)]
pub trait HookViaHooks<S: StoreState> {
    fn __svl_on_write(&self, s: &mut S, f: S::Field) -> Option<Result<(), NativeError>>;
}

impl<S: StoreHooks> HookViaHooks<S> for HookProbe<S> {
    fn __svl_on_write(&self, s: &mut S, f: S::Field) -> Option<Result<(), NativeError>> {
        Some(s.on_write(f))
    }
}

#[doc(hidden)]
pub trait HookViaDefault<S: StoreState> {
    fn __svl_on_write(&self, s: &mut S, f: S::Field) -> Option<Result<(), NativeError>>;
}

impl<S: StoreState> HookViaDefault<S> for &HookProbe<S> {
    fn __svl_on_write(&self, _: &mut S, _: S::Field) -> Option<Result<(), NativeError>> {
        None
    }
}

/// One host's subscription to one store handle: the dirty bits since the host last looked.
#[doc(hidden)]
pub struct StoreSub {
    handle: u64,
    mask: Box<[AtomicU64]>,
    full: Box<[u64]>,
    pending: AtomicBool,
    hub: Arc<Hub>,
}

impl StoreSub {
    pub(crate) fn new(handle: u64, fields: usize, hub: Arc<Hub>) -> Arc<StoreSub> {
        let words = fields.div_ceil(64).max(1);
        let full: Box<[u64]> = (0..words)
            .map(|w| {
                let n = fields.saturating_sub(w * 64).min(64);
                if n == 64 { u64::MAX } else { (1u64 << n) - 1 }
            })
            .collect();
        Arc::new(StoreSub { handle, mask: (0..words).map(|_| AtomicU64::new(0)).collect(), full, pending: AtomicBool::new(false), hub })
    }

    fn notify(&self) {
        if !self.pending.swap(true, Ordering::AcqRel) {
            self.hub.push_store(self.handle);
        }
    }

    fn mark(&self, i: usize) {
        if let Some(w) = self.mask.get(i / 64) {
            w.fetch_or(1 << (i % 64), Ordering::AcqRel);
            self.notify();
        }
    }

    fn mark_all(&self) {
        for (w, f) in self.mask.iter().zip(self.full.iter()) {
            w.fetch_or(*f, Ordering::AcqRel);
        }
        self.notify();
    }

    /// The dirty field indices since the last take (JS thread).
    pub(crate) fn take(&self) -> Vec<usize> {
        self.pending.store(false, Ordering::Release);
        let mut out = Vec::new();
        for (wi, w) in self.mask.iter().enumerate() {
            let mut bits = w.swap(0, Ordering::AcqRel);
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                out.push(wi * 64 + b);
                bits &= bits - 1;
            }
        }
        out
    }
}

/// The shared cell behind a [`Store`]: the state under an `RwLock`, and the hosts that observe it.
pub struct StoreCell<S> {
    state: RwLock<S>,
    subs: RwLock<Vec<Weak<StoreSub>>>,
}

impl<S> StoreCell<S> {
    fn read(&self) -> RwLockReadGuard<'_, S> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, S> {
        self.state.write().unwrap_or_else(|e| e.into_inner())
    }

    fn each_sub(&self, f: impl Fn(&StoreSub)) {
        let subs = self.subs.read().unwrap_or_else(|e| e.into_inner());
        for s in subs.iter() {
            if let Some(s) = s.upgrade() {
                f(&s);
            }
        }
    }

    pub(crate) fn subscribe(&self, sub: &Arc<StoreSub>) {
        let mut subs = self.subs.write().unwrap_or_else(|e| e.into_inner());
        subs.retain(|w| w.strong_count() > 0);
        subs.push(Arc::downgrade(sub));
    }

    /// Hosts observing this store (tests).
    pub fn observers(&self) -> usize {
        self.subs.read().unwrap_or_else(|e| e.into_inner()).iter().filter(|w| w.strong_count() > 0).count()
    }
}

/// Reactive Rust state that Svelte reads like `$state` (design note §6.4). Cheap to clone (an `Arc`);
/// `Send + Sync`: any thread may write.
///
/// ```ignore
/// #[svelters::store(module = "sim")]
/// #[derive(Default)]
/// pub struct SimStats { pub step: u64, pub energy: f64, #[store(writable)] pub speed: f64 }
///
/// let stats = Store::new(SimStats::default());
/// stats.update(|s| { s.step += 1; s.energy = e; });                   // marks every field
/// stats.write(SimStatsField::Step, |s| s.step += 1);                  // marks one field
/// let e = stats.get(|s| s.energy);                                     // a read (Rust widgets, workers)
/// ```
///
/// In JS a store is an object whose field getters are reactive (`observed`) and whose writable fields
/// are assignable, so `<scale bind:value={stats.speed} />` binds to it.
pub struct Store<S>(pub(crate) Arc<StoreCell<S>>);

impl<S> Clone for Store<S> {
    fn clone(&self) -> Self {
        Store(self.0.clone())
    }
}

impl<S: StoreState> Store<S> {
    pub fn new(state: S) -> Store<S> {
        Store(Arc::new(StoreCell { state: RwLock::new(state), subs: RwLock::new(Vec::new()) }))
    }

    /// Reads under the read lock.
    pub fn get<R>(&self, f: impl FnOnce(&S) -> R) -> R {
        f(&self.0.read())
    }

    /// The read lock itself (keep it short: writers wait).
    pub fn read(&self) -> RwLockReadGuard<'_, S> {
        self.0.read()
    }

    /// Changes the state under the write lock and marks every field dirty (compare-with-last drops what
    /// did not change). Wakes each observing host once per burst.
    pub fn update<R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        let r = f(&mut self.0.write());
        self.0.each_sub(|s| s.mark_all());
        r
    }

    /// Changes the state under the write lock and marks only `field` dirty.
    pub fn write<R>(&self, field: S::Field, f: impl FnOnce(&mut S) -> R) -> R {
        let r = f(&mut self.0.write());
        let i = S::__index(field);
        self.0.each_sub(|s| s.mark(i));
        r
    }

    /// Changes the state under the write lock and marks the given fields dirty.
    pub fn write_fields<R>(&self, fields: &[S::Field], f: impl FnOnce(&mut S) -> R) -> R {
        let r = f(&mut self.0.write());
        for field in fields {
            let i = S::__index(*field);
            self.0.each_sub(|s| s.mark(i));
        }
        r
    }

    /// Marks `field` dirty without changing anything (state changed through interior mutability).
    pub fn touch(&self, field: S::Field) {
        let i = S::__index(field);
        self.0.each_sub(|s| s.mark(i));
    }

    /// Hosts observing this store right now (diagnostics, tests).
    pub fn observers(&self) -> usize {
        self.0.observers()
    }

    /// Whether `a` and `b` are the same store.
    pub fn ptr_eq(a: &Store<S>, b: &Store<S>) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}

/// An app-wide store in a `static`: created from `S::default()` (or [`StoreStatic::with`]) on first use,
/// never dropped, the same Rust object after every hot reload. Derefs to [`Store<S>`].
///
/// ```ignore
/// #[svelters::export(module = "sim")]                      // JS: import { simStats } from 'svelters:app/sim'
/// pub static SIM_STATS: StoreStatic<SimStats> = StoreStatic::new();
/// SIM_STATS.update(|s| s.step += 1);                        // any thread
/// ```
pub struct StoreStatic<S> {
    cell: OnceLock<Store<S>>,
    init: fn() -> S,
}

impl<S: StoreState + Default> StoreStatic<S> {
    pub const fn new() -> StoreStatic<S> {
        StoreStatic { cell: OnceLock::new(), init: S::default }
    }
}

impl<S: StoreState + Default> Default for StoreStatic<S> {
    fn default() -> Self {
        StoreStatic::new()
    }
}

impl<S: StoreState> StoreStatic<S> {
    /// A static store whose initial state comes from `init`.
    pub const fn with(init: fn() -> S) -> StoreStatic<S> {
        StoreStatic { cell: OnceLock::new(), init }
    }

    /// The store (created on first use).
    pub fn store(&self) -> &Store<S> {
        self.cell.get_or_init(|| Store::new((self.init)()))
    }
}

impl<S: StoreState> Deref for StoreStatic<S> {
    type Target = Store<S>;
    fn deref(&self) -> &Store<S> {
        self.store()
    }
}

impl<S: StoreState> ExportObject for StoreCell<S> {
    const MODULE: &'static str = S::MODULE;
    const NAME: &'static str = S::NAME;
    const SHARED: bool = true;
    const DOCS: &'static str = S::DOCS;
    type Holder = Arc<StoreCell<S>>;
    type Ref<'a> = &'a StoreCell<S>;
    fn __holder(o: &AnyObj) -> Option<Self::Holder> {
        match o {
            AnyObj::Shared(a) => a.clone().downcast::<StoreCell<S>>().ok(),
            AnyObj::Ui(_) => None,
        }
    }
    fn __borrow(h: &Self::Holder) -> Result<Self::Ref<'_>, NativeError> {
        Ok(&**h)
    }
    fn __erase(h: Self::Holder) -> AnyObj {
        AnyObj::Shared(h)
    }
    fn __new(_: Self) -> Self::Holder {
        unreachable!("a store cell is made by Store::new")
    }
}

impl<S: StoreState> SharedObject for StoreCell<S> {}

impl<S: StoreState> JsType for Store<S> {
    fn describe(b: &mut TypeBuilder) -> TypeRef {
        b.class::<StoreCell<S>>()
    }
}

impl<S: StoreState> IntoJs for Store<S> {
    fn into_js(self, cx: &CallCx<'_>) -> Result<Value, NativeError> {
        cx.adopt::<StoreCell<S>>(self.0)
    }
}

impl<S: StoreState> super::FromJs for Store<S> {
    fn from_js(v: Value, cx: &CallCx<'_>) -> Result<Self, NativeError> {
        super::__private::object::<StoreCell<S>>(v, cx).map(Store)
    }
}

/// A `static` an `#[svelters::export(module = "m")]` exposes as a module-level value (`export const`).
pub trait ExportStatic: Sync + 'static {
    type Js: IntoJs + JsType;
    fn js(&'static self) -> Self::Js;
}

impl<S: StoreState> ExportStatic for StoreStatic<S> {
    type Js = Store<S>;
    fn js(&'static self) -> Store<S> {
        self.store().clone()
    }
}

/// The store registration `#[svelters::store]` emits next to the class (`StoreCell<S>`).
pub struct StoreDef {
    pub type_id: fn() -> std::any::TypeId,
    pub fields: &'static [StoreField],
    /// Subscribes a host to the store behind an object-table entry.
    pub subscribe: fn(&AnyObj, &Arc<StoreSub>) -> bool,
}

inventory::collect!(StoreDef);

#[doc(hidden)]
pub fn subscribe_any<S: StoreState>(o: &AnyObj, sub: &Arc<StoreSub>) -> bool {
    match <StoreCell<S> as ExportObject>::__holder(o) {
        Some(cell) => {
            cell.subscribe(sub);
            true
        }
        None => false,
    }
}

/// The generated getter of field `i`: clones it under the read lock.
#[doc(hidden)]
pub fn store_get<S: StoreState>(cx: &CallCx<'_>, i: usize) -> Result<Value, NativeError> {
    let cell = super::__private::this::<StoreCell<S>>(cx)?;
    let g = cell.read();
    g.__read(i, cx)
}

/// The generated setter of writable field `i`: assigns under the write lock, runs the hooks, then marks
/// the field (or every field after hooks) in each observing host.
#[doc(hidden)]
pub fn store_set<S: StoreState>(cx: &CallCx<'_>, i: usize, args: Vec<Value>) -> Result<Value, NativeError> {
    let cell = super::__private::this::<StoreCell<S>>(cx)?;
    let v = args.into_iter().next().unwrap_or(Value::Null);
    let hooked = {
        let mut g = cell.write();
        g.__write(i, v, cx)?
    };
    if hooked {
        cell.each_sub(|s| s.mark_all());
    } else {
        cell.each_sub(|s| s.mark(i));
    }
    Ok(Value::Null)
}

// ---- events ----

/// One event of a `#[derive(svelters::Events)]` enum.
#[derive(Clone, Copy, Debug)]
pub struct EventInfo {
    /// The JS event name (kebab-case of the variant, or `#[js(rename)]`): `obj.on('diverged', fn)`.
    pub name: &'static str,
    /// `#[event(latest)]`: only the last occurrence per checkpoint is delivered.
    pub latest: bool,
    pub docs: &'static str,
}

/// An event enum (`#[derive(svelters::Events)]`): one JS event per variant. A unit variant has no
/// argument, a one-field tuple variant passes that value, a struct variant passes a plain object.
pub trait EventSet: Clone + Send + 'static {
    const EVENTS: &'static [EventInfo];
    #[doc(hidden)]
    fn __index(&self) -> usize;
    #[doc(hidden)]
    fn __payload(self, cx: &CallCx<'_>) -> Result<Option<Value>, NativeError>;
    /// The payload type of each event (`None`: no argument).
    #[doc(hidden)]
    fn __describe(b: &mut TypeBuilder) -> Vec<Option<TypeRef>>;
}

/// One host's subscription to one object's events: which events it listens to, and the slots of
/// `#[event(latest)]` events.
pub(crate) struct EmitSub {
    pub(crate) handle: u64,
    listened: AtomicU64,
    latest: Mutex<HashMap<usize, Payload>>,
    latest_pending: AtomicU64,
    hub: Arc<Hub>,
}

impl EmitSub {
    pub(crate) fn new(handle: u64, hub: Arc<Hub>) -> Arc<EmitSub> {
        Arc::new(EmitSub { handle, listened: AtomicU64::new(0), latest: Mutex::new(HashMap::new()), latest_pending: AtomicU64::new(0), hub })
    }

    pub(crate) fn listen(&self, index: usize, on: bool) {
        if index >= 64 {
            return;
        }
        if on {
            self.listened.fetch_or(1 << index, Ordering::AcqRel);
        } else {
            self.listened.fetch_and(!(1 << index), Ordering::AcqRel);
        }
    }

    pub(crate) fn listens(&self, index: usize) -> bool {
        index < 64 && self.listened.load(Ordering::Acquire) & (1 << index) != 0
    }

    pub(crate) fn listened_mask(&self) -> u64 {
        self.listened.load(Ordering::Acquire)
    }

    pub(crate) fn set_listened_mask(&self, m: u64) {
        self.listened.store(m, Ordering::Release);
    }

    fn post(self: &Arc<Self>, index: usize, latest: bool, payload: Payload) {
        if latest {
            lock(&self.latest).insert(index, payload);
            let bit = 1u64 << index;
            if self.latest_pending.fetch_or(bit, Ordering::AcqRel) & bit == 0 {
                self.hub.push_event(HubEvent::Latest { handle: self.handle, index, sub: self.clone() });
            }
        } else {
            self.hub.push_event(HubEvent::Event { handle: self.handle, index, payload });
        }
    }

    /// The latest payload of `index` (JS thread).
    pub(crate) fn take_latest(&self, index: usize) -> Option<Payload> {
        self.latest_pending.fetch_and(!(1u64 << index), Ordering::AcqRel);
        lock(&self.latest).remove(&index)
    }
}

/// The type-erased side of an [`Emitter`]: the hosts listening to it.
pub struct EmitterCore {
    subs: RwLock<Vec<Weak<EmitSub>>>,
}

impl EmitterCore {
    pub(crate) fn subscribe(&self, sub: &Arc<EmitSub>) {
        let mut subs = self.subs.write().unwrap_or_else(|e| e.into_inner());
        subs.retain(|w| w.strong_count() > 0);
        if !subs.iter().any(|w| w.upgrade().is_some_and(|s| Arc::ptr_eq(&s, sub))) {
            subs.push(Arc::downgrade(sub));
        }
    }
}

/// Events from Rust to JS (`obj.on('name', fn)`), emitted from any thread. A field of an exported object:
///
/// ```ignore
/// #[derive(svelters::Events, Clone)]
/// pub enum SimEvent { Diverged { step: u64 }, #[event(latest)] Tick(f64) }
///
/// #[svelters::object(shared, module = "sim")]
/// pub struct Simulation { events: Emitter<SimEvent>, /* ... */ }
///
/// self.events.emit(SimEvent::Diverged { step });           // any thread
/// ```
///
/// Delivery is at the host's next checkpoint, after the batch of changed properties and stores, in emit
/// order per object. An event nobody listens to costs a lock and a bit test. Debug builds warn once per
/// object above 1000 events per checkpoint (use `#[event(latest)]` or a store for high-rate values).
pub struct Emitter<E> {
    core: Arc<EmitterCore>,
    _e: PhantomData<fn(E)>,
}

impl<E> Clone for Emitter<E> {
    fn clone(&self) -> Self {
        Emitter { core: self.core.clone(), _e: PhantomData }
    }
}

impl<E: EventSet> Default for Emitter<E> {
    fn default() -> Self {
        Emitter::new()
    }
}

impl<E: EventSet> Emitter<E> {
    pub fn new() -> Emitter<E> {
        Emitter { core: Arc::new(EmitterCore { subs: RwLock::new(Vec::new()) }), _e: PhantomData }
    }

    /// Queues `event` for every host listening to it; returns how many were.
    pub fn emit(&self, event: E) -> usize {
        let i = event.__index();
        let latest = E::EVENTS.get(i).is_some_and(|e| e.latest);
        let subs: Vec<Arc<EmitSub>> = {
            let subs = self.core.subs.read().unwrap_or_else(|e| e.into_inner());
            subs.iter().filter_map(|w| w.upgrade()).filter(|s| s.listens(i)).collect()
        };
        let n = subs.len();
        let mut event = Some(event);
        for (k, s) in subs.iter().enumerate() {
            let e = if k + 1 == n { event.take().expect("last") } else { event.as_ref().expect("event").clone() };
            s.post(i, latest, Box::new(move |cx: &CallCx<'_>| e.__payload(cx)));
        }
        n
    }

    /// Whether any host listens to `name` (to skip building an expensive event).
    pub fn is_listened(&self, name: &str) -> bool {
        let Some(i) = E::EVENTS.iter().position(|e| e.name == name) else { return false };
        let subs = self.core.subs.read().unwrap_or_else(|e| e.into_inner());
        subs.iter().filter_map(|w| w.upgrade()).any(|s| s.listens(i))
    }

    #[doc(hidden)]
    pub fn __core(&self) -> Arc<EmitterCore> {
        self.core.clone()
    }
}

/// The events registration `#[svelters::object]` emits for a struct with an `Emitter<E>` field.
pub struct EmitterDef {
    pub type_id: fn() -> std::any::TypeId,
    pub events: &'static [EventInfo],
    pub describe: fn(&mut TypeBuilder) -> Vec<Option<TypeRef>>,
    /// The emitter of the object behind an object-table entry.
    pub emitter: fn(&AnyObj) -> Option<Arc<EmitterCore>>,
}

inventory::collect!(EmitterDef);

/// The emitter of an exported object (generated accessor).
pub fn emitter_of<T: ExportObject>(o: &AnyObj, f: fn(&T) -> Arc<EmitterCore>) -> Option<Arc<EmitterCore>> {
    let h = T::__holder(o)?;
    let r = T::__borrow(&h).ok()?;
    Some(f(&r))
}

// ---- module-level values: statics and services ----

/// A module-level value (`export const name` in `svelters:app/<module>`): an exported `static` or a
/// service. The stub reads it once when the module is evaluated (`callStatic` on the module kind).
pub struct ValueDef {
    pub module: &'static str,
    /// The JS name.
    pub name: &'static str,
    pub rust: &'static str,
    pub docs: &'static str,
    /// `"static"` or `"service"` (typings and schema).
    pub what: &'static str,
    pub ty: fn(&mut TypeBuilder) -> TypeRef,
    pub get: super::Thunk,
}

inventory::collect!(ValueDef);
