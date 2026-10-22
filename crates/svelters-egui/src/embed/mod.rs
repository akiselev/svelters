//! EMBED (EM1, plans/research/ARCH-EMBED.md §4–§6): svelters-egui as a library inside a host's
//! egui frame, with many Svelte roots in one QuickJS runtime.
//!
//! ```ignore
//! let embed = Embed::new(&ctx, EmbedConfig::default())?;
//! let entry = embed.load("ui/dist/embed.js")?;                       // svelteConfig({ embed })
//! let slot = embed.create_slot("panel.task", SlotOptions::fill());
//! let root = embed.mount(MountSpec::new(entry, "TaskPanel", slot, "panel.task"))?;
//! // App::logic:  embed.logic(ctx);                                  JS runs here only
//! // App::ui:     embed.show_slot(ui, slot);                         from any pane, never JS
//! embed.set_root_props(root, Props::new().with("count", 3))?;      // queued for the next logic
//! for ev in embed.take_events() { /* Mounted, Faulted, InfoChanged, Emitted, Error … */ }
//! ```
//!
//! - **Slots** are Rust-created nodes with no parent, drawn wherever the host calls
//!   [`Embed::show_slot`]. Node ids derive from the embed's salt and the root's key (`ids.rs`), never
//!   from the `Ui`, so drawing a slot from another pane keeps egui memory and AccessKit ids.
//! - **Roots** are mounts of a bundle's exported component into a slot. Every Rust command (mount,
//!   props, unmount, close, restart) is queued and applied in the next [`Embed::logic`], the only
//!   place JS runs (cadatomic K-1).
//! - **Faults** are per root: a render or effect error (the root's boundary in
//!   `@svelters/native-runtime/embed`), an interrupted entry (the handler budget) or too many
//!   handler exceptions dispose that root only; its slot draws the fault UI.
//! - **Idle** is 0 frames: nothing requests a repaint unless work is pending, and
//!   [`Embed::settled`] / [`BusyProbe`] say why it is not idle.
//!
//! EM2 adds deferred viewports (`current.rs`: `SlotPainter`, the begin-pass plugin), per-viewport
//! repaint routing, focus restore on `set_viewport`, and `move_root`. Writer lane: EMBED.

mod current;
mod roots;
mod settle;
mod slots;

pub use roots::{FaultCause, RootError, RootId, RootInfo, RootOwner, RootState};
pub use settle::{Busy, BusyProbe, Settle};
pub use slots::{FaultUi, SlotId, SlotOptions, SlotResponse, SlotSizing};

use crate::backend::EguiBackend;
use crate::scene::{Handle, NodeId, Owner, Phase, TreeRoot};
use roots::{Root, SentInfo};
use slots::Slot;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::Value;
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
    /// Salts every egui id of this embed. Two embeds in one `Context` must differ (debug-asserted).
    pub id_salt: &'static str,
    /// The host's config. `turn_budget` is the budget of `load`; the default here does not quit
    /// when the last window goes (an embedder has none).
    pub host: HostConfig,
    pub budgets: Budgets,
    pub faults: FaultPolicy,
    /// A timer due within this counts as busy (`Busy::TIMER_SOON`); longer ones do not, or a clock
    /// would never settle.
    pub timer_horizon: Duration,
    /// Run `logic` from an egui begin-pass plugin for deferred viewports (§5.2). EM2: ignored by EM1.
    pub prepass_plugin: bool,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        EmbedConfig {
            id_salt: "svelters",
            host: HostConfig { quit_on_last_window: false, ..HostConfig::default() },
            budgets: Budgets::default(),
            faults: FaultPolicy::default(),
            timer_horizon: Duration::from_millis(50),
            prepass_plugin: true,
        }
    }
}

impl EmbedConfig {
    /// For an app bundle (the desktop runners, `TestApp`): the host's config as given, every entry
    /// under its `turn_budget` (as before EM1), quitting when the last window goes if it says so.
    pub fn app(host: HostConfig) -> EmbedConfig {
        let b = host.turn_budget.unwrap_or(Duration::from_secs(24 * 3600));
        EmbedConfig { host, budgets: Budgets { warn: b, interrupt: b, mount: b }, ..EmbedConfig::default() }
    }
}

// ---- names and values ----

/// The host's stable name for a slot, e.g. the pane id `"panel.task"`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlotKey(pub String);

/// The host's stable name for a root: salts the root's egui ids (ARCH-EMBED §4.4), so it must be
/// stable across runs and unique in the embed.
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
pub enum EmbedError {
    UnknownSlot,
    SlotOccupied,
    UnknownEntry,
    UnknownRoot,
    DuplicateKey(String),
    /// Isolates (SV15) are not built yet.
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
    /// `logic` calls that ran (first pass of a frame).
    pub logics: u64,
    pub mounts: u64,
    pub unmounts: u64,
    pub faults: u64,
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
    /// A fault found outside `logic` (a panic in `show_slot`): dispose the root's JS.
    Dispose(RootId),
    DestroySlot(SlotId),
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
    last_pass: Option<u64>,
    stats: EmbedStats,
}

struct EmbedInner {
    // `host` first: it detaches from (and drops) its vtable before the backend handle below goes.
    host: Host<'static>,
    backend: EguiBackend,
    cfg: EmbedConfig,
    st: RefCell<State>,
    probe: BusyProbe,
    /// The context the salt was registered in (debug check), for `Drop`.
    salt_ctx: egui::Context,
}

/// The embedded svelters-egui: one backend, one host (one QuickJS runtime), its slots and roots.
/// `!Send`; cheap to clone (all clones are the same embed).
#[derive(Clone)]
pub struct Embed {
    inner: Rc<EmbedInner>,
}

#[derive(Clone, Default)]
struct Salts(Vec<String>);

fn salts_id() -> egui::Id {
    egui::Id::unique("svelters-embed-salts")
}

impl Drop for EmbedInner {
    fn drop(&mut self) {
        let salt = self.cfg.id_salt;
        self.salt_ctx.data_mut(|d| {
            if let Some(s) = d.get_temp_mut_or_default::<Salts>(salts_id()).0.iter().position(|x| x == salt) {
                d.get_temp_mut_or_default::<Salts>(salts_id()).0.remove(s);
            }
        });
    }
}

impl Embed {
    /// Creates the backend and the host (which owns its vtable) and attaches the host (ABI v2.7
    /// `start`). Wakes and host ops repaint `ctx` (rebound by every `logic`).
    pub fn new(ctx: &egui::Context, cfg: EmbedConfig) -> Result<Embed, EmbedError> {
        let clash = ctx.data_mut(|d| {
            let s = d.get_temp_mut_or_default::<Salts>(salts_id());
            let clash = s.0.iter().any(|x| x == cfg.id_salt);
            s.0.push(cfg.id_salt.to_string());
            clash
        });
        debug_assert!(!clash, "svelters-egui: two embeds with the id salt '{}' in one egui Context", cfg.id_salt);
        let backend = EguiBackend::new();
        backend.bind_ctx(ctx);
        backend.0.scene.hook_passes(ctx);
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
                salt_ctx: ctx.clone(),
            }),
        })
    }

    fn salt(&self) -> &'static str {
        self.inner.cfg.id_salt
    }

    // ---- bundles ----

    /// Evaluates an embed bundle (`svelteConfig({ embed })`, `dist/embed.js`) and keeps its
    /// namespace. Runs JS: call it before the first frame or from `App::logic`.
    pub fn load(&self, bundle: impl AsRef<Path>) -> Result<EntryId, JsError> {
        let host = &self.inner.host;
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_entry(bundle);
        host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        let n = r?;
        self.inner.st.borrow_mut().entries = n + 1;
        Ok(EntryId(n))
    }

    /// The desktop order (the runners): evaluates an app bundle that mounts its `App` at the root
    /// itself (`<window>`s, drawn by [`Embed::show_windows`]).
    pub fn load_app(&self, bundle: impl AsRef<Path>) -> Result<(), JsError> {
        let host = &self.inner.host;
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_bundle(bundle);
        host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        r
    }

    // ---- slots (no JS) ----

    /// Creates a slot (or returns the one with this key).
    pub fn create_slot(&self, key: impl Into<SlotKey>, opts: SlotOptions) -> SlotId {
        let key = key.into().0;
        if let Some(&s) = self.inner.st.borrow().slot_keys.get(&key) {
            return s;
        }
        let id = crate::ids::slot_id(self.salt(), &key);
        let box_kind = crate::kinds::tag_index("box").expect("the box kind");
        // A host-owned box (JS sees it borrowed), the top of its own scene root (ARCH-SCENE §4.5).
        let h = {
            let mut scene = self.inner.backend.0.scene.borrow_mut();
            let n = scene.create(box_kind, Owner::Host).expect("a box");
            let i = scene.ix(n);
            scene.core[i].egui_id = id;
            scene.add_root(TreeRoot::Slot { node: n }, n);
            n.to_raw()
        };
        let slot = SlotId(h);
        let mut st = self.inner.st.borrow_mut();
        st.slot_keys.insert(key.clone(), slot);
        st.slots.insert(
            slot,
            Slot { key, opts, root: None, visible: true, viewport: egui::ViewportId::ROOT, rect: None, content: egui::Vec2::ZERO },
        );
        slot
    }

    pub fn slot(&self, key: &str) -> Option<SlotId> {
        self.inner.st.borrow().slot_keys.get(key).copied()
    }

    /// Unmounts the slot's root (queued) and frees the slot after it.
    pub fn destroy_slot(&self, slot: SlotId) {
        let root = self.inner.st.borrow().slots.get(&slot).and_then(|s| s.root);
        if let Some(r) = root {
            self.unmount(r);
        }
        self.queue(Command::DestroySlot(slot));
    }

    /// Hidden: no layout or walk, its animation frames wait, and JS sees `visible = false`.
    pub fn set_visible(&self, slot: SlotId, visible: bool) {
        let mut st = self.inner.st.borrow_mut();
        if let Some(s) = st.slots.get_mut(&slot)
            && s.visible != visible
        {
            s.visible = visible;
            drop(st);
            self.repaint();
        }
    }

    /// The viewport the slot is drawn in from now on (`root().viewport`). EM2 adds per-viewport
    /// repaint routing and focus restore.
    pub fn set_viewport(&self, slot: SlotId, viewport: egui::ViewportId) {
        let mut st = self.inner.st.borrow_mut();
        if let Some(s) = st.slots.get_mut(&slot)
            && s.viewport != viewport
        {
            s.viewport = viewport;
            drop(st);
            self.repaint();
        }
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
        st.slots.get_mut(&spec.slot).unwrap().root = Some(id);
        st.roots.insert(id, Root::new(spec.key, spec.entry, spec.export.into_owned(), spec.slot, spec.owner, &spec.props));
        st.commands.push_back(Command::Mount(id));
        drop(st);
        self.repaint();
        Ok(id)
    }

    /// Shallow-merges `patch` into the root's props (queued).
    pub fn set_root_props(&self, root: RootId, patch: Props) -> Result<(), EmbedError> {
        let mut st = self.inner.st.borrow_mut();
        let r = st.roots.get_mut(&root).ok_or(EmbedError::UnknownRoot)?;
        patch.merge_into(&mut r.props);
        st.commands.push_back(Command::Props(root, patch));
        drop(st);
        self.repaint();
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
        drop(st);
        self.repaint();
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
        self.repaint();
    }

    fn repaint(&self) {
        if let Some(ctx) = self.inner.backend.ctx() {
            ctx.request_repaint();
        }
    }

    // ---- the frame ----

    /// From `App::logic` (or first thing in the root viewport's ui): queued commands, root info,
    /// the host checkpoint, animation frames, remeasure, faults, repaints. The first pass of a
    /// frame only; later calls in the same pass are no-ops. All JS runs here.
    pub fn logic(&self, ctx: &egui::Context) {
        let b = &self.inner.backend;
        // R17: this frame's logic ran (or runs only in the first pass): the rest of the pass is `Ui`.
        if ctx.current_pass_index() != 0 {
            b.0.scene.set_phase(Phase::Ui);
            return;
        }
        let pass = ctx.cumulative_pass_nr();
        {
            let mut st = self.inner.st.borrow_mut();
            if st.last_pass == Some(pass) {
                b.0.scene.set_phase(Phase::Ui);
                return;
            }
            st.last_pass = Some(pass);
        }
        let t0 = Instant::now();
        b.bind_ctx(ctx);
        b.0.scene.hook_passes(ctx);
        b.0.scene.set_phase(Phase::Logic);
        b.0.in_frame.set(true);
        self.apply_commands();
        self.sync_info(ctx);
        b.0.in_frame.set(false);
        // checkpoint (events, changes, timers), rAF, remeasure: every entry under the handler budget
        b.logic_pass(ctx);
        self.collect();
        // faults found above dispose their roots now
        b.0.in_frame.set(true);
        self.apply_commands();
        b.0.in_frame.set(false);
        b.after_pass(ctx, Vec::new());
        if !self.inner.st.borrow().commands.is_empty() {
            ctx.request_repaint();
        }
        // R17: `write` fails from here to the end of the egui pass (the pass hooks return the scene
        // to `Idle`); `defer` runs at the next logic. A logic-only frame (no pass) ends in `Idle`.
        b.0.scene.set_phase(if b.0.scene.in_pass() { Phase::Ui } else { Phase::Idle });
        let dt = t0.elapsed();
        {
            let mut st = self.inner.st.borrow_mut();
            st.stats.logics += 1;
            st.stats.last_logic = dt;
            st.stats.max_logic = st.stats.max_logic.max(dt);
        }
        #[cfg(debug_assertions)]
        if dt > self.inner.cfg.budgets.warn * 4 && self.inner.cfg.host.echo_errors {
            eprintln!("[svelters-egui] embed logic took {dt:?} (budget warn {:?})", self.inner.cfg.budgets.warn);
        }
        self.publish();
    }

    /// Draws `slot` in `ui`'s available rect: layout if stale, then the walk; widget events go to
    /// the host's queue for the next `logic`. Never runs JS. Call it from `App::ui` or any
    /// immediate viewport's closure, on the UI thread.
    pub fn show_slot(&self, ui: &mut egui::Ui, slot: SlotId) -> SlotResponse {
        let (key, visible, root, label, fault_ui) = {
            let st = self.inner.st.borrow();
            let Some(s) = st.slots.get(&slot) else {
                debug_assert!(false, "show_slot: a destroyed or foreign slot");
                return SlotResponse { rect: egui::Rect::NOTHING, content_size: egui::Vec2::ZERO, state: RootState::Empty };
            };
            (s.key.clone(), s.visible, s.root, s.opts.label.clone(), s.opts.fault_ui.clone())
        };
        let state = root.and_then(|r| self.inner.st.borrow().roots.get(&r).map(|r| r.state.clone())).unwrap_or(RootState::Empty);
        if !visible {
            return SlotResponse { rect: egui::Rect::NOTHING, content_size: egui::Vec2::ZERO, state: RootState::Hidden };
        }
        let rect = ui.available_rect_before_wrap();
        if let (RootState::Faulted(err), Some(r)) = (&state, root) {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).scope_id(crate::ids::slot_id(self.salt(), &key).with("fault")));
            let reload = match &fault_ui {
                FaultUi::Default => {
                    let name = self.inner.st.borrow().roots.get(&r).map(|r| r.key.0.clone()).unwrap_or_default();
                    child.label(format!("{name} failed: {}", err.error.message));
                    child.button("Reload").clicked()
                }
                FaultUi::Custom(f) => f(&mut child, err),
            };
            ui.advance_cursor_after_rect(rect);
            if reload {
                self.restart_root(r);
            }
            self.publish();
            return SlotResponse { rect, content_size: egui::Vec2::ZERO, state };
        }
        let b = &self.inner.backend;
        let sid = b.0.scene.borrow().get(slot.0).map(|n| n.id).unwrap_or(egui::Id::NULL);
        let mut builder = egui::UiBuilder::new().max_rect(rect).scope_id(sid).accessibility_role(egui::accesskit::Role::Group);
        if let Some(l) = &label {
            builder = builder.accessibility_label(l.clone());
        }
        let mut child = ui.new_child(builder);
        crate::a11y::slot_group(ui.ctx(), child.unique_id(), &key);
        b.0.in_frame.set(true);
        b.0.pass_active.set(true);
        let mut out = Vec::new();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.slot_ui(&mut child, slot.0, &mut out)));
        b.0.pass_active.set(false);
        ui.advance_cursor_after_rect(rect);
        let (drawn, content) = match res {
            Ok(x) => x,
            Err(p) => {
                let msg = p
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panic".into());
                if let Some(r) = root {
                    self.fault_later(r, FaultCause::Panic, JsError::new(ErrorKind::Host, format!("panic while drawing: {msg}")));
                }
                (rect, egui::Vec2::ZERO)
            }
        };
        let resized = {
            let mut st = self.inner.st.borrow_mut();
            let s = st.slots.get_mut(&slot).unwrap();
            let resized = s.rect.is_none_or(|r| r.size() != drawn.size());
            s.rect = Some(drawn);
            s.content = content;
            resized
        };
        b.after_pass(ui.ctx(), out);
        b.0.in_frame.set(false);
        if resized && root.is_some() {
            // `root().width/height` follow at the next logic
            ui.ctx().request_repaint();
        }
        self.publish();
        SlotResponse { rect: drawn, content_size: content, state }
    }

    /// The desktop runners' walk: presented `<window>`s (the root one in `ui`, the others as
    /// immediate viewports), for bundles loaded with [`Embed::load_app`].
    pub fn show_windows(&self, ui: &mut egui::Ui) {
        self.inner.backend.show_windows(ui);
        self.publish();
    }

    // ---- state, idle, events ----

    /// Why the embed is busy (empty: settled).
    pub fn settled(&self) -> Settle {
        let hb = self.inner.host.busy();
        let b = &self.inner.backend;
        let st = self.inner.st.borrow();
        let mut busy = Busy::empty();
        busy.set(Busy::EVENTS, hb.events || hb.running || b.0.shared.wake.load(std::sync::atomic::Ordering::Acquire));
        busy.set(Busy::CHANGES, hb.changes);
        busy.set(Busy::JOBS, hb.jobs);
        busy.set(Busy::OPS_IN_FLIGHT, hb.ops_in_flight > 0);
        busy.set(Busy::STORES, hb.stores);
        busy.set(Busy::FRAME, hb.frame || b.0.frame_pending.get());
        busy.set(Busy::OUTBOX, b.0.checkpoint_pending.get());
        let info_pending = st.roots.values().any(|r| r.state.is_live() && r.sent.as_ref() != Some(&self.info_of(&st, r)));
        busy.set(Busy::COMMANDS, !st.commands.is_empty() || info_pending);
        busy.set(Busy::MOUNTING, st.roots.values().any(|r| matches!(r.state, RootState::Mounting)));
        // A visible slot with a root is stale until drawn once, then while its scene root has queued
        // layout work; leaves anywhere may wait to be remeasured (next logic).
        let scene = b.0.scene.borrow();
        let stale = st.slots.iter().any(|(id, s)| {
            s.visible && s.root.is_some() && (s.rect.is_none() || scene.slot_root(id.0).is_some_and(|e| !e.dirty.is_empty()))
        });
        let remeasure = scene.roots.iter().any(|(_, e)| !e.dirty.remeasure.is_empty());
        busy.set(Busy::LAYOUT, stale || remeasure);
        drop(scene);
        // R16: Rust writes deferred to the next logic, and announcements not delivered yet.
        busy |= settle::scene_busy(&b.0.scene);
        let now = Instant::now();
        let next = [hb.next_timer, b.0.deadline.get()].into_iter().flatten().min();
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
        let s = st.slots.get(&r.slot)?;
        Some(RootInfo {
            key: r.key.clone(),
            slot: r.slot,
            state: r.state.clone(),
            title: r.title.clone(),
            dirty: r.dirty,
            size: s.rect.map(|x| x.size()).unwrap_or(egui::Vec2::ZERO),
            viewport: s.viewport,
            visible: s.visible,
            owner: r.owner,
        })
    }

    pub fn roots(&self) -> Vec<RootId> {
        self.inner.st.borrow().roots.keys().copied().collect()
    }

    /// Last frame's screen rect of the node with `anchor="…"` in `root` (overlays, tutorials; SV7).
    pub fn anchor_rect(&self, root: RootId, anchor: &str) -> Option<(egui::ViewportId, egui::Rect)> {
        let n = self.anchored(root, anchor)?;
        let st = self.inner.st.borrow();
        let s = st.slots.get(&st.roots.get(&root)?.slot)?;
        let origin = s.rect?.min.to_vec2();
        let scene = self.inner.backend.0.scene.borrow();
        let node = scene.get(n)?;
        let r = if node.drawn != egui::Rect::NOTHING && node.children.is_empty() { node.drawn } else { node.rect.translate(origin) };
        Some((s.viewport, r))
    }

    /// The egui id of the node with `anchor="…"` in `root` (tests, overlays).
    pub fn anchor_id(&self, root: RootId, anchor: &str) -> Option<egui::Id> {
        let h = self.anchored(root, anchor)?;
        self.inner.backend.0.scene.borrow().get(h).map(|n| n.id)
    }

    fn anchored(&self, root: RootId, anchor: &str) -> Option<Handle> {
        if anchor.is_empty() {
            return None;
        }
        let slot = self.inner.st.borrow().roots.get(&root)?.slot;
        let scene = self.inner.backend.0.scene.borrow();
        let r = scene.core[scene.ix(scene.live(slot.0).ok()?)].root;
        scene.ids().find(|&id| {
            let i = scene.ix(id);
            scene.core[i].root == r && scene.common(i).anchor == anchor
        })
        .map(NodeId::to_raw)
    }

    /// Exports, op log, stats, interrupt flag: the underlying host.
    pub fn host(&self) -> &Host<'static> {
        &self.inner.host
    }

    /// The backend (scene access for tests and devtools: `with_scene`, `handles_of`, `stats`).
    pub fn backend(&self) -> &EguiBackend {
        &self.inner.backend
    }

    pub fn stats(&self) -> EmbedStats {
        self.inner.st.borrow().stats
    }

    // ---- internals ----

    fn publish(&self) {
        self.inner.probe.publish(self.settled().busy);
    }

    fn info_of(&self, st: &State, r: &Root) -> SentInfo {
        let s = st.slots.get(&r.slot);
        let size = s.and_then(|s| s.rect).map(|x| x.size()).unwrap_or(egui::Vec2::ZERO);
        let ppp = self.inner.backend.ctx().map(|c| c.pixels_per_point()).unwrap_or(1.0);
        SentInfo {
            key: r.key.0.clone(),
            visible: s.is_some_and(|s| s.visible),
            width: size.x as f64,
            height: size.y as f64,
            scale: ppp as f64,
            viewport: s.map(|s| viewport_name(s.viewport)).unwrap_or_default(),
        }
    }

    /// Step 3: the host-owned `svelters:embed` fields of every live root, sent when they changed.
    fn sync_info(&self, _ctx: &egui::Context) {
        let todo: Vec<(RootId, SentInfo)> = {
            let st = self.inner.st.borrow();
            st.roots
                .iter()
                .filter(|(_, r)| r.state.is_live())
                .map(|(&id, r)| (id, self.info_of(&st, r)))
                .filter(|(id, now)| st.roots[id].sent.as_ref() != Some(now))
                .collect()
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
                            r.mount_requested = Instant::now();
                            r.handler_errors.clear();
                        }
                        self.do_mount(id);
                    }
                }
                Command::Dispose(id) => self.dispose(id),
                Command::DestroySlot(slot) => self.do_destroy_slot(slot),
            }
        }
    }

    fn state(&self, id: RootId) -> Option<RootState> {
        self.inner.st.borrow().roots.get(&id).map(|r| r.state.clone())
    }

    fn do_mount(&self, id: RootId) {
        let Some((entry, export, slot, props, key)) = self.inner.st.borrow().roots.get(&id).and_then(|r| {
            matches!(r.state, RootState::Mounting).then(|| (r.entry, r.export.clone(), r.slot, r.props.clone(), r.key.0.clone()))
        }) else {
            return;
        };
        // The root's nodes take ids from its key as they are inserted (the attach hook).
        self.set_slot_base(slot, Some(crate::ids::root_base(self.salt(), &key)));
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
        drop(st);
        self.set_slot_base(r.slot, None);
    }

    /// The egui id base of the root mounted in `slot` (its scene root's `base`).
    fn set_slot_base(&self, slot: SlotId, base: Option<egui::Id>) {
        let mut scene = self.inner.backend.0.scene.borrow_mut();
        if let Ok(n) = scene.live(slot.0) {
            let r = scene.core[scene.ix(n)].root;
            scene.set_root_base(r, base);
        }
    }

    /// Disposes the root's JS (its natives, handlers, effects) and frees whatever is still under
    /// its slot. The root's table entry stays.
    fn dispose(&self, id: RootId) {
        let Some(slot) = self.inner.st.borrow().roots.get(&id).map(|r| r.slot) else { return };
        if let Err(e) = self.inner.host.unmount_root(id.0) {
            self.inner.st.borrow_mut().events.push(EmbedEvent::Error { root: Some(id), error: e });
        }
        // What a failed dispose left behind (a root interrupted mid-update) goes natively.
        let b = &self.inner.backend;
        let mut dead = Vec::new();
        {
            let mut scene = b.0.scene.borrow_mut();
            if let Ok(n) = scene.live(slot.0) {
                let kids = scene.children[scene.ix(n)].clone();
                for k in kids {
                    dead.extend(scene.free_subtree(k).into_iter().map(NodeId::to_raw));
                }
            }
        }
        if !dead.is_empty()
            && let Some(host) = b.host()
        {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h, if i == 0 { svelters_abi::SVL_GONE_OWNER } else { svelters_abi::SVL_GONE_PARENT });
            }
        }
    }

    fn do_destroy_slot(&self, slot: SlotId) {
        let Some(s) = self.inner.st.borrow_mut().slots.remove(&slot) else { return };
        self.inner.st.borrow_mut().slot_keys.remove(&s.key);
        if let Some(r) = s.root {
            self.do_unmount(r);
        }
        let b = &self.inner.backend;
        // frees the box and anything left under it, and ends its scene root
        let dead: Vec<Handle> = {
            let mut scene = b.0.scene.borrow_mut();
            match scene.live(slot.0) {
                Ok(n) => scene.free_subtree(n).into_iter().map(NodeId::to_raw).collect(),
                Err(_) => Vec::new(),
            }
        };
        if let Some(host) = b.host() {
            for (i, &h) in dead.iter().enumerate() {
                host.on_destroyed(h, if i == 0 { svelters_abi::SVL_GONE_OWNER } else { svelters_abi::SVL_GONE_PARENT });
            }
        }
    }

    /// Faults `id` now (in `logic`): disposes its JS and records the error.
    fn fault(&self, id: RootId, cause: FaultCause, error: JsError) {
        if !self.mark_faulted(id, cause, error) {
            return;
        }
        self.dispose(id);
    }

    /// Faults `id` outside `logic` (no JS may run): the dispose is queued.
    fn fault_later(&self, id: RootId, cause: FaultCause, error: JsError) {
        if self.mark_faulted(id, cause, error) {
            self.queue(Command::Dispose(id));
        }
    }

    fn mark_faulted(&self, id: RootId, cause: FaultCause, error: JsError) -> bool {
        let mut st = self.inner.st.borrow_mut();
        let Some(r) = st.roots.get_mut(&id) else { return false };
        if r.state.is_faulted() || matches!(r.state, RootState::Unmounting) {
            return false;
        }
        let err = RootError { cause, error };
        r.state = RootState::Faulted(err.clone());
        st.stats.faults += 1;
        st.events.push(EmbedEvent::Faulted { root: id, error: err });
        true
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

    /// The root that owns handle `h` (the node's scene root is one of our slots), if any.
    fn root_of_handle(&self, h: u64) -> Option<RootId> {
        let slot = {
            let scene = self.inner.backend.0.scene.borrow();
            let n = scene.live(h).ok()?;
            match scene.roots.get(scene.core[scene.ix(n)].root)?.kind {
                TreeRoot::Slot { node } => SlotId(node.to_raw()),
                _ => return None,
            }
        };
        self.inner.st.borrow().slots.get(&slot)?.root
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

fn viewport_name(v: egui::ViewportId) -> String {
    if v == egui::ViewportId::ROOT { "root".into() } else { format!("{:016x}", v.0.value()) }
}
