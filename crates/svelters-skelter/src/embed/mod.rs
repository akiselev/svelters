//! The embed model on skelter (ARCH-EMBED §4–§6, svelters-egui's `embed/` minus egui): many Svelte
//! roots in one QuickJS runtime, each mounted into a slot the host owns.
//!
//! ```ignore
//! let embed = Embed::new(EmbedConfig::default())?;
//! let entry = embed.load("ui/dist/embed.js")?;                       // svelteConfig({ embed })
//! let slot = embed.create_slot("panel.task", SlotOptions::default());
//! let root = embed.mount(MountSpec::new(entry, "TaskPanel", slot, "panel.task"))?;
//! // the app's UI phase:  embed.logic();                             JS runs here only
//! // after layout:        embed.set_slot_layout(slot, window, [w, h], scale);
//! embed.set_root_props(root, Props::new().with("count", 3))?;      // queued for the next logic
//! for ev in embed.take_events() { /* Mounted, Faulted, InfoChanged, Emitted, Error … */ }
//! ```
//!
//! - **Slots** are host-owned `box` nodes with no parent ([`SkelterBackend::create_borrowed`]): JS
//!   mounts into them and cannot destroy them. The app places a slot's node in its tree (a pane body,
//!   a dialog) and reports where it went with [`Embed::set_slot_layout`].
//! - **Roots** are mounts of a bundle's exported component into a slot. Every Rust command (mount,
//!   props, unmount, move, close, restart) is queued and applied in the next [`Embed::logic`], the
//!   only place the embed runs JS (event-time delivery runs it too, through the backend).
//! - **Faults** are per root: a render or effect error, an interrupted entry (the handler budget) or
//!   too many handler exceptions dispose that root only.
//! - **Idle** is 0 frames: [`Embed::settled`] / [`BusyProbe`] say why it is not idle, including a
//!   commit still on its way to the compositor (`COMMIT_IN_FLIGHT`, set by the app).

mod roots;
mod settle;
mod slots;

pub use roots::{FaultCause, RootError, RootId, RootInfo, RootOwner, RootState};
pub use settle::{Busy, BusyProbe, Settle};
pub use slots::{SlotId, SlotOptions};

use crate::backend::SkelterBackend;
use skelter_base::ids::{NodeId, WindowId};
use roots::{Root, SentInfo};
use slots::Slot;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::{Backend, Value, svl_handle};
use svelters_host::{ErrorKind, Host, HostConfig, JsError, RootMessage};

// ---- configuration ----

/// Per-entry time budgets (docking §6.4).
#[derive(Clone, Copy, Debug)]
pub struct Budgets {
    /// One entry into JS longer than this logs a warning in debug builds.
    pub warn: Duration,
    /// One entry into JS (an event handler, a props update, a timer) longer than this is
    /// interrupted and faults the root it ran for.
    pub interrupt: Duration,
    /// A first render may take longer than a handler.
    pub mount: Duration,
}

impl Default for Budgets {
    fn default() -> Self {
        Budgets { warn: Duration::from_millis(8), interrupt: Duration::from_millis(50), mount: Duration::from_millis(500) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FaultPolicy {
    /// Event-handler exceptions are reported (`EmbedEvent::Error`); this many within the window
    /// fault the root. `None`: never.
    pub handler_errors: Option<(u32, Duration)>,
}

impl Default for FaultPolicy {
    fn default() -> Self {
        FaultPolicy { handler_errors: Some((5, Duration::from_secs(10))) }
    }
}

#[derive(Clone, Debug)]
pub struct EmbedConfig {
    /// The host's config. `turn_budget` is the budget of `load`.
    pub host: HostConfig,
    pub budgets: Budgets,
    pub faults: FaultPolicy,
    /// A timer due within this counts as busy (`Busy::TIMER_SOON`); longer ones do not, or a clock
    /// would never settle.
    pub timer_horizon: Duration,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        EmbedConfig { host: HostConfig::default(), budgets: Budgets::default(), faults: FaultPolicy::default(), timer_horizon: Duration::from_millis(100) }
    }
}

impl EmbedConfig {
    /// For an app bundle (the runner): the host's config as given, every entry under its
    /// `turn_budget`.
    pub fn app(host: HostConfig) -> EmbedConfig {
        let b = host.turn_budget.unwrap_or(Duration::from_secs(24 * 3600));
        EmbedConfig { host, budgets: Budgets { warn: b, interrupt: b, mount: b }, ..EmbedConfig::default() }
    }
}

// ---- names and values ----

/// The host's stable name for a slot, e.g. the pane id `"panel.task"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlotKey(pub String);

/// The host's stable name for a root, unique in the embed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RootKey(pub String);

macro_rules! key_from {
    ($t:ident) => {
        impl From<&str> for $t {
            fn from(s: &str) -> $t {
                $t(s.to_string())
            }
        }
        impl From<String> for $t {
            fn from(s: String) -> $t {
                $t(s)
            }
        }
        impl $t {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
key_from!(SlotKey);
key_from!(RootKey);

/// A bundle loaded with [`Embed::load`] (its module namespace is kept).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(pub(crate) u32);

/// Props for a mount or a patch: an ordered map of values, plus keys to delete (a patch is a
/// shallow merge, D10).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props {
    set: Vec<(String, Value)>,
    delete: Vec<String>,
}

impl Props {
    pub fn new() -> Props {
        Props::default()
    }

    /// Sets `key` (replacing an earlier value for it in this patch).
    pub fn with(mut self, key: impl Into<String>, value: impl Into<PropValue>) -> Props {
        self.set(key, value);
        self
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<PropValue>) {
        let key = key.into();
        self.delete.retain(|k| *k != key);
        self.set.retain(|(k, _)| *k != key);
        self.set.push((key, value.into().0));
    }

    /// Removes `key` from the root's props (in a patch).
    pub fn delete(mut self, key: impl Into<String>) -> Props {
        let key = key.into();
        self.set.retain(|(k, _)| *k != key);
        self.delete.push(key);
        self
    }

    /// Props from a JSON object (any other JSON value is an empty set).
    pub fn json(v: serde_json::Value) -> Props {
        let mut p = Props::new();
        if let serde_json::Value::Object(m) = v {
            for (k, v) in m {
                p.set.push((k, json_value(v)));
            }
        }
        p
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.delete.is_empty()
    }

    /// Applies this patch to `base` (the root's last props).
    pub(crate) fn merge_into(&self, base: &mut Vec<(String, Value)>) {
        for (k, v) in &self.set {
            match base.iter_mut().find(|(b, _)| b == k) {
                Some(slot) => slot.1 = v.clone(),
                None => base.push((k.clone(), v.clone())),
            }
        }
        base.retain(|(k, _)| !self.delete.contains(k));
    }

    /// The patch as the runtime takes it (`$$delete` lists the removed keys).
    fn encoded(&self) -> Vec<(String, Value)> {
        let mut v = self.set.clone();
        if !self.delete.is_empty() {
            v.push(("$$delete".into(), Value::List(self.delete.iter().map(|k| Value::Str(k.clone())).collect())));
        }
        v
    }
}

/// A value for [`Props`]: anything `svelters_abi::Value` holds, with conversions from plain Rust.
pub struct PropValue(pub Value);

macro_rules! prop_value {
    ($($t:ty => $e:expr),* $(,)?) => {$(
        impl From<$t> for PropValue {
            fn from(x: $t) -> PropValue {
                let f: fn($t) -> Value = $e;
                PropValue(f(x))
            }
        }
    )*};
}
prop_value! {
    Value => |v| v,
    bool => Value::Bool,
    i32 => |x| Value::I64(x as i64),
    i64 => Value::I64,
    u32 => |x| Value::I64(x as i64),
    f32 => |x| Value::F64(x as f64),
    f64 => Value::F64,
    String => Value::Str,
    &str => |s| Value::Str(s.to_string()),
    serde_json::Value => json_value,
}

impl<T: Into<PropValue>> From<Vec<T>> for PropValue {
    fn from(v: Vec<T>) -> PropValue {
        PropValue(Value::List(v.into_iter().map(|x| x.into().0).collect()))
    }
}

/// JSON as a host value (objects become plain JS objects).
pub fn json_value(v: serde_json::Value) -> Value {
    use serde_json::Value as J;
    match v {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Number(n) => match n.as_i64() {
            Some(i) if i.unsigned_abs() < (1 << 53) => Value::I64(i),
            _ => Value::F64(n.as_f64().unwrap_or(f64::NAN)),
        },
        J::String(s) => Value::Str(s),
        J::Array(a) => Value::List(a.into_iter().map(json_value).collect()),
        J::Object(m) => Value::Map(m.into_iter().map(|(k, v)| (k, json_value(v))).collect()),
    }
}

/// One mount: a component exported by a loaded bundle, into an empty slot.
#[derive(Clone, Debug)]
pub struct MountSpec {
    pub entry: EntryId,
    /// The export's name, e.g. `"TaskPanel"`.
    pub export: Cow<'static, str>,
    pub slot: SlotId,
    pub key: RootKey,
    pub props: Props,
    pub owner: RootOwner,
}

impl MountSpec {
    pub fn new(entry: EntryId, export: impl Into<Cow<'static, str>>, slot: SlotId, key: impl Into<RootKey>) -> MountSpec {
        MountSpec { entry, export: export.into(), slot, key: key.into(), props: Props::new(), owner: RootOwner::InProcess }
    }
    pub fn props(mut self, props: Props) -> MountSpec {
        self.props = props;
        self
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum EmbedError {
    UnknownSlot,
    SlotOccupied,
    UnknownEntry,
    UnknownRoot,
    DuplicateKey(String),
    /// Isolates (SV15) are not built.
    Unsupported(&'static str),
    Host(JsError),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::UnknownSlot => f.write_str("no such slot (destroyed or another embed's)"),
            EmbedError::SlotOccupied => f.write_str("the slot already has a root"),
            EmbedError::UnknownEntry => f.write_str("no such entry"),
            EmbedError::UnknownRoot => f.write_str("no such root"),
            EmbedError::DuplicateKey(k) => write!(f, "a root with key '{k}' is already mounted"),
            EmbedError::Unsupported(w) => write!(f, "not supported yet: {w}"),
            EmbedError::Host(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EmbedError {}

/// What happened since the last [`Embed::take_events`].
#[derive(Clone, Debug)]
pub enum EmbedEvent {
    Mounted(RootId),
    Faulted { root: RootId, error: RootError },
    CloseDecided { root: RootId, allow: bool },
    /// JS wrote `root().title` or `root().dirty`: re-read [`Embed::root_info`].
    InfoChanged(RootId),
    /// `root().emit(name, value)`.
    Emitted { root: RootId, name: String, value: Value },
    /// Reported, not fatal: a handler exception (`root` from the handle it was dispatched on), or an
    /// error with no root context (a timer, a shared microtask).
    Error { root: Option<RootId>, error: JsError },
    /// Out of memory or another runtime-wide failure: every root faulted.
    RuntimeFaulted(JsError),
}

/// Counters for tests and measurements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmbedStats {
    pub logics: u64,
    pub mounts: u64,
    pub unmounts: u64,
    pub faults: u64,
    pub moves: u64,
    /// Wall time of the last `logic` and the longest one so far.
    pub last_logic: Duration,
    pub max_logic: Duration,
}

enum Command {
    Mount(RootId),
    Props(RootId, Props),
    Unmount(RootId),
    Close(RootId),
    Restart(RootId),
    DestroySlot(SlotId),
    /// `move_root`: the root's nodes go under `to` (reserved for it when queued).
    Move(RootId, SlotId),
}

#[derive(Default)]
struct State {
    slots: BTreeMap<SlotId, Slot>,
    slot_keys: HashMap<String, SlotId>,
    roots: BTreeMap<RootId, Root>,
    next_root: u32,
    entries: u32,
    commands: VecDeque<Command>,
    events: Vec<EmbedEvent>,
    stats: EmbedStats,
}

struct EmbedInner {
    // `host` first: it detaches from (and drops) its vtable before the backend handle below goes.
    host: Host<'static>,
    backend: SkelterBackend,
    cfg: EmbedConfig,
    st: RefCell<State>,
    probe: BusyProbe,
    commit_in_flight: Cell<bool>,
}

/// The embedded svelters on skelter: one backend, one host (one QuickJS runtime), its slots and
/// roots. `!Send`; cheap to clone (all clones are the same embed).
#[derive(Clone)]
pub struct Embed {
    inner: Rc<EmbedInner>,
}

impl Embed {
    /// An embed over the built-in kinds.
    pub fn new(cfg: EmbedConfig) -> Result<Embed, EmbedError> {
        Embed::with_backend(SkelterBackend::builtin(), cfg)
    }

    /// Creates the host over `backend` (the host owns its vtable) and attaches it (ABI v2.7
    /// `start`); the app drives the loop.
    pub fn with_backend(backend: SkelterBackend, cfg: EmbedConfig) -> Result<Embed, EmbedError> {
        let host = Host::new_owned(backend.clone(), cfg.host.clone()).map_err(EmbedError::Host)?;
        host.start().map_err(|e| EmbedError::Host(JsError::new(ErrorKind::Host, e.to_string())))?;
        host.set_turn_budget(Some(cfg.budgets.interrupt));
        Ok(Embed {
            inner: Rc::new(EmbedInner {
                host,
                backend,
                cfg,
                st: RefCell::new(State { next_root: 1, ..State::default() }),
                probe: BusyProbe::default(),
                commit_in_flight: Cell::new(false),
            }),
        })
    }

    pub fn host(&self) -> &Host<'static> {
        &self.inner.host
    }

    pub fn backend(&self) -> &SkelterBackend {
        &self.inner.backend
    }

    pub fn has_slot(&self, slot: SlotId) -> bool {
        self.inner.st.borrow().slots.contains_key(&slot)
    }

    // ---- bundles ----

    /// Evaluates an embed bundle (`svelteConfig({ embed })`, `dist/embed.js`) and keeps its
    /// namespace. Runs JS.
    pub fn load(&self, bundle: impl AsRef<Path>) -> Result<EntryId, JsError> {
        let host = &self.inner.host;
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_entry(bundle);
        self.loaded(r)
    }

    /// [`Embed::load`] from a module provider (an embed entry compiled from source).
    pub fn load_from(&self, provider: Rc<dyn svelters_host::ModuleProvider>) -> Result<EntryId, JsError> {
        let host = &self.inner.host;
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_entry_from(provider);
        self.loaded(r)
    }

    fn loaded(&self, r: Result<u32, JsError>) -> Result<EntryId, JsError> {
        self.inner.host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        let n = r?;
        self.inner.st.borrow_mut().entries = n + 1;
        Ok(EntryId(n))
    }

    /// The desktop order (the runner): evaluates an app bundle that mounts its `App` at the root
    /// itself (its `<window>`s).
    pub fn load_app(&self, bundle: impl AsRef<Path>) -> Result<(), JsError> {
        let host = &self.inner.host;
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_bundle(bundle);
        host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        r
    }

    /// [`Embed::load_app`] from a module provider (an app compiled from source).
    pub fn load_app_from(&self, provider: Rc<dyn svelters_host::ModuleProvider>) -> Result<(), JsError> {
        let host = &self.inner.host;
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_from(provider);
        host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        r
    }

    // ---- slots (no JS) ----

    /// Creates a slot (or returns the one with this key): a host-owned `box` node with no parent.
    pub fn create_slot(&self, key: impl Into<SlotKey>, opts: SlotOptions) -> SlotId {
        let key = key.into().0;
        if let Some(&s) = self.inner.st.borrow().slot_keys.get(&key) {
            return s;
        }
        let b = &self.inner.backend;
        let h = b.create_borrowed("box").expect("the built-in registry has `box`");
        if let Some(label) = opts.label {
            let p = b.resolve(svelters_abi::NameKind::Prop, b.handle_info(h).map(|i| i.kind).unwrap_or(0), "accessible-name");
            // `box` has `accessible-name` through `Widget`; a registry without it leaves the slot unnamed
            if p != 0 {
                let _ = b.set_prop(h, p, &Value::Str(label));
            }
        }
        let slot = SlotId(h);
        let mut st = self.inner.st.borrow_mut();
        st.slot_keys.insert(key.clone(), slot);
        st.slots.insert(slot, Slot::new(key));
        slot
    }

    /// Makes `node`, a node of the host's tree (the backend is [`SkelterBackend::over`] the host's
    /// document), the slot with this key (or returns the existing one): roots mount into it as into a
    /// created slot; destroying the slot leaves the node to the host.
    pub fn adopt_slot(&self, key: impl Into<SlotKey>, node: NodeId) -> Result<SlotId, String> {
        let key = key.into().0;
        if let Some(&s) = self.inner.st.borrow().slot_keys.get(&key) {
            return Ok(s);
        }
        let h = self.inner.backend.adopt(node)?;
        let slot = SlotId(h);
        let mut st = self.inner.st.borrow_mut();
        st.slot_keys.insert(key.clone(), slot);
        st.slots.insert(slot, Slot::new(key));
        Ok(slot)
    }

    pub fn slot(&self, key: &str) -> Option<SlotId> {
        self.inner.st.borrow().slot_keys.get(key).copied()
    }

    /// The slot's node, for the app to place in its tree.
    pub fn slot_node(&self, slot: SlotId) -> Option<NodeId> {
        self.inner.backend.node(slot.0)
    }

    /// Unmounts the slot's root (queued) and frees the slot after it.
    pub fn destroy_slot(&self, slot: SlotId) {
        let root = self.inner.st.borrow().slots.get(&slot).and_then(|s| s.root);
        if let Some(r) = root {
            self.unmount(r);
        }
        self.queue(Command::DestroySlot(slot));
    }

    /// Visibility comes from the host (the dock knows it exactly; D7). JS sees `root().visible` at the
    /// next logic.
    pub fn set_visible(&self, slot: SlotId, visible: bool) {
        if let Some(s) = self.inner.st.borrow_mut().slots.get_mut(&slot) {
            s.visible = visible;
        }
    }

    /// Where skelter's layout put the slot: its window, its size in points and the window's scale.
    /// JS sees `root().width`/`height`/`scale`/`viewport` at the next logic.
    pub fn set_slot_layout(&self, slot: SlotId, window: WindowId, size: [f32; 2], scale: f32) {
        if let Some(s) = self.inner.st.borrow_mut().slots.get_mut(&slot) {
            s.window = Some(window);
            s.size = Some(size);
            s.scale = scale;
        }
    }

    /// Keyboard focus entered (`true`) or left the slot (skelter-core's focus model tells the app).
    pub fn set_slot_focused(&self, slot: SlotId, focused: bool) {
        if let Some(s) = self.inner.st.borrow_mut().slots.get_mut(&slot) {
            s.focused = focused;
        }
    }

    /// The app sent a commit and the compositor has not applied it yet (`Busy::COMMIT_IN_FLIGHT`).
    pub fn set_commit_in_flight(&self, on: bool) {
        self.inner.commit_in_flight.set(on);
    }

    // ---- roots (queued; JS runs in the next `logic`) ----

    /// Queues a mount. Errors only for a bad slot or entry, an occupied slot or a key in use.
    pub fn mount(&self, spec: MountSpec) -> Result<RootId, EmbedError> {
        if spec.owner != RootOwner::InProcess {
            return Err(EmbedError::Unsupported("isolates (RootOwner::Isolate, SV15)"));
        }
        let mut st = self.inner.st.borrow_mut();
        if spec.entry.0 >= st.entries {
            return Err(EmbedError::UnknownEntry);
        }
        let slot = st.slots.get(&spec.slot).ok_or(EmbedError::UnknownSlot)?;
        if slot.root.is_some() {
            return Err(EmbedError::SlotOccupied);
        }
        if st.roots.values().any(|r| r.key == spec.key) {
            return Err(EmbedError::DuplicateKey(spec.key.0));
        }
        let id = RootId(st.next_root);
        st.next_root += 1;
        st.slots.get_mut(&spec.slot).expect("checked").root = Some(id);
        st.roots.insert(id, Root::new(spec.key, spec.entry, spec.export.into_owned(), spec.slot, spec.owner, &spec.props));
        st.commands.push_back(Command::Mount(id));
        Ok(id)
    }

    /// Shallow-merges `patch` into the root's props (queued).
    pub fn set_root_props(&self, root: RootId, patch: Props) -> Result<(), EmbedError> {
        let mut st = self.inner.st.borrow_mut();
        let r = st.roots.get_mut(&root).ok_or(EmbedError::UnknownRoot)?;
        patch.merge_into(&mut r.props);
        st.commands.push_back(Command::Props(root, patch));
        Ok(())
    }

    /// Unmounts the root (queued); its slot becomes empty.
    pub fn unmount(&self, root: RootId) {
        let mut st = self.inner.st.borrow_mut();
        let Some(r) = st.roots.get_mut(&root) else { return };
        if matches!(r.state, RootState::Unmounting) {
            return;
        }
        r.state = RootState::Unmounting;
        st.commands.push_back(Command::Unmount(root));
    }

    /// Moves a root into another, empty slot without a remount (a pane body ↔ an extension window):
    /// at the next logic its nodes are reparented under `to` (same JS instances, `$state` kept) and
    /// the runtime appends to `to` from then on.
    pub fn move_root(&self, root: RootId, to: SlotId) -> Result<(), EmbedError> {
        let mut st = self.inner.st.borrow_mut();
        let from = st.roots.get(&root).ok_or(EmbedError::UnknownRoot)?.slot;
        if from == to {
            return Ok(());
        }
        let t = st.slots.get_mut(&to).ok_or(EmbedError::UnknownSlot)?;
        if t.root.is_some() {
            return Err(EmbedError::SlotOccupied);
        }
        t.root = Some(root);
        st.commands.push_back(Command::Move(root, to));
        Ok(())
    }

    /// Asks the root's `onCloseRequest` handlers (in the next `logic`); the answer comes as
    /// [`EmbedEvent::CloseDecided`].
    pub fn request_close(&self, root: RootId) {
        self.queue(Command::Close(root));
    }

    /// Mounts a faulted root again with its last props (queued).
    pub fn restart_root(&self, root: RootId) {
        self.queue(Command::Restart(root));
    }

    fn queue(&self, c: Command) {
        self.inner.st.borrow_mut().commands.push_back(c);
    }

    // ---- the frame ----

    /// The app's UI phase, before style and layout: queued commands, root info, the host checkpoint
    /// (events, changes, timers), faults. All of the embed's own JS runs here.
    pub fn logic(&self) {
        let t0 = Instant::now();
        self.apply_commands();
        self.sync_info();
        self.inner.backend.poll_wake();
        // what kinds emitted outside input dispatch since the last logic (a list's row requests
        // from the frame, a programmatic focus move) reaches JS here (A25b)
        self.inner.backend.pump();
        self.inner.backend.count(|t| t.checkpoints += 1);
        self.inner.host.checkpoint();
        self.collect();
        // faults found above dispose their roots now
        self.apply_commands();
        let dt = t0.elapsed();
        {
            let mut st = self.inner.st.borrow_mut();
            st.stats.logics += 1;
            st.stats.last_logic = dt;
            st.stats.max_logic = st.stats.max_logic.max(dt);
        }
        #[cfg(debug_assertions)]
        if dt > self.inner.cfg.budgets.warn * 4 && self.inner.cfg.host.echo_errors {
            eprintln!("[svelters-skelter] embed logic took {dt:?} (budget warn {:?})", self.inner.cfg.budgets.warn);
        }
        self.publish();
    }

    // ---- state, idle, events ----

    /// Why the embed is busy (empty: settled).
    pub fn settled(&self) -> Settle {
        let hb = self.inner.host.busy();
        let b = &self.inner.backend;
        let st = self.inner.st.borrow();
        let mut busy = Busy::empty();
        busy.set(Busy::EVENTS, hb.events || hb.running || b.with_doc(|d| d.busy().contains(skelter_core::Busy::EVENTS)));
        busy.set(Busy::CHANGES, hb.changes);
        busy.set(Busy::JOBS, hb.jobs);
        busy.set(Busy::OPS_IN_FLIGHT, hb.ops_in_flight > 0);
        busy.set(Busy::STORES, hb.stores);
        busy.set(Busy::FRAME, hb.frame || b.0.frame_pending.get());
        let info_pending = st.roots.values().any(|r| r.state.is_live() && r.sent.as_ref() != Some(&info_of(&st, r)));
        busy.set(Busy::COMMANDS, !st.commands.is_empty() || info_pending);
        busy.set(Busy::MOUNTING, st.roots.values().any(|r| matches!(r.state, RootState::Mounting)));
        busy.set(Busy::LAYOUT, st.slots.values().any(|s| s.visible && s.root.is_some() && s.size.is_none()));
        busy.set(Busy::COMMIT_IN_FLIGHT, self.inner.commit_in_flight.get());
        let now = Instant::now();
        let next = [hb.next_timer, b.deadline()].into_iter().flatten().min();
        busy.set(Busy::TIMER_SOON, next.is_some_and(|t| t <= now + self.inner.cfg.timer_horizon));
        Settle { busy, next_timer: next }
    }

    /// A `Send + Sync` mirror of [`Embed::settled`] for idle barriers on other threads.
    pub fn busy_probe(&self) -> BusyProbe {
        self.inner.probe.clone()
    }

    pub fn take_events(&self) -> Vec<EmbedEvent> {
        std::mem::take(&mut self.inner.st.borrow_mut().events)
    }

    pub fn root_info(&self, root: RootId) -> Option<RootInfo> {
        let st = self.inner.st.borrow();
        let r = st.roots.get(&root)?;
        let s = st.slots.get(&r.slot);
        Some(RootInfo {
            key: r.key.clone(),
            slot: r.slot,
            state: r.state.clone(),
            title: r.title.clone(),
            dirty: r.dirty,
            size: s.and_then(|s| s.size).unwrap_or([0.0, 0.0]),
            window: s.and_then(|s| s.window),
            visible: s.is_some_and(|s| s.visible),
            owner: r.owner,
        })
    }

    pub fn roots(&self) -> Vec<RootId> {
        self.inner.st.borrow().roots.keys().copied().collect()
    }

    pub fn stats(&self) -> EmbedStats {
        self.inner.st.borrow().stats
    }

    // ---- internals ----

    fn publish(&self) {
        self.inner.probe.publish(self.settled().busy);
    }

    /// The host-owned `svelters:embed` fields of every live root, sent when they changed.
    fn sync_info(&self) {
        let todo: Vec<(RootId, SentInfo)> = {
            let st = self.inner.st.borrow();
            st.roots.iter().filter(|(_, r)| r.state.is_live()).map(|(&id, r)| (id, info_of(&st, r))).filter(|(id, now)| st.roots[id].sent.as_ref() != Some(now)).collect()
        };
        for (id, now) in todo {
            let patch = now.diff(self.inner.st.borrow().roots[&id].sent.as_ref());
            if let Err(e) = self.inner.host.root_info(id.0, &patch) {
                self.on_call_error(id, e);
            }
            if let Some(r) = self.inner.st.borrow_mut().roots.get_mut(&id) {
                r.sent = Some(now);
            }
        }
    }

    fn apply_commands(&self) {
        loop {
            let Some(cmd) = self.inner.st.borrow_mut().commands.pop_front() else { break };
            match cmd {
                Command::Mount(id) => self.do_mount(id),
                Command::Props(id, patch) => {
                    if self.state(id).is_some_and(|s| s.is_live())
                        && let Err(e) = self.inner.host.root_props(id.0, &patch.encoded())
                    {
                        self.on_call_error(id, e);
                    }
                }
                Command::Unmount(id) => self.do_unmount(id),
                Command::Close(id) => {
                    let allow = match self.state(id) {
                        Some(RootState::Live) => match self.inner.host.close_root(id.0) {
                            Ok(a) => a,
                            Err(e) => {
                                self.on_call_error(id, e);
                                true
                            }
                        },
                        Some(_) => true,
                        None => continue,
                    };
                    self.inner.st.borrow_mut().events.push(EmbedEvent::CloseDecided { root: id, allow });
                }
                Command::Restart(id) => {
                    if self.state(id).is_some_and(|s| s.is_faulted()) {
                        self.dispose(id);
                        if let Some(r) = self.inner.st.borrow_mut().roots.get_mut(&id) {
                            r.state = RootState::Mounting;
                            r.handler_errors.clear();
                        }
                        self.do_mount(id);
                    }
                }
                Command::DestroySlot(slot) => self.do_destroy_slot(slot),
                Command::Move(id, to) => self.do_move(id, to),
            }
        }
    }

    fn state(&self, id: RootId) -> Option<RootState> {
        self.inner.st.borrow().roots.get(&id).map(|r| r.state.clone())
    }

    fn do_mount(&self, id: RootId) {
        let Some((entry, export, slot, props)) =
            self.inner.st.borrow().roots.get(&id).and_then(|r| matches!(r.state, RootState::Mounting).then(|| (r.entry, r.export.clone(), r.slot, r.props.clone())))
        else {
            return;
        };
        let host = &self.inner.host;
        host.set_turn_budget(Some(self.inner.cfg.budgets.mount));
        let r = host.mount_root(entry.0, &export, id.0, slot.0, &props);
        host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        self.inner.st.borrow_mut().stats.mounts += 1;
        match r {
            Ok(()) => {
                let mut st = self.inner.st.borrow_mut();
                if let Some(r) = st.roots.get_mut(&id)
                    && matches!(r.state, RootState::Mounting)
                {
                    r.state = RootState::Live;
                    r.sent = None;
                    st.events.push(EmbedEvent::Mounted(id));
                }
            }
            Err(e) => {
                let cause = if e.kind == ErrorKind::Interrupted { FaultCause::Interrupt } else { FaultCause::Render };
                self.fault(id, cause, e);
            }
        }
    }

    fn do_unmount(&self, id: RootId) {
        if self.inner.st.borrow().roots.get(&id).is_none() {
            return;
        }
        self.dispose(id);
        let mut st = self.inner.st.borrow_mut();
        let Some(r) = st.roots.remove(&id) else { return };
        if let Some(s) = st.slots.get_mut(&r.slot)
            && s.root == Some(id)
        {
            s.root = None;
        }
        st.stats.unmounts += 1;
    }

    /// Disposes the root's JS (its natives, handlers, effects) and frees whatever is still under
    /// its slot. The root's table entry stays.
    fn dispose(&self, id: RootId) {
        let Some(slot) = self.inner.st.borrow().roots.get(&id).map(|r| r.slot) else { return };
        if let Err(e) = self.inner.host.unmount_root(id.0) {
            self.inner.st.borrow_mut().events.push(EmbedEvent::Error { root: Some(id), error: e });
        }
        // what a failed dispose left behind (a root interrupted mid-update) goes natively
        let b = &self.inner.backend;
        let mut kids: Vec<NodeId> = match b.node(slot.0) {
            Some(n) => b.with_doc(|d| d.children(n).to_vec()),
            None => Vec::new(),
        };
        // an adopted host node keeps the host's own children
        if b.is_adopted(slot.0) {
            kids.retain(|&k| b.handle(k).is_some());
        }
        let dead: Vec<svl_handle> = kids.into_iter().flat_map(|k| b.free_subtree(k)).collect();
        if let Some(host) = b.host() {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h, if i == 0 { svelters_abi::SVL_GONE_OWNER } else { svelters_abi::SVL_GONE_PARENT });
            }
        }
    }

    /// `move_root` (queued): the root's nodes go under `to`, the runtime's mount target follows
    /// (`__svelters_embed.retarget`).
    fn do_move(&self, id: RootId, to: SlotId) {
        let (from, live) = {
            let mut st = self.inner.st.borrow_mut();
            let Some(r) = st.roots.get(&id) else {
                // unmounted meanwhile: free the reservation
                if let Some(t) = st.slots.get_mut(&to).filter(|t| t.root == Some(id)) {
                    t.root = None;
                }
                return;
            };
            let (from, live) = (r.slot, r.state.is_live());
            if from == to || st.slots.get(&to).is_none_or(|t| t.root != Some(id)) {
                return; // `to` was destroyed (or the move is stale)
            }
            (from, live)
        };
        let b = &self.inner.backend;
        if let (Some(f), Some(t)) = (b.node(from.0), b.node(to.0)) {
            b.with_doc(|d| {
                for k in d.children(f).to_vec() {
                    // on the runtime's behalf: it is told below (retarget)
                    let _ = d.insert(t, k, None, None);
                }
            });
        }
        if live && let Err(e) = self.inner.host.retarget_root(id.0, to.0) {
            self.on_call_error(id, e);
        }
        let mut st = self.inner.st.borrow_mut();
        if let Some(r) = st.roots.get_mut(&id) {
            r.slot = to;
        }
        st.stats.moves += 1;
        if let Some(f) = st.slots.get_mut(&from) {
            f.root = None;
            f.focused = false;
        }
        if let Some(t) = st.slots.get_mut(&to) {
            t.size = None; // laid out at its next frame
        }
    }

    fn do_destroy_slot(&self, slot: SlotId) {
        let Some(s) = self.inner.st.borrow_mut().slots.remove(&slot) else { return };
        self.inner.st.borrow_mut().slot_keys.remove(&s.key);
        if let Some(r) = s.root {
            self.do_unmount(r);
        }
        let b = &self.inner.backend;
        // what JS left under it goes with it; the slot node itself is the embedder's
        let mut kids: Vec<NodeId> = match b.node(slot.0) {
            Some(n) => b.with_doc(|d| d.children(n).to_vec()),
            None => Vec::new(),
        };
        // an adopted host node keeps the host's own children
        if b.is_adopted(slot.0) {
            kids.retain(|&k| b.handle(k).is_some());
        }
        let dead: Vec<svl_handle> = kids.into_iter().flat_map(|k| b.free_subtree(k)).collect();
        if let Some(host) = b.host() {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h, if i == 0 { svelters_abi::SVL_GONE_OWNER } else { svelters_abi::SVL_GONE_PARENT });
            }
        }
        b.destroy_borrowed(slot.0);
    }

    /// Faults `id` now: disposes its JS and records the error.
    fn fault(&self, id: RootId, cause: FaultCause, error: JsError) {
        let mut st = self.inner.st.borrow_mut();
        let Some(r) = st.roots.get_mut(&id) else { return };
        if r.state.is_faulted() || matches!(r.state, RootState::Unmounting) {
            return;
        }
        let err = RootError { cause, error };
        r.state = RootState::Faulted(err.clone());
        st.stats.faults += 1;
        st.events.push(EmbedEvent::Faulted { root: id, error: err });
        drop(st);
        self.dispose(id);
    }

    /// An error thrown by one of the host's calls into a root (props, info, close).
    fn on_call_error(&self, id: RootId, e: JsError) {
        match e.kind {
            ErrorKind::Interrupted => self.fault(id, FaultCause::Interrupt, e),
            ErrorKind::OutOfMemory => self.runtime_fault(e),
            _ => self.fault(id, FaultCause::Render, e),
        }
    }

    fn runtime_fault(&self, e: JsError) {
        self.inner.st.borrow_mut().events.push(EmbedEvent::RuntimeFaulted(e.clone()));
        for id in self.roots() {
            self.fault(id, FaultCause::Runtime, e.clone());
        }
    }

    /// The root that owns handle `h`: the slot its node hangs under.
    fn root_of_handle(&self, h: svl_handle) -> Option<RootId> {
        let b = &self.inner.backend;
        let mut n = b.node(h)?;
        loop {
            if let Some(hh) = b.handle(n)
                && let Some(s) = self.inner.st.borrow().slots.get(&SlotId(hh))
            {
                return s.root;
            }
            n = b.with_doc(|d| d.parent(n))?;
        }
    }

    /// After the checkpoint: the host's errors (attributed through the handle they were
    /// dispatched on) and what roots reported.
    fn collect(&self) {
        for e in self.inner.host.take_errors() {
            let root = e.handle.and_then(|h| self.root_of_handle(h));
            match (e.kind, root) {
                (ErrorKind::OutOfMemory, _) => self.runtime_fault(e),
                (ErrorKind::Interrupted, Some(r)) => self.fault(r, FaultCause::Interrupt, e),
                (_, Some(r)) => {
                    let exceeded = {
                        let mut st = self.inner.st.borrow_mut();
                        st.events.push(EmbedEvent::Error { root: Some(r), error: e.clone() });
                        match (self.inner.cfg.faults.handler_errors, st.roots.get_mut(&r)) {
                            (Some((n, window)), Some(root)) => {
                                let now = Instant::now();
                                root.handler_errors.push_back(now);
                                while root.handler_errors.front().is_some_and(|&t| now.duration_since(t) > window) {
                                    root.handler_errors.pop_front();
                                }
                                root.handler_errors.len() as u32 >= n
                            }
                            _ => false,
                        }
                    };
                    if exceeded {
                        self.fault(r, FaultCause::Handler, e);
                    }
                }
                (_, None) => self.inner.st.borrow_mut().events.push(EmbedEvent::Error { root: None, error: e }),
            }
        }
        for m in self.inner.host.take_root_messages() {
            match m {
                RootMessage::Fault { root, error } => self.fault(RootId(root), FaultCause::Render, error),
                RootMessage::Info { root, title, dirty } => {
                    let mut st = self.inner.st.borrow_mut();
                    if let Some(r) = st.roots.get_mut(&RootId(root)) {
                        if let Some(t) = title {
                            r.title = t;
                        }
                        if let Some(d) = dirty {
                            r.dirty = d;
                        }
                        st.events.push(EmbedEvent::InfoChanged(RootId(root)));
                    }
                }
                RootMessage::Emit { root, name, value } => {
                    let value = serde_json::from_str(&value).map(json_value).unwrap_or(Value::Null);
                    self.inner.st.borrow_mut().events.push(EmbedEvent::Emitted { root: RootId(root), name, value });
                }
            }
        }
    }
}

fn info_of(st: &State, r: &Root) -> SentInfo {
    let s = st.slots.get(&r.slot);
    let size = s.and_then(|s| s.size).unwrap_or([0.0, 0.0]);
    SentInfo {
        key: r.key.0.clone(),
        visible: s.is_some_and(|s| s.visible),
        width: size[0] as f64,
        height: size[1] as f64,
        scale: s.map(|s| s.scale).unwrap_or(1.0) as f64,
        viewport: s.and_then(|s| s.window).map(|w| format!("window:{}", w.0)).unwrap_or_default(),
        focused: s.is_some_and(|s| s.focused),
    }
}
