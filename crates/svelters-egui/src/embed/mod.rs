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
//! - **Viewports** (EM2, §5.2): a slot is drawn in whichever viewport the host says
//!   ([`Embed::set_viewport`]); repaints are routed per viewport (a change in window 2 repaints window
//!   2 only), focus follows the slot, and a deferred viewport's closure draws through a
//!   [`SlotPainter`] or [`with_current`] (`current.rs`, with the begin-pass plugin that runs `logic`
//!   before a deferred viewport's pass). [`Embed::move_root`] moves a root into another slot without
//!   a remount.
//!
//! Writer lane: EMBED.

mod current;
mod roots;
mod settle;
mod slots;

pub use current::{EmbedId, SlotPainter, show_viewport_passes, with_current, with_embed};
pub use roots::{FaultCause, RootError, RootId, RootInfo, RootOwner, RootState};
pub use settle::{Busy, BusyProbe, Settle};
pub use slots::{FaultUi, SlotId, SlotOptions, SlotResponse, SlotSizing, SlotWarning};

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
    /// Run `logic` from an egui begin-pass plugin before each pass of a deferred viewport (§5.2): input
    /// in that window is handled in its own next pass, with no root-viewport frame. `false` (a host
    /// that owns the per-frame order, cadatomic's seam 4): only the root viewport's `logic` runs JS,
    /// so input in a deferred window costs one root frame, whose logic routes the repaint back.
    pub prepass_plugin: bool,
    /// OW0: an app's change to the native behaviour policy ([`EmbedConfig::with_policy`]).
    pub policy: Option<fn(&mut crate::native::Policy)>,
    /// ZM1: who handles Ctrl+= / Ctrl+- / Ctrl+0 ([`ZoomKeys`]). Default `Host`: the embedder owns
    /// the shortcuts and calls [`Embed::set_zoom`]; egui's own handler is turned off. `app` sets
    /// `Embed` (steps of 1.2).
    pub zoom_keys: crate::zoom::ZoomKeys,
    /// U3 (UTILITY-CSS §4.6, UQ8): utility classes (`p-4 bg-surface hover:bg-list-hover`). Off by
    /// default; [`EmbedConfig::with_utilities`]. [`EguiBackend::set_utilities`] changes it later.
    pub utilities: crate::style::Utilities,
}

impl EmbedConfig {
    /// U3: utility classes for this embed: `cfg.with_utilities(Utilities::on())`, optionally with
    /// a prefix (`Utilities::on().prefix("tw")`: only `tw:p-4` is a utility) and the strict
    /// colour lint (`.strict_tokens(true)`).
    pub fn with_utilities(mut self, u: crate::style::Utilities) -> EmbedConfig {
        self.utilities = u;
        self
    }

    /// OW0 (WIDGET-PARITY §4.3): changes fields of the native behaviour policy of the embed's
    /// context (tooltip delay, drag threshold, button order…): `cfg.with_policy(|p|
    /// p.tooltip_delay_ms = 400)`. Applied by [`Embed::new`].
    pub fn with_policy(mut self, f: fn(&mut crate::native::Policy)) -> EmbedConfig {
        self.policy = Some(f);
        self
    }

    /// OW0 (WIDGET-PARITY §4.3, Q1): registers part painter `name` (CSS selects it with
    /// `background-image: paint(name)` or `content: paint(name)`). Painters are process-wide.
    pub fn painter(self, name: &str, p: impl crate::paint::PartPainter) -> EmbedConfig {
        crate::paint::register_painter(name, p);
        self
    }
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
            policy: None,
            zoom_keys: crate::zoom::ZoomKeys::Host,
            utilities: crate::style::Utilities::default(),
        }
    }
}

impl EmbedConfig {
    /// For an app bundle (the desktop runners, `TestApp`): the host's config as given, every entry
    /// under its `turn_budget` (as before EM1), quitting when the last window goes if it says so.
    /// ZM1: the embed handles the zoom keys (`ZoomKeys::Embed`, steps of 1.2).
    pub fn app(host: HostConfig) -> EmbedConfig {
        let b = host.turn_budget.unwrap_or(Duration::from_secs(24 * 3600));
        EmbedConfig { host, budgets: Budgets { warn: b, interrupt: b, mount: b }, zoom_keys: crate::zoom::ZoomKeys::Embed, ..EmbedConfig::default() }
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
    /// `move_root`s applied.
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
    /// A fault found outside `logic` (a panic in `show_slot`): dispose the root's JS.
    Dispose(RootId),
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
    /// The pass whose logic ran (viewport, its cumulative pass number): once per pass.
    last_pass: Option<(egui::ViewportId, u64)>,
    stats: EmbedStats,
    warnings: Vec<SlotWarning>,
    /// The viewports with a pass in the current outermost pass (it and its immediate children).
    pass_vps: Vec<egui::ViewportId>,
}

struct EmbedInner {
    id: EmbedId,
    // `host` first: it detaches from (and drops) its vtable before the backend handle below goes.
    host: Host<'static>,
    backend: EguiBackend,
    cfg: EmbedConfig,
    st: RefCell<State>,
    probe: BusyProbe,
    /// The context the salt was registered in (debug check), for `Drop`.
    salt_ctx: egui::Context,
    /// AS1: the asset registry of the provider loaded last.
    assets: RefCell<Option<std::sync::Arc<svelters_modules::Assets>>>,
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
        current::unregister(self.id);
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
        if let Some(f) = cfg.policy {
            crate::native::override_policy(ctx, f);
        }
        let backend = EguiBackend::new();
        backend.bind_ctx(ctx);
        backend.set_utilities(&cfg.utilities);
        let host = Host::new_owned(backend.clone(), cfg.host.clone()).map_err(EmbedError::Host)?;
        host.start().map_err(|e| EmbedError::Host(JsError::new(ErrorKind::Host, e.to_string())))?;
        host.set_turn_budget(Some(cfg.budgets.interrupt));
        let id = EmbedId::next();
        let inner = Rc::new(EmbedInner {
            id,
            host,
            backend,
            cfg,
            st: RefCell::new(State { next_root: 1, ..State::default() }),
            probe: BusyProbe::default(),
            salt_ctx: ctx.clone(),
            assets: RefCell::new(None),
        });
        current::register(id, &inner);
        let e = Embed { inner };
        e.attach(ctx);
        Ok(e)
    }

    /// Binds `ctx` (wakes and host ops repaint it) and registers the embed's pass callbacks on it:
    /// R17's phases and the begin-pass plugin (§5.2). `new` and every `logic` do it; call it yourself
    /// when the embed was created before the context it is drawn in exists (eframe's creation
    /// callback, a kittest harness's context setup), so that the first frame has them too.
    pub fn attach(&self, ctx: &egui::Context) {
        let b = &self.inner.backend;
        b.bind_ctx(ctx);
        crate::zoom::configure(ctx, self.inner.cfg.zoom_keys);
        b.0.scene.hook_passes(ctx);
        current::install(self, ctx);
        if let Some(a) = self.assets() {
            crate::assets::install(ctx, a);
        }
    }

    /// AS1: the asset registry of the module provider [`Embed::load_from`] / [`Embed::load_app_from`]
    /// loaded (`ModuleProvider::assets`), if it has one. Its loaders are installed on the embed's
    /// contexts (the one it was created in, and every one passed to [`Embed::attach`]).
    pub fn assets(&self) -> Option<std::sync::Arc<svelters_modules::Assets>> {
        self.inner.assets.borrow().clone()
    }

    fn adopt_assets(&self, provider: &dyn svelters_host::ModuleProvider) {
        if let Some(a) = provider.assets() {
            crate::assets::install(&self.inner.salt_ctx, a.clone());
            *self.inner.assets.borrow_mut() = Some(a);
        }
    }

    /// This embed's id in its thread's registry ([`with_embed`]).
    pub fn id(&self) -> EmbedId {
        self.inner.id
    }

    /// Makes this embed the one [`with_current`] resolves on this thread (default: the oldest live).
    pub fn make_current(&self) {
        current::make_current(self.inner.id);
    }

    /// A `Copy + Send + Sync` painter of `slot` for a deferred viewport's closure (§5.2).
    pub fn painter(&self, slot: SlotId) -> SlotPainter {
        SlotPainter::new(self.inner.id, slot)
    }

    pub fn has_slot(&self, slot: SlotId) -> bool {
        self.inner.st.borrow().slots.contains_key(&slot)
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
        self.loaded(r)
    }

    /// [`Embed::load`] from a module provider (Q1): an embed entry compiled from source
    /// (`svelters_host::modules::SourceProvider`).
    pub fn load_from(&self, provider: std::rc::Rc<dyn svelters_host::ModuleProvider>) -> Result<EntryId, JsError> {
        let host = &self.inner.host;
        self.adopt_assets(&*provider);
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_entry_from(provider);
        self.loaded(r)
    }

    fn loaded(&self, r: Result<u32, JsError>) -> Result<EntryId, JsError> {
        let host = &self.inner.host;
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

    /// [`Embed::load_app`] from a module provider (Q1): an app compiled from source.
    pub fn load_app_from(&self, provider: std::rc::Rc<dyn svelters_host::ModuleProvider>) -> Result<(), JsError> {
        let host = &self.inner.host;
        self.adopt_assets(&*provider);
        host.set_turn_budget(self.inner.cfg.host.turn_budget);
        let r = host.load_from(provider);
        host.set_turn_budget(Some(self.inner.cfg.budgets.interrupt));
        r
    }

    // ---- slots (no JS) ----

    /// Creates a slot (or returns the one with this key).
    pub fn create_slot(&self, key: impl Into<SlotKey>, opts: SlotOptions) -> SlotId {
        self.create_slot_as(key.into(), opts, None)
    }

    /// A slot, or (SG4) an overlay root on order `overlay`.
    fn create_slot_as(&self, key: SlotKey, opts: SlotOptions, overlay: Option<egui::Order>) -> SlotId {
        let key = key.0;
        if let Some(&s) = self.inner.st.borrow().slot_keys.get(&key) {
            return s;
        }
        let id = crate::ids::slot_id(self.salt(), &key);
        let box_kind = crate::kinds::tag_index("box").expect("the box kind");
        // A host-owned box (JS sees it borrowed), the top of its own scene root (ARCH-SCENE §4.5).
        let (h, rix) = {
            let mut scene = self.inner.backend.0.scene.borrow_mut();
            let n = scene.create(box_kind, Owner::Host).expect("a box");
            let i = scene.ix(n);
            scene.core[i].egui_id = id;
            let kind = match overlay {
                Some(layer) => TreeRoot::Overlay { node: n, layer },
                None => TreeRoot::Slot { node: n },
            };
            let rix = scene.add_root(kind, n);
            (n.to_raw(), rix)
        };
        let slot = SlotId(h);
        self.inner.backend.0.root_vp.borrow_mut().insert(rix, Some(egui::ViewportId::ROOT));
        let scale = self.inner.backend.ctx().map(|c| c.pixels_per_point()).unwrap_or(1.0);
        let mut st = self.inner.st.borrow_mut();
        st.slot_keys.insert(key.clone(), slot);
        st.slots.insert(slot, Slot::new(key, opts, rix, scale));
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

    /// Visibility comes from the host (the dock knows it exactly; D7). Hidden: no layout, walk or
    /// repaint, its animation frames wait (also those requested from its event handlers), and JS
    /// sees `root().visible = false` at the next logic. Timers, effects and store deliveries keep
    /// running (§5.3).
    pub fn set_visible(&self, slot: SlotId, visible: bool) {
        let mut st = self.inner.st.borrow_mut();
        let Some(s) = st.slots.get_mut(&slot) else { return };
        if s.visible == visible {
            return;
        }
        s.visible = visible;
        s.undrawn = 0;
        let (vp, rix) = (s.viewport, s.rix);
        drop(st);
        self.inner.backend.0.root_vp.borrow_mut().insert(rix, visible.then_some(vp));
        // journaled; hit-testing skips a hidden root (ARCH-SCENE SG3)
        self.inner.backend.0.scene.borrow_mut().root_changed(rix, crate::scene::RootChange::Visible(visible));
        self.wake_viewport(vp);
    }

    /// The slot is drawn in `viewport` from now on (§7.1): (a) its root's changes repaint that
    /// viewport only, (b) the node that had keyboard focus inside the slot gets it again at the
    /// slot's next draw (egui keeps focus per viewport), (c) `root().viewport` and `root().scale`
    /// follow (the layout event, SV18). Nothing moves in the scene: ids, egui memory (caret,
    /// scroll), JS instances and `$state` are untouched.
    pub fn set_viewport(&self, slot: SlotId, viewport: egui::ViewportId) {
        let mut st = self.inner.st.borrow_mut();
        let Some(s) = st.slots.get_mut(&slot) else { return };
        if s.viewport == viewport {
            return;
        }
        let old = s.viewport;
        s.viewport = viewport;
        s.undrawn = 0;
        s.warned &= !slots::WARN_VIEWPORT;
        if s.focus.is_some() {
            s.pending_focus = s.focus;
        }
        let (visible, rix) = (s.visible, s.rix);
        drop(st);
        self.inner.backend.0.root_vp.borrow_mut().insert(rix, visible.then_some(viewport));
        // journaled; per-root damage and R20 (hit_test skips the old viewport's entries) read it
        self.inner.backend.0.scene.borrow_mut().root_changed(rix, crate::scene::RootChange::Viewport(viewport));
        if let Some(ctx) = self.inner.backend.ctx() {
            ctx.request_repaint_of(old);
        }
        self.wake_viewport(viewport);
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

    /// Moves a root into another, empty slot without a remount (§7.1: a pane body ↔ an extension
    /// window): at the next logic its nodes are reparented under `to` (same egui ids, so caret,
    /// scroll and focus stay; same JS instances) and the runtime appends to `to` from then on. `to`
    /// is reserved for the root now; the old slot is empty after the move.
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
        drop(st);
        self.repaint();
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
        self.repaint();
    }

    /// Queued work: a pass of the viewport that runs logic next (the root viewport, or the deferred
    /// viewport whose pass is running with the begin-pass plugin).
    fn repaint(&self) {
        if let Some(ctx) = self.inner.backend.ctx() {
            ctx.request_repaint_of(self.inner.backend.logic_vp());
        }
    }

    /// A slot of `vp` changed in a way its next draw and the next logic both see (visibility,
    /// viewport): a pass of `vp`, and of the root viewport too when `vp`'s passes run no logic.
    fn wake_viewport(&self, vp: egui::ViewportId) {
        let Some(ctx) = self.inner.backend.ctx() else { return };
        ctx.request_repaint_of(vp);
        if vp != egui::ViewportId::ROOT && !self.inner.cfg.prepass_plugin {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
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
        // pass numbers are per viewport: a deferred viewport's pass N is not the root's pass N
        let pass = (ctx.viewport_id(), ctx.cumulative_pass_nr());
        {
            let mut st = self.inner.st.borrow_mut();
            if st.last_pass == Some(pass) {
                b.0.scene.set_phase(Phase::Ui);
                return;
            }
            st.last_pass = Some(pass);
        }
        let t0 = Instant::now();
        self.attach(ctx);
        b.0.scene.set_phase(Phase::Logic);
        b.0.in_frame.set(true);
        self.apply_commands();
        self.sync_info(ctx);
        b.0.in_frame.set(false);
        // ZM1: the runner's zoom keys (the host's own keymap otherwise); `logic_pass` applies it
        if self.inner.cfg.zoom_keys == crate::zoom::ZoomKeys::Embed {
            crate::zoom::handle_keys(ctx, b);
        }
        // checkpoint (events, changes, timers), rAF, remeasure: every entry under the handler budget
        b.logic_pass(ctx);
        self.collect();
        // faults found above dispose their roots now
        b.0.in_frame.set(true);
        self.apply_commands();
        b.0.in_frame.set(false);
        b.after_pass(ctx, Vec::new());
        if !self.inner.st.borrow().commands.is_empty() {
            self.repaint();
        }
        // step 7: every viewport showing a root that changed, except the one this pass draws
        // (a logic-only frame draws none)
        let drawing = b.0.scene.in_pass().then(|| ctx.viewport_id());
        b.route_repaints(ctx, drawing);
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
            self.warn_once(slot, slots::WARN_HIDDEN, |key| SlotWarning::DrawnWhileHidden { slot, key });
            return SlotResponse { rect: egui::Rect::NOTHING, content_size: egui::Vec2::ZERO, state: RootState::Hidden };
        }
        let pending_focus = self.note_drawn(ui, slot);
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
        // focus restore after a viewport change: before the walk, so the widget draws focused
        if let Some(id) = pending_focus {
            ui.memory_mut(|m| m.request_focus(id));
        }
        b.0.in_frame.set(true);
        b.0.pass_active.set(true);
        let mut out = Vec::new();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| b.slot_ui(&mut child, slot.0, &mut out)));
        b.0.pass_active.set(false);
        ui.advance_cursor_after_rect(rect);
        let (drawn, content) = match res {
            Ok(x) => {
                // SG4, §7.2 step e: the root's scene passes (and after_layout of a fresh layout)
                let rix = self.inner.st.borrow().slots.get(&slot).map(|s| s.rix);
                if let Some(rix) = rix {
                    crate::provider::pass::run_root_passes(&b.0.scene, ui, rix, x.0);
                }
                x
            }
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
        self.note_focus(ui, slot);
        if resized && root.is_some() {
            // `root().width/height` follow at the next logic
            self.repaint();
        }
        self.publish();
        SlotResponse { rect: drawn, content_size: content, state }
    }

    /// The debug checks and the per-viewport fields of a slot being drawn in `ui` (visible): marks it
    /// drawn this pass, checks its viewport, follows a scale change (`root().scale`, one relayout of
    /// this slot only). Returns the focus to restore, if any.
    fn note_drawn(&self, ui: &egui::Ui, slot: SlotId) -> Option<egui::Id> {
        let vp = ui.ctx().viewport_id();
        let ppp = ui.ctx().pixels_per_point();
        let (wrong, rescaled, focus, rix) = {
            let mut st = self.inner.st.borrow_mut();
            let s = st.slots.get_mut(&slot)?;
            s.drawn = true;
            s.undrawn = 0;
            s.warned &= !(slots::WARN_UNDRAWN | slots::WARN_HIDDEN);
            let rescaled = s.scale != ppp;
            s.scale = ppp;
            ((s.viewport != vp).then_some(s.viewport), rescaled, s.pending_focus.take(), s.rix)
        };
        if let Some(set) = wrong {
            self.warn_once(slot, slots::WARN_VIEWPORT, |key| SlotWarning::WrongViewport { slot, key, set, drawn: vp });
        }
        if rescaled {
            self.inner.backend.0.scene.borrow_mut().mark_full(rix);
            self.repaint();
        }
        focus
    }

    /// After a draw: which node inside the slot has keyboard focus in this viewport (for focus
    /// restore and `root().focused`). Looks the focused id up only when it changed.
    fn note_focus(&self, ui: &egui::Ui, slot: SlotId) {
        let f = ui.memory(|m| m.focused());
        let rix = {
            let st = self.inner.st.borrow();
            let Some(s) = st.slots.get(&slot) else { return };
            if s.focus_seen == f {
                return;
            }
            s.rix
        };
        let inside = f.filter(|&id| {
            let scene = self.inner.backend.0.scene.borrow();
            scene.ids().any(|n| {
                let i = scene.ix(n);
                scene.core[i].root == rix && scene.core[i].egui_id == id
            })
        });
        let changed = match self.inner.st.borrow_mut().slots.get_mut(&slot) {
            Some(s) => {
                s.focus_seen = f;
                let changed = s.focus.is_some() != inside.is_some();
                s.focus = inside;
                changed && s.root.is_some()
            }
            None => false,
        };
        if changed {
            self.repaint(); // `root().focused` follows at the next logic
        }
    }

    /// Records a debug-check warning once per slot and kind (until the slot behaves again).
    fn warn_once(&self, slot: SlotId, bit: u8, w: impl FnOnce(String) -> SlotWarning) {
        let mut st = self.inner.st.borrow_mut();
        let Some(s) = st.slots.get_mut(&slot) else { return };
        if s.warned & bit != 0 {
            return;
        }
        s.warned |= bit;
        let w = w(s.key.clone());
        #[cfg(debug_assertions)]
        if self.inner.cfg.host.echo_errors {
            eprintln!("[svelters-egui] slot check: {w:?}");
        }
        st.warnings.push(w);
    }

    /// What the debug checks found since the last call (ARCH-EMBED §5.3): a slot drawn while set
    /// hidden, a visible slot not drawn for 60 passes of its viewport, a slot drawn in a viewport
    /// other than the one set. Also the font problems found since the last call (`SlotWarning::Font`,
    /// from `crate::fonts::take_warnings`).
    pub fn take_warnings(&self) -> Vec<SlotWarning> {
        let mut w = std::mem::take(&mut self.inner.st.borrow_mut().warnings);
        w.extend(crate::fonts::take_warnings().into_iter().map(SlotWarning::Font));
        w
    }

    // ---- the pass plugin (current.rs) ----

    /// Every pass of every viewport, before the host's ui (`outer`: not an immediate viewport
    /// nested in another pass). A deferred viewport's outermost pass runs `logic` first when the
    /// prepass plugin is on, and pending work then asks for that viewport's next pass.
    pub(super) fn begin_pass(&self, ctx: &egui::Context, outer: bool) {
        let vp = ctx.viewport_id();
        {
            let mut st = self.inner.st.borrow_mut();
            if outer {
                st.pass_vps.clear();
            }
            if !st.pass_vps.contains(&vp) {
                st.pass_vps.push(vp);
            }
        }
        if !outer {
            return;
        }
        let deferred = vp != egui::ViewportId::ROOT;
        let logic_here = deferred && self.inner.cfg.prepass_plugin;
        self.inner.backend.set_logic_vp(if logic_here { vp } else { egui::ViewportId::ROOT });
        if logic_here {
            self.logic(ctx);
        }
    }

    /// The end of every pass (`outer` as above): the "not drawn" check for the slots of the
    /// viewports that had a pass, then pending work goes back to the root viewport.
    pub(super) fn end_pass(&self, _ctx: &egui::Context, outer: bool) {
        if !outer {
            return;
        }
        let mut late = Vec::new();
        {
            let mut st = self.inner.st.borrow_mut();
            let vps = std::mem::take(&mut st.pass_vps);
            for (&id, s) in st.slots.iter_mut() {
                if !vps.contains(&s.viewport) {
                    continue;
                }
                if s.visible && s.root.is_some() && !s.drawn {
                    s.undrawn += 1;
                    if s.undrawn >= slots::UNDRAWN_PASSES {
                        late.push((id, s.undrawn));
                    }
                }
                s.drawn = false;
            }
        }
        for (slot, passes) in late {
            self.warn_once(slot, slots::WARN_UNDRAWN, |key| SlotWarning::NotDrawn { slot, key, passes });
        }
        self.inner.backend.set_logic_vp(egui::ViewportId::ROOT);
    }

    /// ARCH-SCENE R21: the host calls this at the end of each viewport's ui (the root viewport's
    /// `App::ui` and every deferred viewport's closure), after berth drew its panes, so that
    /// `PassScope::Viewport`/`AllViewports` scene passes (overlays over every pane of the window)
    /// run once per viewport. EM2 lands the call site; the passes themselves are SG4's
    /// (`provider/pass.rs`): until then it only publishes the busy probe.
    ///
    /// SG4: runs those passes over `ui`'s max rect (each on its own layer, `ScenePass::order`).
    /// Returns how many ran. From a deferred viewport's `Send + Sync` closure, use
    /// [`show_viewport_passes`](crate::embed::show_viewport_passes) (the thread's current embed).
    pub fn show_viewport_passes(&self, ui: &mut egui::Ui) -> usize {
        let n = crate::provider::pass::run_viewport_passes(&self.inner.backend.0.scene, ui);
        self.publish();
        n
    }

    /// SG4 (R23): registers a state provider (`SceneHandle::register_state_provider`).
    pub fn register_state_provider<P: crate::provider::StateProvider>(&self, p: P) -> crate::provider::ProviderHandle<P> {
        self.inner.backend.0.scene.register_state_provider(p)
    }

    /// SG4: registers a scene pass (`SceneHandle::register_pass`).
    pub fn register_pass<P: crate::provider::ScenePass>(&self, p: P) -> crate::provider::PassHandle<P> {
        self.inner.backend.0.scene.register_pass(p)
    }

    /// EV1 (SV6): a Rust event filter on this embed's scene ([`crate::events::EventFilter`]).
    pub fn register_filter(&self, scope: crate::events::FilterScope, f: impl crate::events::EventFilter) -> crate::events::FilterHandle {
        self.inner.backend.0.scene.register_filter(scope, f)
    }

    /// EV1 (SV7): an overlay painted after the walk ([`crate::events::Overlay`]).
    pub fn register_overlay<O: crate::events::Overlay>(&self, o: O) -> crate::events::OverlayHandle<O> {
        self.inner.backend.0.scene.register_overlay(o)
    }

    /// SG4 (ARCH-SCENE §7.5): an overlay root, a slot drawn with [`Embed::show_overlay`] in an
    /// `egui::Area` of order `layer` over other content (a 3D viewport, a canvas). Mount into it
    /// like any slot (JS), or build its content from Rust (`SceneMut`, the slot's node is
    /// Host-owned). Returns the existing slot for a key in use.
    pub fn create_overlay(&self, key: impl Into<SlotKey>, layer: egui::Order, opts: SlotOptions) -> SlotId {
        self.create_slot_as(key.into(), opts, Some(layer))
    }

    /// SG4: draws overlay `slot` at `rect` (screen points) above what is under it. The area is
    /// click-through, so the content under it keeps receiving pointer input, except while the
    /// pointer is over one of the overlay's nodes that takes input (a button, a field, a focusable
    /// provider kind), as laid out and drawn in the last pass: then that node gets it. A slot made
    /// with `create_slot` is drawn the same way, on the `Foreground` order.
    pub fn show_overlay(&self, ctx: &egui::Context, slot: SlotId, rect: egui::Rect) -> SlotResponse {
        let (layer, rix, id) = {
            let scene = self.inner.backend.0.scene.borrow();
            let Some(n) = scene.get(slot.0) else {
                return SlotResponse { rect: egui::Rect::NOTHING, content_size: egui::Vec2::ZERO, state: RootState::Empty };
            };
            let rix = scene.core[scene.ix(n.handle)].root;
            let layer = match scene.roots.get(rix).map(|e| e.kind) {
                Some(TreeRoot::Overlay { layer, .. }) => layer,
                _ => egui::Order::Foreground,
            };
            (layer, rix, n.id.with("overlay"))
        };
        let captures = ctx.pointer_hover_pos().is_some_and(|p| self.overlay_takes_input(rix, ctx.viewport_id(), p));
        egui::Area::new(id)
            .order(layer)
            .fixed_pos(rect.min)
            .constrain(false)
            .interactable(captures)
            .sense(egui::Sense::hover())
            .show(ctx, |ui| {
                ui.set_min_size(rect.size());
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                self.show_slot(&mut child, slot)
            })
            .inner
    }

    /// True if the topmost node of overlay root `rix` at `pos` (last pass's hit list) takes input.
    fn overlay_takes_input(&self, rix: crate::scene::RootIx, vp: egui::ViewportId, pos: egui::Pos2) -> bool {
        let Ok(scene) = self.inner.backend.0.scene.try_borrow() else { return false };
        let s = crate::scene::SceneRef::new(&scene);
        s.hit_test(vp, pos).into_iter().filter_map(|t| s.get(t.node())).find(|n| n.root() == rix).is_some_and(|n| takes_input(n.builtin()))
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
        // SG4, R16/R8: a state provider that wants (or was kicked), or a provider widget at work
        busy.set(Busy::PROVIDER, crate::provider::pass::providers_busy(&b.0.scene) || crate::provider::pass::widgets_busy(&b.0.scene.borrow()));
        let now = Instant::now();
        let next = [hb.next_timer, b.0.deadline.get()].into_iter().flatten().min();
        busy.set(Busy::TIMER_SOON, next.is_some_and(|t| t <= now + self.inner.cfg.timer_horizon));
        Settle { busy, next_timer: next }
    }

    /// A `Send + Sync` mirror of [`Embed::settled`] for idle barriers on other threads.
    pub fn busy_probe(&self) -> BusyProbe {
        self.inner.probe.clone()
    }

    /// EV3 (QT-SURVEY §2.3): what has keyboard focus in `ctx`'s current viewport, when it is a node
    /// of this embed, with its root. cadatomic's `TextFocus` layer reads it.
    pub fn focus_info(&self, ctx: &egui::Context) -> crate::focus::FocusInfo {
        self.with_root(self.backend().focus_info(ctx))
    }

    /// EV3b: the text field of this embed that [`Embed::text_command`] acts on, with its root: the
    /// focused one, else the last one that had focus while nothing or a menu item has it (a click
    /// on host chrome, the Edit menu open), so the Edit menu enables Cut, Copy and Paste. A
    /// built-in `text-edit`, an own text widget or a provider kind whose `Widget::text_input` is
    /// `Some`. `None` once another widget took focus, or the field was removed, hidden or disabled.
    pub fn last_text_field(&self, ctx: &egui::Context) -> Option<crate::focus::FocusInfo> {
        self.backend().last_text_field(ctx).map(|i| self.with_root(i))
    }

    /// `info` with the root of its node filled in.
    fn with_root(&self, mut info: crate::focus::FocusInfo) -> crate::focus::FocusInfo {
        if let Some(node) = info.node {
            let top = {
                let scene = self.backend().0.scene.borrow();
                let mut at = node;
                while let Some(p) = scene.idx(at).and_then(|i| scene.core[i].parent) {
                    at = p;
                }
                at
            };
            info.root = self.inner.st.borrow().slots.get(&SlotId(top.to_raw())).and_then(|s| s.root);
        }
        info
    }

    /// EV3, Qt's `ShortcutOverride` (ARCH-EMBED R11 `consume_key`): true when the focused node of
    /// this embed takes `chord` itself (Ctrl+Z, Del and the arrows in a text field; every key while
    /// an input method composes), so the host keymap must leave it. Keys the focused node took in
    /// the current pass are also in [`crate::focus::consumed_keys`].
    pub fn shortcut_override(&self, ctx: &egui::Context, chord: &egui::KeyboardShortcut) -> bool {
        self.backend().shortcut_override(ctx, chord)
    }

    /// EV3b: [`Embed::shortcut_override`] for a press with its logical (typed) and physical key,
    /// for a keymap with cadatomic KL1's Latin fallback: the typed key wins, and a Ctrl or Cmd
    /// chord whose logical key is a symbol or a non-Latin letter is also asked with the physical
    /// (Latin) key. True when the focused node claims either.
    pub fn shortcut_override_press(&self, ctx: &egui::Context, logical: egui::Key, physical: Option<egui::Key>, modifiers: egui::Modifiers) -> bool {
        self.backend().shortcut_override_press(ctx, logical, physical, modifiers)
    }

    /// EV3 (QT-SURVEY §2.3 `text_command`): an Edit-menu command for the focused text field of this
    /// embed (or the last one, when a click on host chrome or a menu took focus: see
    /// [`Embed::last_text_field`]). Cut, Copy and Paste use the host's clipboard (DX2
    /// `Host::platform()`, the one `svelters:clipboard` uses); Cut and Copy also go to egui's. A
    /// provider kind with `Widget::text_input` gets it through `Widget::text_command` first (EV3b).
    /// False when there is no such field: the host runs its own command.
    pub fn text_command(&self, ctx: &egui::Context, cmd: crate::focus::TextCommand) -> bool {
        // DX2: the clipboard JS uses (`Host::platform()`)
        let clip = crate::focus::Clip(self.host().platform().clipboard.clone());
        self.backend().text_command_with(ctx, cmd, Some(clip))
    }

    /// R11 `composing()`: an input method is composing in a svelters field of `ctx`'s viewport.
    pub fn composing(&self, ctx: &egui::Context) -> bool {
        crate::focus::composing(ctx)
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
        // the root's anchor map (ARCH-SCENE SG3)
        crate::scene::SceneRef::new(&scene).by_anchor(r, anchor).map(|n| n.id().to_raw())
    }

    /// Exports, op log, stats, interrupt flag: the underlying host.
    pub fn host(&self) -> &Host<'static> {
        &self.inner.host
    }

    /// ZM1: sets the UI zoom factor, multiplied with the OS scale ([`EguiBackend::set_zoom`]).
    /// cadatomic maps its zoom levels to factors (`crate::zoom::level_factor`: 1.2^n).
    pub fn set_zoom(&self, zoom: f32) {
        self.inner.backend.set_zoom(zoom);
    }

    /// ZM1: the UI zoom factor ([`EguiBackend::zoom`]).
    pub fn zoom(&self) -> f32 {
        self.inner.backend.zoom()
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
        SentInfo {
            key: r.key.0.clone(),
            visible: s.is_some_and(|s| s.visible),
            width: size.x as f64,
            height: size.y as f64,
            // the scale of the viewport the slot was last drawn in (EM2: per viewport)
            scale: s.map(|s| s.scale).unwrap_or(1.0) as f64,
            viewport: s.map(|s| viewport_name(s.viewport)).unwrap_or_default(),
            focused: s.is_some_and(|s| s.focus.is_some()),
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
                Command::Move(id, to) => self.do_move(id, to),
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
        self.set_slot_base(slot, Some(&key));
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

    /// The egui id base of the root mounted in `slot` under `key` (its scene root's `base`), and
    /// the key itself (`SceneRef::by_path`).
    fn set_slot_base(&self, slot: SlotId, key: Option<&str>) {
        let base = key.map(|k| crate::ids::root_base(self.salt(), k));
        let mut scene = self.inner.backend.0.scene.borrow_mut();
        if let Ok(n) = scene.live(slot.0) {
            let r = scene.core[scene.ix(n)].root;
            scene.set_root_base(r, base);
            scene.set_root_key(r, key);
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

    /// `move_root` (queued): the root's nodes go under `to` with the same egui ids (the root's id
    /// base moves with them), the runtime's mount target follows (`__svelters_embed.retarget`), the
    /// focus inside it is restored at `to`'s next draw.
    fn do_move(&self, id: RootId, to: SlotId) {
        let (from, key, live) = {
            let mut st = self.inner.st.borrow_mut();
            let Some(r) = st.roots.get(&id) else {
                // unmounted meanwhile: free the reservation
                if let Some(t) = st.slots.get_mut(&to).filter(|t| t.root == Some(id)) {
                    t.root = None;
                }
                return;
            };
            let (from, key, live) = (r.slot, r.key.0.clone(), r.state.is_live());
            if from == to || st.slots.get(&to).is_none_or(|t| t.root != Some(id)) {
                return; // `to` was destroyed (or the move is stale)
            }
            (from, key, live)
        };
        self.set_slot_base(to, Some(&key));
        {
            let mut scene = self.inner.backend.0.scene.borrow_mut();
            if let (Ok(f), Ok(t)) = (scene.live(from.0), scene.live(to.0)) {
                let kids = scene.children[scene.ix(f)].clone();
                for k in kids {
                    // on the runtime's behalf: it is told below (its nodes are runtime-owned)
                    let _ = scene.insert(t, k, None, crate::scene::Origin::Js);
                }
            }
        }
        self.set_slot_base(from, None);
        if live && let Err(e) = self.inner.host.retarget_root(id.0, to.0) {
            self.on_call_error(id, e);
        }
        let vps = {
            let mut st = self.inner.st.borrow_mut();
            if let Some(r) = st.roots.get_mut(&id) {
                r.slot = to;
            }
            st.stats.moves += 1;
            let (old_vp, focus) = match st.slots.get_mut(&from) {
                Some(f) => {
                    f.root = None;
                    let focus = f.focus.take();
                    f.focus_seen = None;
                    (f.viewport, focus)
                }
                None => (egui::ViewportId::ROOT, None),
            };
            let t = st.slots.get_mut(&to).unwrap();
            t.pending_focus = focus.or(t.pending_focus);
            t.rect = None; // laid out at its next draw
            [old_vp, t.viewport]
        };
        if let Some(ctx) = self.inner.backend.ctx() {
            for vp in vps {
                ctx.request_repaint_of(vp);
            }
        }
    }

    fn do_destroy_slot(&self, slot: SlotId) {
        let Some(s) = self.inner.st.borrow_mut().slots.remove(&slot) else { return };
        self.inner.st.borrow_mut().slot_keys.remove(&s.key);
        self.inner.backend.0.root_vp.borrow_mut().remove(&s.rix);
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
            SlotId(scene.roots.get(scene.core[scene.ix(n)].root)?.kind.slot_node()?.to_raw())
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

/// SG4: a node that takes pointer input in an overlay (the overlay's area turns interactable over it).
fn takes_input(w: &crate::scene::W) -> bool {
    use crate::scene::W;
    match w {
        W::Button { .. } | W::Checkbox { .. } | W::TextEdit { .. } => true,
        // scrolls and selects (LS1)
        W::VirtualList(_) => true,
        W::Run { anchor, .. } => *anchor,
        W::Label { selectable, rich, .. } => *selectable || rich.as_ref().is_some_and(|r| r.runs.iter().any(|x| x.link.is_some())),
        W::Ext(x) => {
            let c = x.entry().caps;
            // OW2: the own scroll area, split view and scroll bar scroll and drag; a panel when resizable
            c.contains(crate::provider::Caps::FOCUSABLE)
                || c.contains(crate::provider::Caps::EDITABLE)
                || matches!(x.entry().replaces, Some("scroll-area" | "split-view" | "scrollbar"))
                || x.with(|p: &crate::widgets::panel::Panel| p.is_resizable()).unwrap_or(false)
        }
        W::Box | W::Window { .. } => false,
        // the WIDGETS lanes' kinds (WG1, WG2a, WG2b, WG4a, WG4b): decoration and plain layout take none
        W::Separator { .. }
        | W::Grid { .. }
        | W::Sides
        | W::Frame { .. }
        | W::Stack { .. }
        | W::Image { .. }
        | W::ProgressBar { .. }
        | W::Spinner { .. }
        | W::TableColumn { .. }
        | W::Tab { .. } => false,
        W::Icon { frame, .. } => *frame,
        W::ToggleButton { .. }
        | W::ComboBox { .. }
        | W::CollapsingHeader { .. }
        | W::Menu(_)
        | W::Overlay(_)
        | W::Hyperlink { .. }
        | W::RadioButton { .. }
        | W::Switch { .. }
        | W::Canvas(_)
        | W::Table(_)
        | W::TabView { .. }
        | W::ToastHost(_)
        | W::Toast(_)
        | W::DialogButtons(_) => true,
    }
}

fn viewport_name(v: egui::ViewportId) -> String {
    if v == egui::ViewportId::ROOT { "root".into() } else { format!("{:016x}", v.0.value()) }
}
