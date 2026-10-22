# ARCH-EMBED: svelters-egui as an embedded library, and how Rust extends it

Status: architecture proposal (research, no code). 2026-10-22. svelters master `d1cfb14d`, cadatomic master
`92fae3f`. Scope: the two svelters tracks that SC1 needs first. **EMBED** covers svelters-egui as a library inside
a host's egui frame. **EXTEND** covers Rust crates extending the compiler, runtime and egui backend. The dock and
code-editor crates are cadatomic's; this document only specifies the seams svelters offers them (§7).

Supersedes nothing. Refines TRACKS.md (EMBED EG0/EM1–EM3, RUSTAPI RA1–RA3, part of EVENTS EV1/EV2),
CADATOMIC.md §2.1/§2.5/§2.8 (SV1, SV2, SV4, SV9, SV10, SV16–SV19, SV35, SVP1–SVP5) and `docs/notes/egui-rust-exports.md`
§7 (`#[svelters::widget]`).

Short names for cadatomic sources (all under `~/projects/cadatomic/docs/`):

| Name | File |
|---|---|
| docking | `research/2026-10-22-docking-windows-and-svelters-roots.md` |
| framework | `research/2026-10-22-component-framework.md` |
| scene-node | `research/2026-10-22-scene-node-and-svelte-semantics.md` |
| js-api | `research/2026-10-22-js-api-publishing-and-live-ide.md` |
| editor | `research/2026-10-22-code-editor-and-lsp.md` |
| native-ext | `research/2026-10-22-native-extensions-abi-and-engines.md` |
| slint | `research/2026-10-22-slint-lessons.md` |
| widgets | `research/2026-10-22-widget-survey-and-native-acceptance.md` |
| models | `research/2026-10-22-models-filtering-and-dynamic-data.md` |
| shell | `research/2026-10-22-ide-shell-and-ux.md` |
| decisions / overview / experiments | `plan/00-decisions.md`, `plan/01-overview.md`, `plan/05-experiments-and-research.md` |
| requests | `plan/04-kernel-requests.md` (SV table) |

Docking's experiments W1–W13 are written **DOCK-W*n*** here. workspace-document-versioning reuses the names W1–W6
for requirements, so the prefix keeps them apart.

## 0. Summary

1. **A slot is a free-standing root of the arena, drawn wherever the host calls `show_slot(ui, slot)`.** Rust
   creates slots, and JS mounts components into them with the existing `mount(Component, { target })`
   (`packages/native-runtime/src/index.js:1308`). A slot has no parent in the scene. The dock pane, the floating
   window or the viewport that shows it this frame is whatever `Ui` the host passes. **Moving a pane therefore
   moves nothing**: the same `show_slot` call simply comes from another pane or viewport. Node ids derive from
   the embed salt plus the root key, never from the `Ui`, so egui memory, AccessKit ids, focus targets and JS
   instances all survive. This meets SV18 and DOCK-W4 more simply than docking §6.2's "reparent the slot node".
2. **One owned handle, `Embed`, with three call sites.**
   - `embed.logic(ctx)` from `App::logic`: closes, the JS checkpoint, rAF, queued mounts and props, remeasure,
     repaint routing.
   - `embed.show_slot(ui, slot)` from `App::ui` or any viewport callback: layout if dirty, the walk, the outbox.
   - An egui `on_begin_pass` plugin that runs the same `logic` before a *deferred* viewport's pass. eframe calls
     `App::logic` only for the root viewport (`eframe-0.37.0/src/native/epi_integration.rs:349-358`).

   JS runs only in `logic`. Every mount, props update, unmount and close request from Rust is queued and applied
   there. That is cadatomic's rule (overview §3 P3; SCOPE §6.2 R3/R4).
3. **Many roots, one QuickJS runtime, isolated by root.**
   - Render and effect errors are caught by a per-root boundary. They fault that root only, and the slot draws a
     Rust error state.
   - A handler exception is attributed to the root of the handle it was dispatched on.
   - A runaway handler is interrupted by a per-entry budget (warn at 8 ms, interrupt at 50 ms, per docking §6.4)
     and charged to its root.
   - Only runtime-wide faults (OOM, stack overflow) fault every root. Recovery then restarts the runtime with the
     reload machinery and remounts every root into the same slots.
   - `RootOwner` reserves isolates (SV15) without building them.
4. **Idle is 0 frames.**
   - Repaints are routed per viewport through the slots each change touched (`request_repaint_of`).
   - Hidden slots get no layout, no walk and no rAF callbacks.
   - `settled()` reports *why* it is busy, as a bit set. A `Send + Sync` probe lets cadatomic's `IdleBarrier`
     register it as a busy source (`add_busy_source` needs `Send + Sync`).
5. **EXTEND is one registry and one macro family.**
   - `#[svelters::widget]` registers a kind through `inventory`. Its props are typed with the same `JsType`
     schema the E0 exports use, and its behaviour implements `svelters_egui::Widget` (`measure`, `ui`, `a11y`).
   - A provider can attach typed per-node data and register named state bits.
   - Native behaviours are *attached kinds* (the runtime already has them, `index.js:240`), reached through
     ordinary `use:` actions imported from a generated module. No compiler hook is needed.
   - The compiler takes a **spec fragment** (`--spec-fragment`, merged into the built-in egui spec). Typings come
     from the same dump that writes `exports.json` and `app.d.ts`.
6. **`on:` forwarding is a lowering.** A bare `on:name` inside a component, on an element or on a child
   component, lowers to `onname={$$props.onname}` and declares the callback prop in the component's typings
   (user decision; TRACKS EV1). There are no legacy component events.
7. **Milestones, in lane-days.**
   - EG0 split: 1.
   - EM1, the SC1 core: 6–8.
   - RA1 kind providers: 5–6.
   - EM2 viewports and moves: 5–6.
   - FW1 `on:` forwarding: 1–2.
   - RA2 `Json<T>` and data enums: 4–5.
   - EV2b native behaviours: 3–4.
   - EM3 adaptive hidden roots: 4–5, after HMR H3.

   Twelve decisions are open (§13), each with a recommendation; §12 lists what is deliberately deferred.

## 1. Problem

cadatomic's UI is egui 0.37. Rust owns the shell: dock, tabs, toolbars, menus and windows. Svelte renders the
contents of panels, dialogs, windows and overlays, with many roots at once (svelters CLAUDE.md "Current focus";
shell §9, Rust/svelters split; decisions "Shell answers"). svelters-egui today is a *program*, not a library:

- it draws presented `window` elements, the first in a `CentralPanel` and the rest as immediate viewports
  (`crates/svelters-egui/src/backend.rs:310-343`);
- it runs JS from inside the egui pass's caller (`backend.rs:206-223`, `before_pass` on pass 0);
- its node ids would collide between two embeds in one `Context` (`scene.rs:240`, `Id::unique(("svl", handle))`;
  both arenas start their handles at the same values);
- its vocabulary is a closed static table (`kinds.rs:285`, `pub static KINDS`) with a closed enum of widgets
  (`scene.rs:17-25`, `enum W`).

cadatomic needs:

- slots it creates;
- many mounts per runtime;
- moves between panes and OS windows without remount (DOCK-W4);
- per-root visibility and error isolation;
- an idle barrier (overview §3 P10);
- Rust components usable as Svelte elements (SV1), native behaviours (SV2) and provider state (SV4).

## 2. Constraints

Decided by the user (svelters CLAUDE.md, 2026-10-21; not relitigated here):

- **C-1** Stock QuickJS-NG (rquickjs 0.14.0). No fork, no determinism work.
- **C-2** A bare `on:event` inside a component forwards an `on<event>` callback prop. No legacy component events.
- **C-3** One retained scene, owned by svelters-egui. cadatomic extends it through kind, behaviour and state
  providers.
- **C-4** egui stays at 0.37 and is upgraded together with cadatomic.
- **C-5** The Svelte UI sources live in cadatomic.
- **C-6** The Rust CSS engine and the Rust module loader/HMR come later. This design must not block them.
- **C-7** Dock and code editor are standalone egui-only crates owned by cadatomic. The dock draws a pane's contents
  through a host callback `fn(&mut egui::Ui, PaneId)` and emits pane lifecycle events.

From cadatomic, adopted:

- **K-1** `App::logic` drains and executes; `App::ui` builds widgets only (overview §3 P3; SCOPE §6.1/§6.2 R3/R4).
  Side effects are gated on pass index 0 (SCOPE §6.2 R3, experiments EA-3).
- **K-2** Idle produces zero frames, verified with counters (overview §3 P8, SCOPE §6.2 R6, experiments PF-M16).
  A change in one window repaints only that window (DOCK-W2).
- **K-3** Ids are tuples of domain ids, never labels or positions (overview §3 P4, SCOPE §6.2 R2). egui ids are
  never derived from a tree salt (docking §5.10 F4), so a move keeps them (docking §6.7).
- **K-4** First-party Svelte runs in process on the UI thread (docking §10 D4). Third-party runs in isolates
  (framework §5.1, docking §6.3). Isolates are later work for svelters (SV15).
- **K-5** Wait-idle is a real barrier, with a mutation test (overview §3 P10, §8 rule 4). A busy source is
  `Fn() -> usize + Send + Sync` (`cadatomic-core/src/jobs/barrier.rs`, `IdleBarrier::add_busy_source`).
- **K-6** kittest 0.37 fails unnamed inputs (SCOPE §6.2 R10). Every kind forwards an accessible name.
- **K-7** No `unsafe` in cadatomic crates (native-ext §1, workspace `unsafe_code = "deny"`). Every trait and macro
  here must be implementable without `unsafe`.

Technical facts that shape the design (verified in the 0.37 sources):

- `App::logic` is called inside `run_ui` for the **root viewport only**. A deferred viewport's pass runs only its
  callback (`eframe-0.37.0/src/native/epi_integration.rs:349-358`). `logic` also runs on discarded passes.
- `show_viewport_deferred` takes `Fn(&mut Ui, ViewportClass) + Send + Sync + 'static`
  (`egui-0.37.0/src/context.rs:4608-4613`).
- Focus is per viewport (`memory/mod.rs:110`, `focus: ViewportIdMap<Focus>`). Ids and `Memory::data` are global
  per `Context` (docking §4.3).
- egui's AccessKit node id is the widget's `egui::Id` (`context.rs:4203`, `accesskit_node_builder(id, …)`).
  accesskit 0.25 nodes carry an `author_id` (`accesskit-0.25.1/src/lib.rs:2129`), which must be unique among
  siblings.
- egui 0.37 has plugins with `on_begin_pass(&mut Ui)` (`plugin.rs:33`). A plugin must be `Send + Sync`.

## 3. Current code facts

| Fact | Where |
|---|---|
| `EguiBackend` is `Rc<Inner>`: the arena, a raw `*const svl_host`, an `Arc<Shared>` (wake flag, frame counter, `Mutex<Option<Context>>`) | `backend.rs:52-84` |
| One entry point per pass, `ui(&mut Ui)`: pass 0 runs `before_pass` (closes, checkpoint, rAF, remeasure), then `walk`, then `after_pass` (outbox flush, repaint) | `backend.rs:206-262`, `:401-428` |
| The walk draws presented windows only: window 0 in a `CentralPanel`, the others with `show_viewport_immediate` | `backend.rs:310-343` |
| Layout runs in the walk when dirty or when the window size changed, keyed per window | `backend.rs:380-399` |
| Any host op outside a frame calls `ctx.request_repaint()` (whole app, current viewport) | `backend.rs:176-182` (`touched`) |
| A pass-active flag panics if JS would run during the walk | `backend.rs:184-186` |
| Node ids are `Id::unique(("svl", handle))`. They survive keyed reorders, not reloads. They clash across two backends in one `Context` | `scene.rs:240`; EGUI.md §1.1 |
| The paint never reuses a widget's id for its scope (`n.id.with("scope")`); focus requests are applied in the walk | `render.rs:232-236`, `:325-328` |
| Kinds: a static slice of 8 `KindDef`s; ids are `index+1`, members `(owner+1)<<16 \| (i+1)`. Widgets are a closed `enum W` with typed fields | `kinds.rs:285-398`, `:422-441`; `scene.rs:17-25` |
| The spec `metadata/semantics/egui.json` is generated from the kind table and checked by a test | `spec.rs:9`; `tests/spec.rs` |
| The compiler embeds the egui spec; `--spec` *replaces* it (no merge) | `semantics/backend.rs:13,41-49`; `bin/svelters-compile.rs:65-68` |
| `Host<'a>` borrows the backend's vtable; tests leak it to get `'static` | `svelters-host/src/lib.rs:167-196`; `harness.rs:39` |
| The engine evaluates the entry and **drops the module namespace** | `quickjs.rs:260` (`let (_m, promise) = …`) |
| The runtime's `mount(Component, { props, target })` already mounts into an existing host handle, never destroys the target, and returns an unmount with `.exports` | `index.js:1308-1362`; Svelte-shaped wrapper `packages/native-runtime/src/svelte.js` |
| The build's entry mounts one `App` at the root | `packages/build/entry.js` |
| Runtime "attached" nodes are inserted into an element with no slot or position (event controllers, context menus) | `index.js:240-246` |
| One turn budget per entry into JS, default 10 s; the interrupt handler also takes an external flag | `core.rs:80`; `quickjs.rs:149-159`; `lib.rs:506` |
| Errors are collected globally, with no origin | `core.rs:438-451`; `lib.rs:482` |
| `is_idle` = no queued events and no live timers; it ignores E1 ops, dirty E3 stores, pending frames, layout and the outbox | `core.rs:407-410` |
| The resolver serves `svelters:core` and `svelters:app/*` built-ins; everything else must be relative | `quickjs.rs:1045-1080` |
| Exports: inventory registry, kinds `app:<module>.<Type>` in pack 63, app handles bit 52 | `exports/mod.rs:1-60` |
| `on:name={h}` on an element lowers to a listener; a bare `on:name` is `unsupported_feature`; `use:` lowers to `Op::Action`; on components, every directive but `bind:` and spreads is rejected | `lower/mod.rs:791-798`, `:873`, `:1394` |

## 4. EMBED: the proposed API

### 4.1 Objects

| Object | What it is | Owner |
|---|---|---|
| `Embed` | the owned handle: `EguiBackend`, its vtable, the `Host`, the root and slot tables; `!Send`, cheap `Clone` (`Rc`) | the app (`ShellApp` field) |
| `SlotId` / `SlotKey` | a Rust-created arena node of kind `slot` (no parent; taffy root; flex column) / the host's stable name for it, e.g. the pane id `"panel.task"` | Rust |
| `RootId` / `RootKey` | one mount into one slot / the host's stable name for it (ids, logs, MCP, scenarios) | Rust (lifetime), JS (content) |
| `EntryId` | a loaded bundle whose module namespace is kept, so mounts can name exports | the embed |
| `SlotPainter` | `Copy + Send + Sync` token (embed id + slot id) for deferred-viewport closures; resolves through a thread-local on the UI thread | anyone |
| `BusyProbe` | `Send + Sync` atomic mirror of `settled()` for cadatomic's `IdleBarrier` | anyone |

### 4.2 Rust signatures

```rust
// crates/svelters-egui/src/embed/mod.rs
pub struct Embed { inner: Rc<EmbedInner> }

pub struct EmbedConfig {
    /// Salts every egui Id of this embed. Two embeds in one Context must differ (debug-asserted).
    pub id_salt: &'static str,                       // default "svelters"
    pub host: svelters_host::HostConfig,             // memory limit, echo, quit_on_last_window = false here
    pub budgets: Budgets,
    pub faults: FaultPolicy,
    /// Run `logic` from an egui begin-pass plugin for non-root viewports (§5.2). Default true.
    pub prepass_plugin: bool,
}
pub struct Budgets {
    pub warn: Duration,        // 8 ms: one entry into JS longer than this logs a dev warning (docking §6.4)
    pub interrupt: Duration,   // 50 ms: interrupted and charged to the entry's root (docking §6.4)
    pub mount: Duration,       // 500 ms: a first render may take longer than a handler
}
pub struct FaultPolicy {
    /// Handler exceptions are reported; this many in `window` fault the root. None = never.
    pub handler_errors: Option<(u32, Duration)>,     // default Some((5, 10 s))
}

impl Embed {
    /// Creates the backend, the vtable and the host (owned: no leak, no lifetime) and attaches through ABI v2.7 `start`.
    pub fn new(ctx: &egui::Context, cfg: EmbedConfig) -> Result<Embed, EmbedError>;

    // ---- bundles ----
    /// Evaluates an ESM bundle and keeps its namespace. Runs JS: call from `logic` or before the first frame.
    pub fn load(&self, bundle: impl AsRef<Path>) -> Result<EntryId, JsError>;

    // ---- slots (any time, any thread phase; no JS) ----
    pub fn create_slot(&self, key: impl Into<SlotKey>, opts: SlotOptions) -> SlotId;
    pub fn slot(&self, key: &str) -> Option<SlotId>;
    /// Unmounts the slot's root (queued) and frees the slot after it.
    pub fn destroy_slot(&self, slot: SlotId);
    /// Visibility comes from the host (the dock knows it exactly). Hidden: no layout, walk or rAF; JS sees visible = false.
    pub fn set_visible(&self, slot: SlotId, visible: bool);
    /// The slot will be drawn in `viewport` from now on: per-viewport repaint routing, focus restore, the layout event.
    pub fn set_viewport(&self, slot: SlotId, viewport: egui::ViewportId);

    // ---- roots (queued; JS runs at the next `logic`) ----
    pub fn mount(&self, spec: MountSpec) -> Result<RootId, EmbedError>;   // Err only for a bad slot/entry or an occupied slot
    pub fn set_root_props(&self, root: RootId, patch: Props) -> Result<(), EmbedError>;  // shallow merge
    pub fn unmount(&self, root: RootId);
    /// Moves a root into another (empty) slot without remount: an arena reparent plus the moved event.
    pub fn move_root(&self, root: RootId, to: SlotId) -> Result<(), EmbedError>;
    /// Asks the root's `onCloseRequest` handlers (sync, in `logic`); the answer comes as `EmbedEvent::CloseDecided`.
    pub fn request_close(&self, root: RootId);
    /// Remounts a faulted root with its last props.
    pub fn restart_root(&self, root: RootId);

    // ---- the frame ----
    /// In `App::logic` (pass 0 only; later calls in the same pass are no-ops).
    pub fn logic(&self, ctx: &egui::Context);
    /// In `App::ui` or any viewport callback, on the UI thread. Never runs JS.
    pub fn show_slot(&self, ui: &mut egui::Ui, slot: SlotId) -> SlotResponse;
    pub fn painter(&self, slot: SlotId) -> SlotPainter;

    // ---- state, idle, events ----
    pub fn settled(&self) -> Settle;
    pub fn busy_probe(&self) -> BusyProbe;
    pub fn take_events(&self) -> Vec<EmbedEvent>;
    pub fn root_info(&self, root: RootId) -> Option<RootInfo>;     // title, dirty, state, size, viewport
    /// Last frame's screen rect of an anchored node, for overlays and tutorials (SV7).
    pub fn anchor_rect(&self, root: RootId, anchor: &str) -> Option<(egui::ViewportId, egui::Rect)>;
    /// Exports, op log, stats, interrupt flag: the underlying host.
    pub fn host(&self) -> &svelters_host::Host<'static>;
    /// After a runtime fault: new JS runtime, bundles reloaded, every root remounted into its slot.
    pub fn restart_runtime(&self);
}

pub struct MountSpec {
    pub entry: EntryId,
    pub export: Cow<'static, str>,     // "TaskPanel": a component exported by the bundle
    pub slot: SlotId,
    pub key: RootKey,                  // stable across runs: salts the root's ids (§4.4)
    pub props: Props,
    pub owner: RootOwner,              // InProcess (v1); Isolate(IsolateId) reserved for SV15
}
pub struct SlotOptions {
    pub sizing: SlotSizing,            // Fill (pane body) | FitHeight { min: f32, max: f32 } (inspector sections)
    pub label: Option<String>,         // AccessKit name of the slot node (a group)
    pub fault_ui: FaultUi,             // Default (message + "Reload" button) | Custom(Box<dyn Fn(&mut Ui, &RootError) -> bool>)
}
pub struct SlotResponse {
    pub rect: egui::Rect,              // what the slot took
    pub content_size: egui::Vec2,      // taffy's content size (FitHeight slots report what they want)
    pub state: RootState,
}
pub enum RootState { Empty, Mounting, Live, Hidden, Faulted(RootError), Unmounting }
pub enum EmbedEvent {
    Mounted(RootId),
    Faulted { root: RootId, error: RootError },
    CloseDecided { root: RootId, allow: bool },
    InfoChanged(RootId),                         // title / dirty written by JS (§4.6)
    Emitted { root: RootId, name: String, value: svelters_abi::Value },
    Error { root: Option<RootId>, error: JsError },   // reported, not fatal
    RuntimeFaulted(JsError),
}
/// `Props` is an ordered map of JS values; `Props::from(record)` takes any `IntoJs` record (exports E0),
/// `Props::json(serde_json::Value)` any JSON. `Value::Undefined` in a patch deletes the key.
pub struct Props(Vec<(String, svelters_abi::Value)>);
```

`Host` gains an owning constructor so that no embedder, test or cadatomic crate needs `unsafe` or a leak (K-7). The
unsafe lifetime erasure it needs already lives in svelters-host (`lib.rs:206`):

```rust
// crates/svelters-host/src/lib.rs
impl Host<'static> {
    /// The host owns the vtable (boxed, stable address) and drops it after detaching.
    pub fn new_owned<B: svelters_abi::Backend + 'static>(backend: B, cfg: HostConfig) -> Result<Host<'static>, JsError>;
}
```

### 4.3 JS side

The bundle entry for embedding exports components; it does not mount them. `svelteConfig` gains an `embed` map. The
generated `dist/embed.js` is:

```js
import '@svelters/native-runtime/embed';             // installs globalThis.__svelters_embed
export { default as TaskPanel } from '../src/TaskPanel.svelte';
export { default as ProblemsPanel } from '../src/ProblemsPanel.svelte';
```

`__svelters_embed` is the host's only entry into roots. It is called by the engine, under the turn budget, from
`logic`:

```js
globalThis.__svelters_embed = {
  // props: a plain object; wrapped in $state so `props(id, patch)` is reactive (Svelte's documented mount pattern)
  mount(Component, rootId, targetHandle, props) { /* index.js mount(…, { target, props }) inside a per-root
       boundary whose error handler calls __svelters_host.rootFault(rootId, e) instead of rethrowing */ },
  props(rootId, patch) { /* Object.assign on the $state proxy; undefined deletes */ },
  unmount(rootId) {},
  close(rootId) { /* runs the root's onCloseRequest handlers; true = allow */ },
};
```

Components reach their root through a host built-in module (resolver `quickjs.rs:1045`). Its fields are a host-owned
E3 store per root, so they are reactive. They arrive in the R1 changed batch, which means no JS runs when nothing
changed:

```ts
// svelters:embed  (typings generated with the exports)
export interface RootHandle {
  readonly id: number; readonly key: string;
  readonly visible: boolean;            // SV19
  readonly width: number; readonly height: number; readonly scale: number;   // the layout event (SV18)
  readonly viewport: string;            // changes when the pane moves to another OS window
  readonly focused: boolean;
  title: string; dirty: boolean;        // read by the host for the tab (cadatomic PaneHost::title/dirty)
  emit(name: string, value?: unknown): void;               // EmbedEvent::Emitted
  onCloseRequest(fn: () => boolean | void): () => void;    // scope-bound
}
export function root(): RootHandle;     // from the context set by __svelters_embed.mount
```

### 4.4 Ids and accessibility (SV16 minimum, SVP3)

| Thing | `egui::Id` | AccessKit `author_id` |
|---|---|---|
| slot node | `Id::unique(("svelters", salt, "slot", slot_key))` | `slot_key` |
| root base | `Id::unique(("svelters", salt, root_key))` | none |
| node with `anchor="…"` | `root_base.with(("a", anchor))` | `anchor` |
| other node (v1) | `root_base.with(("h", handle))` | none |
| other node (with D4 identity, later) | `root_base.with(("i", identity))` | none |

- **Never the slot, pane or viewport.** A move keeps every id (K-3; docking §6.7). A root that moves to another
  slot (`move_root`) keeps its ids because its base is the root key.
- **Never `ui.id()`.** The walk already gives each widget a global id and a scope id derived from it
  (`render.rs:232-236`). Embedding keeps that, which is what makes `show_slot` position-independent.
- **Assigned at insert.** A node created detached (`create`) gets its id when it is first inserted under a tree
  that reaches a slot: the subtree inherits the root base. `anchor` is a common prop on every kind. Changing it
  re-ids the node and drops its egui memory, and that is documented.
- **Debug checks.** Two embeds with the same salt in one `Context` fail a debug assertion, using a registry in
  `ctx.data`. Duplicate anchors in one root are a dev diagnostic. egui's own id-clash warning stays on.
- **AccessKit.** The slot is a `Group` node named by `SlotOptions::label`, and the root's nodes are its
  descendants in egui's tree. A move is a removal in one window's tree and an insertion in the other's with the
  same ids, as docking §5.8 asks. egui builds one tree per viewport, so this needs no work from us. Names are
  forwarded as today (`render.rs:315-321`).
- **Stability.** v1 node ids still contain handles, so they are stable across keyed reorders and across other
  roots mounting, but not across reloads. Anchored nodes are stable across runs, which is what tests, tutorials
  and MCP need first (CADATOMIC.md SV16 "two layers"; framework §2.1 `AnchorId`). D4 identity is the HMR track's.

## 5. Lifecycle of a frame

### 5.1 Sequence (root viewport, eframe 0.37)

```text
eframe: run_ui(raw_input) for ROOT
  egui plugins on_begin_pass(ui)       svelters plugin: viewport == ROOT → nothing (the host calls logic)
  App::logic(ctx)                      cadatomic: handle_close_request → Jobs::drain → apply_intents …
    embed.logic(ctx)                   (pass 0 only)
      1. flush last pass's outbox      post_event / prop_changed: queue only, no JS
      2. apply queued Rust commands    mount / props / unmount / move / close-request / restart (JS: __svelters_embed.*)
      3. visibility & viewport changes  per-root store fields (visible, viewport, size, scale) marked dirty
      4. host checkpoint               E1 completions → E3 stores → changed batch → events → timers → jobs
                                       (each dispatch armed with the 50 ms budget, charged to its root)
      5. rAF for visible roots         frame callbacks of hidden roots are skipped (SV19, SV37)
      6. remeasure leaves              text changes; a size change marks the slot's layout dirty
      7. route repaints                every visible slot touched by 1–6 → ctx.request_repaint_of(slot.viewport);
                                       earliest timer → request_repaint_after_for(d, ROOT)
      8. publish the busy probe        atomics read by IdleBarrier from any thread
  App::ui(ui)
    dock.show(ui, |ui, pane| adapter.pane_ui(ui, pane))
      embed.show_slot(ui, slot)        (no JS)
        a. hidden or Empty → nothing   Faulted → fault UI   Mounting → spinner after 150 ms (framework §3.3)
        b. layout if slot dirty or the available size changed (taffy on the slot's subtree)
        c. walk: cull by clip rect, paint leaves at their rects, responses → outbox (pass-active guard)
        d. record drawn rect, content size, viewport, focus within; anchors → last-frame rects
    barrier.note_frame(ctx.has_requested_repaint())
```

### 5.2 Deferred viewports (DOCK-W2)

- **The closure.** A deferred viewport's closure must be `Send + Sync + 'static`. It captures `SlotPainter`s (or
  only `PaneId`s, see §7.1), never the `Embed`. On the UI thread `SlotPainter::show(ui)` resolves the embed
  through a `thread_local!` registry (`embed/current.rs`) and calls `show_slot`. On any other thread it draws
  nothing and logs once in release; in debug it panics. That is docking §4.3's design: no `unsafe`, no fake
  `Send`.
- **Its JS.** eframe does not call `App::logic` for a deferred viewport. So when that viewport repaints because of
  its own input, the svelters plugin's `on_begin_pass` runs `embed.logic(ctx)` before the viewport's ui, still on
  pass 0. A click in window 2 is therefore handled in window 2's next pass, with no root-viewport frame. The
  checkpoint can touch roots in other windows; step 7 then requests repaints of exactly those viewports.
- **Wakes from other threads** (stores, workers, timers) do not know their viewport. They request a ROOT repaint,
  because `logic` runs there, and step 7 fans out from it. This costs one root frame per off-thread JS turn even
  when only window 2 changes. A custom runner could avoid it, but that stays deferred (SCOPE §6.1). DOCK-W2's
  frame counters measure it.
- **Immediate viewports** need nothing special: `show_slot` is called inside their closure.

### 5.3 Visibility and pausing (SV19)

The host sets visibility. The dock knows exactly which panes are shown, while a frame-based guess would be wrong
for a deferred viewport that simply did not repaint this frame. A hidden root:

- gets no layout, walk or a11y nodes;
- skips its rAF callbacks: the runtime keeps one rAF queue per root, tagged at registration by the current root
  context;
- has `visible = false` in `svelters:embed`, so its components can stop polling;
- keeps running timers, effects and E3 deliveries. Svelte reactivity is not paused. Pausing it would need HMR's
  snapshot machinery (EM3).

A debug check flags a slot that was drawn while set hidden, or that stayed visible without being drawn for 60
passes of its viewport.

### 5.4 Settling (SV35, SVP4)

```rust
pub struct Settle { pub busy: Busy, pub next_timer: Option<Instant> }
bitflags! { pub struct Busy: u32 {
    const EVENTS = 1; const CHANGES = 2; const JOBS = 4; const OPS_IN_FLIGHT = 8;   // host: queues, dirty set, microtasks, E1
    const STORES = 16; const FRAME = 32; const COMMANDS = 64;                        // E3 dirty bits, rAF, queued mount/props/unmount
    const LAYOUT = 128; const OUTBOX = 256; const MOUNTING = 512; const RELOAD = 1024; // scene and roots
    const TIMER_SOON = 2048;                                                          // a timer due within `horizon` (default 50 ms)
}}
impl Settle { pub fn is_settled(&self) -> bool { self.busy.is_empty() } }
```

- **Host side.** `Host` gets `busy() -> HostBusy`, which replaces `is_idle`'s partial answer (`core.rs:407`) with
  every queue the core holds, including E1 `ops_in_flight` and dirty E3 masks.
- **Timers.** A long timer (`setInterval(…, 1000)`) does not count as busy, otherwise a clock would never settle.
  The horizon is configurable, and `next_timer` lets a test advance a virtual clock.
- **cadatomic.** `barrier.add_busy_source({ let p = embed.busy_probe(); move || p.count() })`. The probe is an
  `Arc<AtomicU32>` of the bits, written at the end of `logic` and `show_slot`, so a race is at worst one frame
  stale. The barrier's N settled frames covers that (K-5).
- **Tests.** `EmbedHarness::run_until_settled(max_iters) -> Result<u32, NotSettled { chain: Vec<Busy> }>` runs
  kittest steps until `is_settled()`. It reports the bits of each iteration, so a non-converging layout ↔ effect
  loop shows its chain. That is slint §2 lesson 2's `flush_until_settled`, with Slint's cap of 10 as the default.

### 5.5 Repaint and idle

- Host ops mark the slot that owns the node dirty. The node caches its slot at insert, so the cost is one field
  read. `touched()`'s blanket `request_repaint()` (`backend.rs:176`) goes away.
- Repaints come only from dirty visible slots, the outbox, pending rAF, a due timer and wakes (K-2).
- G1's idle measurement (`svelters-egui-run --idle-report`) becomes an embed test: N slots, two viewports, and 0
  frames over 2 s.
- cadatomic's note that "a zero-delay `request_repaint` from outside a frame runs two frames"
  (`cadatomic-app/src/app.rs:602-609`) applies to our waker too. The waker keeps `request_repaint()` because it
  needs the latency. The measurement decides whether it should become `request_repaint_after(1 ms)` (§13 D12).

## 6. Threading and failure modes

### 6.1 Threading

| Thread | svelters-egui does | Never |
|---|---|---|
| UI thread | `Embed`, arena, QuickJS runtime, all JS, `logic`, `show_slot`, deferred-viewport callbacks through the thread-local | block on another thread |
| Any thread | `Waker` (`backend.rs:634`), E3 store writes, E1 completions, `BusyProbe` reads, `Host::interrupt_flag` | touch the arena or JS |
| Isolate threads (SV15, later) | their own runtime and a proxy backend; batches applied in `logic` (docking §6.3) | — |

`Embed` is `!Send`, which the type system enforces (`Rc`). One process may hold several embeds, for example
cadatomic's plus a test overlay. They must use different salts. They share egui but no handles.

### 6.2 Failure modes

| Failure | Detection | Effect | Other roots |
|---|---|---|---|
| Exception during a root's first render, or in an effect or template | per-root boundary (`__svelters_embed.mount`), calls `rootFault` | root disposed; slot draws the fault UI; `EmbedEvent::Faulted`; `restart_root` remounts with last props | unaffected |
| Exception in an event handler | `dispatch` error; the host maps the handle to its root through the scene (node → slot cache) | reported as `EmbedEvent::Error { root }`; N in the window faults the root (`FaultPolicy`) | unaffected |
| Exception in a timer, microtask or E1 settle with no root context | the engine error has no handle | `EmbedEvent::Error { root: None }` | unaffected |
| Runaway JS (loop in a handler, effect, mount) | interrupt handler, armed per entry (`Budgets`) | that entry stops; charged to the entry's root; the root faults (docking §6.5 "marked failed"); DOCK-W6 wants an interrupt within 100 ms, and 50 ms meets it | unaffected, unless the loop is in a shared microtask: then `root: None` |
| `ErrorKind::OutOfMemory`, stack overflow, or an engine `Host` error | engine error kinds (`engine.rs:14-27`) | `RuntimeFaulted`; every root faults; `restart_runtime` rebuilds the engine with the reload machinery (`reload.rs`, `MakeEngine`) and remounts all roots into their slots. JS state is lost until HMR snapshots exist | all faulted: one runtime cannot contain these (native-ext §6.4: "memory corruption cannot be contained") |
| Rust panic in a provider `Widget::ui` | `catch_unwind(AssertUnwindSafe)` around each root's walk in `show_slot` | root faulted; its egui child `Ui` is dropped; next frame draws the fault UI. Under `panic = "abort"` there is no recovery | unaffected |
| Panic in an export thunk | already `NativeError { code: 'panic' }` (egui-rust-exports §6.7) | a JS exception: rows above | — |
| `show_slot` on a destroyed or foreign slot | slot generation check | draws nothing; debug assertion | — |
| A root never finishes mounting (awaits forever) | `Busy::MOUNTING` with an age | spinner; a dev warning after 2 s | — |

Docking's isolate rows (an isolate stops, becomes unresponsive, or restarts at most 3 per minute) belong to SV15.
`RootOwner::Isolate` keeps them expressible.

## 7. The seams

### 7.1 The dock seam (shared with the dock/editor architect)

**Contract assumed from the dock crate.** It draws pane contents through `FnMut(&mut egui::Ui, PaneId)`, possibly
inside deferred viewports, and emits:

```rust
pub enum PaneEvent {
    Shown { pane: PaneId, viewport: egui::ViewportId },
    Hidden { pane: PaneId },
    MovedToViewport { pane: PaneId, from: egui::ViewportId, to: egui::ViewportId },
    Closed { pane: PaneId },
    // optional, if the dock asks before closing:
    CloseRequested { pane: PaneId },
}
```

**The adapter (lives in cadatomic, about 60 lines).** Every dock event is one `Embed` call, and drawing is one
lookup:

```rust
pub struct SvelteDock { embed: Embed, roots: HashMap<PaneId, RootId> }

impl SvelteDock {
    /// The host opens a pane with Svelte content (the view registry decides; the dock never mounts).
    pub fn open(&mut self, pane: &PaneId, entry: EntryId, export: &str, props: Props) -> Result<(), EmbedError> {
        let slot = self.embed.create_slot(pane.as_str(), SlotOptions::fill());
        let root = self.embed.mount(MountSpec { entry, export: export.to_owned().into(), slot,
                                                key: RootKey::from(pane.as_str()), props, owner: RootOwner::InProcess })?;
        self.roots.insert(pane.clone(), root);
        Ok(())
    }

    /// In `App::logic`, after `dock.take_events()`.
    pub fn on_event(&mut self, ev: &PaneEvent) {
        let Some(slot) = self.embed.slot(ev.pane().as_str()) else { return };   // not a Svelte pane
        match ev {
            PaneEvent::Shown { viewport, .. } => { self.embed.set_viewport(slot, *viewport); self.embed.set_visible(slot, true) }
            PaneEvent::Hidden { .. } => self.embed.set_visible(slot, false),
            PaneEvent::MovedToViewport { to, .. } => self.embed.set_viewport(slot, *to),
            PaneEvent::CloseRequested { pane } => self.embed.request_close(self.roots[pane]),   // answer: EmbedEvent::CloseDecided
            PaneEvent::Closed { pane } => { self.roots.remove(pane); self.embed.destroy_slot(slot) }
        }
    }

    /// The dock's draw callback. `Send + Sync`-free: it can be called from a deferred viewport's closure
    /// on the UI thread because it needs only the pane id.
    pub fn pane_ui(ui: &mut egui::Ui, pane: &PaneId) {
        svelters_egui::embed::with_current(|e| if let Some(s) = e.slot(pane.as_str()) { e.show_slot(ui, s); });
    }
}
```

What this gives:

- **A move is "draw it from elsewhere".** Nothing in the arena moves. Ids, egui memory (caret, scroll, text-edit
  state), JS instances and `$state` are untouched. `set_viewport` (a) routes repaints to the new viewport,
  (b) re-requests focus on the node that had it when focus was inside the root, because focus is per viewport
  (`memory/mod.rs:110`), and (c) updates `root().viewport`/`scale`, which is SV18's layout event.
- **DOCK-W4's acceptance holds by construction**: no remount (instance counter unchanged); focus, caret and
  scroll kept. A per-monitor scale change invalidates text measurement for that slot only: one relayout, at most
  one frame with old metrics.
- **Disagreement with docking §6.2 and §5.3.** They make the slot a scene node under Rust dock components and a
  move a reparent. The user's decision C-7 (a standalone egui dock crate) removes the dock from the scene. Then a
  free-standing slot is strictly simpler: no cross-tree reparent, no AccessKit graft code, and the same guarantees.
  `move_root` remains for the one case docking §6.2 lists where the slot itself changes (pane body ↔ extension
  window).
- **Hidden panes stay mounted.** That is docking §5.3 and D5 in its "cheap" state. D5's adaptive unmount under
  pressure is EM3, after HMR's snapshot exists.
- **Pane titles and dirty marks** come from `embed.root_info(root).title/.dirty`. JS writes them on `root()`, and
  `EmbedEvent::InfoChanged` tells the dock to re-read. This replaces `PaneHost::title`/`dirty` for Svelte panes
  (`cadatomic-app/src/dock/mod.rs:49-75`).

### 7.2 The editor seam (`<code-editor>` through EXTEND)

C-7 says the editor is cadatomic's own egui-only crate. editor §4.6/§4.14 place it as a framework `Component` in
`cadatomic-ui`; the user's decision wins, and nothing below depends on where the crate lives. To be wrappable as a
kind with `#[svelters::widget]` (§8.2), with no JS on the editing path (editor §4.12; js-api §6.6 safe mode), the
editor crate must offer:

| # | Requirement | Why |
|---|---|---|
| E-1 | State as a plain `'static` struct (`EditorState`), created from options, not requiring `Send` | stored in the node's provider block; survives moves |
| E-2 | `fn show(&mut self, ui: &mut egui::Ui, id: egui::Id, size: egui::Vec2) -> EditorOutput`: draws in the rect it is given and uses **only the caller's `Id`** (and ids derived from it) for egui memory | node ids are salted and stable (§4.4); ids from `ui.id()` would break moves and two embeds |
| E-3 | `EditorOutput { response, edits: SmallVec<ChangeSet>, selection_changed: bool, focus_changed: Option<bool>, consumed_keys: …, commands: Vec<EditorCommandRequest> }`: results returned, never callbacks | events go to the outbox after the pass; no JS or host call inside `ui` |
| E-4 | A document input that is a handle, not a string: `set_document(Arc<dyn TextDoc>)` or a `Snapshot` + version, plus a small-text mode `set_text(&str)` / `text()` for `bind:value` scratch buffers | big documents never cross into JS (models §6.3 rule "no bulk copy into JS, ever") |
| E-5 | `fn desired_size(&self, width: f32, fonts: &egui::text::Fonts) -> egui::Vec2` for auto-height (one-line formula mode, editor §4.12) | `Widget::measure` width-down/height-up (slint §4) |
| E-6 | Its AccessKit nodes (`MultilineTextInput`, text runs) under the given id, with a name the caller supplies | K-6, widgets §5.3 NA3 |
| E-7 | IME through egui's `Event::Ime` / `IMEOutput`, and a `composing()` flag | the embed must not route Enter or Escape to default buttons while composing (widgets §5.3 pin 6) |
| E-8 | `fn apply(&mut self, cmd: EditorCommand)` for methods JS may call (`focus`, `select`, `revealLine`) | kind methods (`call`) |
| E-9 | No eframe dependency, no global state, no threads it spawns (native-ext §6.1) | one egui, cadatomic owns jobs |

The wrapper in cadatomic is then about 80 lines of `#[svelters::widget(tag = "code-editor")]`:

- props: `doc: Obj<Document>`, `language`, `readonly`, a bindable `value` for scratch mode;
- events: `change: Event<Edits>`, `cursor: Event<Selection>`;
- `#[state] editor: EditorState`.

`bind:value` is exact by construction because writes go through `cx.set` (SV10).

## 8. EXTEND: the proposed API

### 8.1 Registry and ids

- **The registry.** `kinds::KindRegistry` replaces the static `KINDS` slice. It is built once per process from:
  - **built-in kinds**, which keep their generated tables and the `enum W` fast path. The SV1 pushback
    (CADATOMIC.md) and slint §10.1 both say to keep generated enums for built-ins;
  - **provider kinds**, collected from `inventory::iter::<KindEntry>`, sorted by spec id, with duplicate tags or
    ids failing at startup. This matches E0's exports and cadatomic decisions 10/16 and Command answers 1.
- **Kind ids.**
  - Built-ins stay `index + 1` (pack 0).
  - Provider kinds use pack 62 (`62 << 22 | local`; CUSTOM-WIDGETS.md §3.2; egui-rust-exports §7.2). `local` is
    the kind's rank in the sorted list, so ids are stable for a given binary.
  - Member ids keep the `(owner+1)<<16 | (i+1)` scheme inside the local part.
- **Tags and spec ids.**
  - A provider kind declares a `namespace` (`"cad"`). Its spec id is `cad.QuantityField`, and its tag is the
    kebab `quantity-field`.
  - A capitalised tag is a Svelte component, not an element, so `<QuantityField>` (scene-node §8's spelling)
    cannot name a kind directly. A generated wrapper component is the optional bridge (§13 D4).
- **The node.** `W` gains `W::Ext(Box<dyn ErasedWidget>)`. `Node` gains `ext: ExtBlock` (provider data),
  `state: StateBits` (u64) and `anchor`. Its handle, key and taffy fields are unchanged.

### 8.2 Authoring a kind

```rust
use svelters_egui::prelude::*;      // Widget, WidgetCx, MeasureCx, A11yCx, Event, Caps, prop/event/state attrs

/// A unit-aware numeric field: `<quantity-field dim="length" bind:value={x} onaccepted={…} />`
#[svelters::widget(namespace = "cad", tag = "quantity-field", caps = FOCUSABLE | EDITABLE)]
pub struct QuantityField {
    /// The committed value in SI units; the field shows it in the user's units.
    #[prop(bind)]                 value: Option<f64>,        // exact by construction: written only via cx.set
    #[prop]                       dim: Dim,                  // #[derive(svelters::Enum)] 'length' | 'angle' | …
    #[prop(default = true)]       units_shown: bool,
    #[prop]                       label: String,             // also the AccessKit name (K-6)
    #[prop]                       problem: Option<String>,   // inline error (cadatomic slot status)
    #[event]                      accepted: Event<f64>,      // onaccepted
    #[event]                      edited: Event<String>,     // raw text, every change
    #[state]                      text: String,              // Rust-only node state; survives prop updates and moves
    #[state]                      parse_error: bool,
}

impl Widget for QuantityField {
    fn measure(&mut self, cx: &mut MeasureCx, avail: AvailableSpace) -> egui::Vec2 {
        cx.text_field_size(&self.text, avail.width_or(160.0))        // width down, height up (slint §4)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response {
        let r = ui.add(egui::TextEdit::singleline(&mut self.text).id(cx.id()).desired_width(cx.rect().width()));
        if r.changed() {
            cx.emit(&self.edited, self.text.clone());
            self.parse_error = units::parse(&self.text, self.dim).is_err();
            cx.set_state(cad::INVALID, self.parse_error);               // provider state bit (§8.4)
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.parse_error {
            let v = units::parse(&self.text, self.dim).unwrap();
            cx.set(&mut self.value, Some(v));                           // (node, value) dirty → prop_changed after the pass
            cx.emit(&self.accepted, v);
        }
        r
    }
    fn a11y(&self, _cx: &A11yCx, node: &mut accesskit::Node) {
        node.set_label(self.label.clone());
        if self.parse_error || self.problem.is_some() { node.set_invalid(accesskit::Invalid::True); }
    }
}
```

The `Widget` trait:

```rust
pub trait Widget: 'static {
    /// Intrinsic size: width down, height up. Called before the pass, only when MEASURE is dirty.
    fn measure(&mut self, cx: &mut MeasureCx, avail: AvailableSpace) -> egui::Vec2;
    /// Paint and interact inside `cx.rect()`. Never runs JS; writes go through `cx`.
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx) -> egui::Response;
    /// Fill role, name, value and states on the AccessKit node of `cx.id()`.
    fn a11y(&self, cx: &A11yCx, node: &mut accesskit::Node) {}
    /// Children of container kinds are laid out by taffy and drawn by `cx.children(ui, slot)`.
    fn caps(&self) -> Caps { Caps::NONE }
}
```

`WidgetCx` gives:

- `id()`, `rect()`, `enabled()`, `state(bit)`, `set_state(bit, on)`;
- `set(&mut field, v)`, the exact write;
- `emit(&event, payload)`, queued;
- `children(ui, slot)`;
- `ext::<T>(key)` (§8.4);
- `request_measure()`;
- `store::<S>()` and `object::<T>(h)`, Rust reads with no JS (egui-rust-exports §7.4 item 6);
- `cached_shapes(version, |painter| …)`, the opt-in paint cache (§13 D3).

**What the macro generates** (one expansion, one schema):

1. A `KindEntry` via `inventory::submit!`, with the spec id, tag, base (`Widget`), capabilities, slots and docs
   from `///`.
2. The prop table. Each prop's type is described by `JsType::describe` from E0, so a prop can be any `Record`,
   `Enum`, `Option`, `Vec`, `Obj<T>`, `Shared<[T]>` or, after RA2, `Json<T>`. Kind props and exports therefore
   share one type schema and one `.d.ts` vocabulary. `set_prop` converts with `FromJs` and `get_prop` with
   `IntoJs`. Generated setters return the scene's `Dirty` (`measure` for props marked `#[prop(measure)]`, the
   text-like ones).
3. Event descriptors (`Event<T>` payload slots), and a sync variant that is one frame late
   (egui-rust-exports §7.2).
4. `ErasedWidget`, which the arena stores.
5. The spec-fragment JSON for the kind and the `IntrinsicElements` entry. Both are emitted by `spec.rs` from the
   registry, not by the macro, so the binary is the source of truth (EGUI.md §6; native-ext §6.3 "manifest-first",
   generated and drift-checked).

**Observe class.** Every `#[prop(bind)]` is `exact`/`delivery`, since writes go through `cx.set`. Other props are
`set_only`, which on egui is honest: nothing else changes them (CADATOMIC.md SV10). The SV10 rule for provider
kinds is enforced by construction: a `Widget` has no other way to change a prop.

**Wrapping an ecosystem widget** keeps `wrap_widget!` (egui-rust-exports §7.3), which expands to the same
`KindEntry` and diffs the bound fields after the call.

### 8.3 How the compiler learns provider kinds

1. **Dump.** The app binary runs `svelters::dump(dir)` (a `--dump-svelters` flag, or a test). It writes three files
   into `ui/.svelters/`:
   - `exports.json` and `app.d.ts`, both of which exist today;
   - the new `kinds.json`, a `svelters-semantics/v0` document with only `kinds` (plus their types).

   A checked-in copy is guarded by a staleness test like `tests/spec.rs`.
2. **Compile.**
   - `SpecBackend::builtin("egui")?.merge(fragment)` adds the kinds. A tag or member collision is an error, never a
     shadow.
   - CLI: `svelters-compile --backend egui --spec-fragment ui/.svelters/kinds.json` (repeatable). `--spec` keeps
     its replace meaning.
   - Vite: `svelteConfig(dir, { specFragments: ['.svelters/kinds.json'] })`.
   - The in-process compiler (SV27/CX1, for the live IDE) takes the same `&[Spec]` from the registry directly, with
     no file.
3. **Typings.** `app.d.ts` gains the `svelteHTML.IntrinsicElements` augmentation for every provider kind:
   ```ts
   'quantity-field': { value?: number | null; dim: Dim; 'units-shown'?: boolean; label: string; problem?: string | null;
                       onaccepted?: (e: CustomEvent<number>) => void; onedited?: (e: CustomEvent<string>) => void; 'bind:value'?: number | null }
   ```
   svelte-check then type-checks cadatomic's `.svelte` files against the binary they will run on.
4. **Checks that follow.** CX2 provider checks (SV8) and kind contracts (SV31) read the same fragment.

### 8.4 Per-node provider data and state bits (storage half of SV4, the C2 gap)

```rust
// a provider declares its node data and states once (statics; registered with the kind provider)
pub static PICK: ExtKey<PickState> = ExtKey::new("cad.pick");
pub static OP:   ExtKey<OpView>    = ExtKey::new("cad.op");
svelters_egui::state_bits! { pub mod cad { PENDING = "pending", STALE = "stale", FAILED = "failed",
                                            PICK_TARGET = "pick-target", PICK_REJECTED = "pick-rejected", INVALID = "invalid" } }

// Rust, outside the pass (e.g. cadatomic's logic after Jobs::drain), by anchor or handle:
embed.node_mut(root, Anchor("extents[0]"), |n| { n.ext_mut(&OP).set(op_view); n.set_state(cad::PENDING, true) });
```

- **Built-in states.** `StateBits` bits 0–31 are svelters' built-ins: hovered, pressed, focused, focus-within,
  disabled, checked, selected, open, expanded, invalid, read-only, dragging and exiting, the scene-node §1
  `StateFlags` set. The walk sets them from responses.
- **Provider states.** Bits 32–63 are allocated to registered provider names at startup. Running out is a startup
  error.
- **Who consumes them.**
  - The style hook (`style.rs`) applies state blocks with no JS (ST3/SV11).
  - A11y projects `busy` and `invalid`.
  - The MCP/test introspection reads them by name.
  - JS can observe them through a read-only `states` prop (`exact`) when a component wants `:pending`-like logic.
- **The `ExtBlock`** is a small `Vec<(ExtKey id, Box<dyn Any>)>`, empty on most nodes. cadatomic's `op`, `pick`,
  `presents` and `owner` (scene-node §1) live here instead of on every node. That is the provider form of C-3, and
  it answers framework C2 in svelters' favour (see §11).

### 8.5 Native behaviours behind `use:` (SV2)

A behaviour is an **attached kind**. The runtime already inserts attached nodes into an element with no slot or
position (`index.js:240-246`), as GTK event controllers do. It has props (its params), events and a Rust
`Behavior` impl in place of a `Widget`:

```rust
#[svelters::behavior(namespace = "cad", name = "pick")]
pub struct Pick { #[prop] accepts: Vec<String>, #[event] picked: Event<PickRef> }
impl Behavior for Pick {
    /// Runs after the host node's `ui`, every frame, on the UI thread, with its response. No JS.
    fn after_ui(&mut self, host: &mut NodeCx, resp: &egui::Response, cx: &mut WidgetCx) { /* hover → PICK_TARGET bit, click → emit picked */ }
}
```

The generated module `svelters:behaviors/cad` (or a re-export from `cadatomic:ui`) exports ordinary Svelte
actions:

```js
export const pick = nativeBehavior('cad.pick');   // (node, params) => { create attached node, set props, listen; update; destroy }
```

```svelte
<script> import { pick } from 'svelters:behaviors/cad'; </script>
<label use:pick={{ accepts: ['mcad.face'] }} onpicked={…}>Face</label>
```

- **No compiler change.** `use:` already lowers to `Op::Action` (`lower/mod.rs:798`), and the action creates the
  attached node through the existing host ops (`create`, `insert`, `set_prop`, `listen`).
- **Cost.** JS runs once at mount and once per params change. The behaviour runs every frame in Rust at no JS
  cost, which is scene-node §4's requirement.
- **Spelling.** It keeps svelte-check and stock tooling working, which is CADATOMIC.md SV2's amendment (import,
  not a special `use:` namespace). The import's typings come from the dump.
- **Isolates.** The same import works in an isolate later, because it is just ops.

### 8.6 Typed Rust↔JS values

There is one type system, E0's `JsType`/`FromJs`/`IntoJs`, used by:

- exports;
- kind props and event payloads (§8.2);
- root props (`Props::from(record)`, §4.2);
- `svelters:embed` fields.

| Need | Mechanism | State |
|---|---|---|
| Plain DTOs in a crate that can derive | `#[derive(svelters::Record/Enum/Error)]`, `#[js(rename/default/bigint/skip)]` | E0, done |
| cadatomic-core types (serde + schemars, must stay headless and `wasm32`, so no svelters derive; the orphan rule blocks `JsType` impls in glue) | `Json<T: Serialize + DeserializeOwned>` crossing as structured values, typed from `JsonSchema` when the `schemars` feature is on | **RA2** (SVP5) |
| Tagged unions | `#[js(tag = "kind")]` data enums → discriminated unions | **RA2** (E0 deviation) |
| Big numeric data | `Shared<[T]>`, `Buffer<T>` | E2, done |
| Reactive state | `#[svelters::store]`, `StoreStatic`, `Store<S>`, fields `exact` | E3, done |
| Handles to Rust objects | `#[svelters::object]` / `object(shared)`, `Obj<T>` | E0, done |
| Job-backed calls on **cadatomic's** pool (js-api §3.4a) | a bare `async fn` export (runs on the JS thread per user decision) awaiting a future cadatomic's job completion resolves; no svelters worker pool involved | E1, done |

`#[svelters::widget]` and `#[svelters::behavior]` join `object/export/store/service/Record/Enum/Error/Events` in the
same `svelters-macros` crate and the same `svelters` facade. The dump writes one schema with modules, kinds and
behaviours. This is the "one macro family, one schema" of EGUI.md §6 and TRACKS RUSTAPI.

### 8.7 `on:` forwarding (SV9 per C-2)

| Source (inside a component `C`) | Lowers to | Typing added to `C`'s props |
|---|---|---|
| `<button on:clicked />` (bare, native element) | `<button onclicked={$$props.onclicked} />` | `onclicked?: (e: CustomEvent<…>) => void`, payload from the kind's signal |
| `<Child on:accepted />` (bare, child component) | `<Child onaccepted={$$props.onaccepted} />` | `onaccepted?:` the child's declared callback prop type |
| `on:x={h}` on an element | unchanged: a listener (`lower/mod.rs:791-796`) | — |
| `on:x={h}` on a component | `onx={h}` (today rejected at `lower/mod.rs:1394`) | — |
| `on:x\|modifier` | still `unsupported_feature`, with a fix-it to `onx={…}` | — |

- **Internal props.** `$$props` here is the compiler's internal props parameter, not the user-visible legacy
  `$$props` that runes mode forbids. Combined with a user `onclicked={h}` on the same element, the bare form
  wins only when it is the only handler. Both together is a `duplicate_event` diagnostic.
- **Semantics.** A bare `on:` forwards to *one* callback prop, the parent's. Legacy forwarding fanned out to every
  `$on` listener. A note diagnostic (`legacy_forwarding_lowered`) says so once per component, which is
  CADATOMIC.md D1's recommendation (b).
- **Relation to propagation.** Bubbling and capture on the scene (EV1, SV9's first half) are independent. Bubbling
  crosses element boundaries; forwarding crosses component boundaries, which have no scene node.
- **Difference with cadatomic's wording.** scene-node §9 S2 and requests SV9 describe forwarding as "re-dispatches
  the event from the component to its parent". The lowering gives the same observable result for one parent
  listener and no legacy event machinery (C-2 wins).

### 8.8 End to end: the `svelters:app/task` module and the task panel

The glue crate is `cadatomic-ui`. It can derive `svelters::*`; cadatomic-core cannot, until `Json<T>` lands.

```rust
// cadatomic-ui/src/svelte/task.rs
use svelters::prelude::*;
use cadatomic_core::lifecycle::{acquire::{SessionView, SlotStatus}, machine::Event};

#[derive(svelters::Enum, Clone, Copy, PartialEq)]
pub enum SlotState { Valid, Missing, Invalid, Stale }                       // 'valid' | 'missing' | …

#[derive(svelters::Record, Clone, PartialEq, Default)]
pub struct SlotRow { pub name: String, pub label: String, pub dim: Option<String>, pub required: bool,
                     pub state: SlotState, pub entered: Option<String>, pub problem: Option<String>, pub group: Option<String> }

/// The open session, mirrored for Svelte. Written by the shell in `logic`; read reactively in JS.
#[svelters::store(module = "task")]
#[derive(Default, Clone)]
pub struct Task {
    pub open: bool,
    pub command: String,
    pub title: String,
    pub slots: Vec<SlotRow>,          // compared deeply (core.rs same_value), delivered only when changed
    pub valid: bool,
    pub refusal: Option<String>,      // catalog text, already localised
    pub stale: bool,
    pub preview_pending: bool,
}
#[svelters::export(module = "task")]
pub static TASK: StoreStatic<Task> = StoreStatic::new();

/// Lifecycle events from the panel, drained by `ShellApp::logic` (overview §3 P3: ui never mutates).
pub static TASK_EVENTS: std::sync::Mutex<Vec<Event>> = std::sync::Mutex::new(Vec::new());
fn push(ev: Event) { TASK_EVENTS.lock().unwrap().push(ev) }

#[svelters::export(module = "task")]
pub fn set_field(slot: String, entered: String) { push(Event::SetField { slot, value: entered.into() }) }
#[svelters::export(module = "task")]
pub fn accept() { push(Event::Accept) }
#[svelters::export(module = "task")]
pub fn cancel() { push(Event::Cancel) }
#[svelters::export(module = "task")]
pub fn rebase() { /* push(Event::Rebase { entered }) with the drafts the store holds */ }
/// Parses with the unit catalog; errors carry `code`s for the inline message.
#[svelters::export(module = "task")]
pub fn parse(text: String, dim: String) -> Result<f64, UnitError> { units::parse_str(&text, &dim) }

impl Task {                                               // called from ShellApp::logic when the session changes
    pub fn publish(view: &SessionView, cat: &dyn TextCatalog) { TASK.update(|t| *t = Task::from_view(view, cat)) }
}
```

```svelte
<!-- cadatomic/ui/src/TaskPanel.svelte -->
<script lang="ts">
  import { task, setField, accept, cancel, rebase } from 'svelters:app/task';   // the static TASK is `task` in JS (E0 camelCase)
  import { root } from 'svelters:embed';
  const r = root();
  $effect(() => { r.title = task.title; });
</script>

{#if task.open}
  <box class="column">
    {#if task.stale}<label>{task.refusal}</label><button onclicked={rebase}>Rebase</button>{/if}
    {#each task.slots as s (s.name)}
      <quantity-field anchor={`slot/${s.name}`} label={s.label} dim={s.dim ?? 'length'}
                      problem={s.problem} onaccepted={(e) => setField(s.name, String(e.detail))} />
    {/each}
    <box class="row">
      <button anchor="accept" enabled={task.valid} tooltip={task.refusal ?? ''} onclicked={accept}>Accept</button>
      <button anchor="cancel" onclicked={cancel}>Cancel</button>
    </box>
  </box>
{/if}
```

Wiring in the shell:

```rust
let embed = Embed::new(&cc.egui_ctx, EmbedConfig { id_salt: "cadatomic", ..Default::default() })?;
let ui = embed.load("ui/dist/embed.js")?;
let slot = embed.create_slot("panel.task", SlotOptions::fill().label("Task"));
embed.mount(MountSpec::new(ui, "TaskPanel", slot, "task"))?;
barrier.add_busy_source({ let p = embed.busy_probe(); move || p.count() });
// logic:  …Jobs::drain…; for ev in TASK_EVENTS.lock().unwrap().drain(..) { task_host.on_event(ev) }; Task::publish(..); embed.logic(ctx);
// ui:     right panel → embed.show_slot(ui, slot);
```

A kittest port of `task_forms.rs` drives this through anchors. `harness.get_by_label("Extents X")` works because
`label` is the AccessKit name; the author id is `slot/extents[0]`.

The one-command-path rule holds: the panel only pushes `lifecycle::Event`s, which `logic` drains (command-
architecture §3.3). The longer-term scheme for published APIs is `cadatomic:api/*` over SV22/SV23 (js-api §4.2).
That document keeps `svelters:app/*` for first-party code, so SC1's module is consistent with it.

## 9. Module split of `crates/svelters-egui` (EG0 + this design)

Each lane gets files of its own, so EMBED, RUSTAPI, WIDGETS, EVENTS and STYLE do not collide (TRACKS §4.3).

| File | Content (from today's code) | Writer lane |
|---|---|---|
| `lib.rs` | modules, prelude, `pub use egui` | any (small) |
| `embed/mod.rs` | `Embed`, `EmbedConfig`, public API | EMBED |
| `embed/roots.rs` | root table, root state, per-root store, faults | EMBED |
| `embed/slots.rs` | slot table, visibility, viewport tracking, `SlotResponse` | EMBED |
| `embed/current.rs` | thread-local registry, `SlotPainter`, `with_current`, begin-pass plugin | EMBED |
| `embed/settle.rs` | `Busy`, `Settle`, `BusyProbe` | EMBED |
| `frame.rs` | `logic`: closes, commands, checkpoint, rAF, remeasure, repaint routing (`before_pass`/`after_pass`, `backend.rs:225-262`, `:401-428`) | EMBED |
| `walk.rs` | the walk, culling, outbox, pass guard (`render.rs:212-242`, `backend.rs:380-399`) | EMBED, then EVENTS (propagation hooks) |
| `leaf.rs` | built-in leaf painting (`render.rs:244-372`) | WIDGETS |
| `ops.rs` | `impl Backend for EguiBackend` (`backend.rs:447-729`) | shared; changes small |
| `scene/{mod,node,props}.rs` | arena (`scene.rs:1-230`), `Node` + `StateBits` + `ExtBlock`, built-in prop get/set (`scene.rs:300-466`) | WIDGETS (props), RUSTAPI (node ext) |
| `ids.rs`, `a11y.rs` | id derivation (§4.4), names, author ids, role projection | EMBED |
| `layout.rs` | taffy on the arena (unchanged) | WIDGETS (per-kind measure), STYLE later |
| `kinds/{mod,builtin,containers}.rs` | `KindRegistry`, id scheme, built-in tables (`kinds.rs`) | WIDGETS (one file per new family) |
| `provider/{mod,state,behavior}.rs` | `Widget`, `WidgetCx`, `MeasureCx`, `KindEntry`, `ExtKey`, state-bit registry, `Behavior` | RUSTAPI, EVENTS (behaviours) |
| `events.rs` | propagation (EV1), empty seam at EG0 | EVENTS |
| `style.rs` | state blocks and decls sink, empty seam at EG0 (ST3, then the Rust CSS engine) | STYLE |
| `spec.rs` | spec, fragment and `IntrinsicElements` generation from the registry | RUSTAPI |
| `windows.rs` | `window` kind as a viewport, for the desktop runner and kittest (`backend.rs:310-378`) | EMBED |
| `runner/{eframe,kittest}.rs` | `run_app` and `TestApp`, rebuilt on `Embed` (one root, the bundle's `window`) | EMBED |

- **Re-expression.** The desktop runner becomes a thin user of `Embed`. It loads the bundle the old way (`mount(App)`
  at the root), and `windows.rs` draws presented windows. Fixtures and op-log replays run unchanged, which is
  EG0's no-behaviour-change gate.
- **Unchanged crates.** svelters-host gets `new_owned`, `load_entry`, the engine's root calls, error origins and
  `busy()`. `svelters-macros` gets `widget`/`behavior`. `packages/native-runtime` gets `embed.js` and per-root rAF.

## 10. Milestones, sizes, tests

Sizes are lane-days in the style of CADATOMIC.md §2. Each milestone ends with the CLAUDE.md gates and CI.

| Id | Content | Size | Depends | Tests (acceptance) |
|---|---|---|---|---|
| **EG0** | The split of §9 with no behaviour change; `KindRegistry` over the existing tables; empty `events.rs`/`style.rs`/`provider/` | 1 | — | all existing svelters-egui tests and op-log replays unchanged; `egui.json` byte-identical |
| **EM1** | SC1 core. Host: `new_owned`, `load_entry`, engine `mount_root/root_props/unmount_root/close_root`, error origin, `busy()`. Runtime: `embed.js`, per-root boundary, per-root rAF, `svelters:embed`. Egui: `Embed`, slots, roots, queued commands, `logic`/`show_slot`, ids and author ids (§4.4), `anchor` prop, `settled`/`BusyProbe`, faults (render, handler, interrupt), fault UI. Build: `svelteConfig({ embed })` | 6–8 | EG0 | kittest: two roots in one runtime inside a host `Ui` (one panel left, one right); props update reactive; unmount frees every handle (`live_callbacks`, arena len); ids unchanged across a keyed reorder, a second root mounting and a second embed with another salt; a throwing component faults only its root; an infinite loop in a handler is interrupted ≤ 100 ms and faults only that root (DOCK-W6 numbers); `run_until_settled` converges ≤ 3 iterations (slint SL6) and reports an injected cycle; idle 0 frames over 2 s with 5 slots |
| **RA1** | `#[svelters::widget]` struct form, `KindEntry` via inventory, pack-62 ids, typed props from `JsType`, events, `#[state]`, `Widget` trait, `ExtKey`/state bits, spec fragment + `--spec-fragment` + Vite option, `IntrinsicElements` in the dump | 5–6 | EG0 (hard), EM1 (soft) | a test crate registers `quantity-field`; the compiler rejects an unknown prop on it and accepts `bind:value` (`exact`); svelte-check passes against the dumped typings; kittest drives it by label; staleness test for `kinds.json` |
| **EM2** | Deferred viewports: thread-local registry, `SlotPainter`, begin-pass plugin; `set_viewport` (repaint routing, focus restore, layout fields); `move_root`; `set_visible` with rAF pause and the debug checks; `request_close`/`CloseDecided` | 5–6 | EM1 | DOCK-W2 on svelters' side: 3 viewports (2 deferred), a change in window 2 repaints only window 2 (per-viewport frame counters), idle 0 frames in every window; DOCK-W4 on svelters' side: a root drawn in viewport A then B keeps caret, scroll and focus, with JS instance counters unchanged; a hidden root's rAF callback does not run; the dock-adapter shape of §7.1 as a test helper |
| **FW1** | `on:` forwarding lowering (§8.7) with typings and diagnostics | 1–2 | the CF2/SP1 merges on `lower/mod.rs` (TRACKS §4.3: one compiler writer) | compiler goldens: element and component forms, the duplicate diagnostic, the modifier fix-it; a runtime test where a grandparent receives a forwarded `clicked` |
| **RA2** | `Json<T>` with `schemars` typings; data enums `#[js(tag)]`; codegen over `&Schema` (SV24) | 4–5 | — | a `Refusal`-like serde type round-trips; typings show the discriminated union |
| **EV2b** | Native behaviours: `#[svelters::behavior]`, attached-kind plumbing in the walk (`after_ui`), the `svelters:behaviors/<ns>` generator | 3–4 | RA1 | `use:pick` from a provider runs with zero JS entries per frame (counted with `Timing`); params update is one `set_prop` |
| **RA3** | fn form, `wrap_widget!`, methods, container slots via `cx.children`, `cached_shapes` | 5–7 | RA1 | `collapsing` container and a wrapped `DragValue`; paint cache hit-rate test |
| **EM3** | Adaptive hidden roots (docking D5): snapshot and unmount under pressure, restore when shown | 4–5 | HMR H3 (hard) | hidden root unmounted at a memory threshold and restored with `$state` and caret |

- **SC1's svelters part** is EG0 + EM1 + RA1 (12–15 days) plus the WIDGETS WG1 subset that runs alongside.
- **EM2** follows, for cadatomic's DK3/DK4.
- **Parallel work.**
  - RA2 and FW1 can run in parallel with anything.
  - RA1 and EM1 touch different files after EG0 (§9): RA1 owns `provider/`, `kinds/`, `spec.rs` and the macros;
    EM1 owns `embed/`, `frame.rs`, `walk.rs` and svelters-host. The scene-node change (`StateBits`, `ExtBlock`,
    `anchor`) is landed by EM1 as empty fields first.

## 11. Agreement and disagreement with cadatomic's research

| cadatomic says | This design | Why |
|---|---|---|
| Slots are scene nodes under the dock's Rust components, and a move reparents the slot (docking §5.3, §6.1, §6.2) | slots are free-standing arena roots; a move is drawing from another `Ui` | C-7 puts the dock outside the scene; the guarantees (DOCK-W4) are the same with less machinery. `HostSlot` variants map to `SlotOptions` plus the host's own enum |
| Root model `Root { id, slot, owner, entry, props, state, node, caps }` (docking §6.1) | adopted: `RootId`, `SlotId`, `RootOwner`, `MountSpec { entry, export }`, `Props`, `RootState`; `caps` stays cadatomic's (policy is U5's) | — |
| egui ids from `NodeKey`, never a tree salt (docking §5.10 F4); framework §2.1 `Id::unique(("node", key))` | ids from embed salt + root key + anchor/handle, never slot, pane or `ui.id()` | the salt is needed for two embeds or hosts in one `Context` (SVP3); it is per root, so moves keep ids, which is the point of F4 |
| svelters' kind table "becomes one adapter onto the framework's node trait" (framework §2); `Node` carries `op`, `pick`, `presents`, `owner` (scene-node §1); C2 open | the arena stays svelters'; those fields are provider ext data and state bits (§8.4) | C-3 decides C2. scene-node §9 S6 ("its own crate … using svelters extension points") agrees |
| `Widget`/`Component` split `measure`/`paint` (cached)/`input`/`a11y` (framework §2.2, scene-node §1) | `measure`/`ui`/`a11y` plus opt-in `cx.cached_shapes` | egui fuses paint and interaction (`TextEdit`, `Slider`), so a forced split would mean reimplementing every wrapped widget. The paint cache is opt-in where it pays (framework §8 wants it measured first). The rect contract ("`interact` uses only the rect the layout gave it", "measured equals rendered") is kept |
| Native behaviours compile to one `attach_behavior(node, id, params)` op (scene-node §4) | attached kinds created by a generated action; existing ops | no compiler hook, no ABI change; the runtime cost is the same (once per mount and params change) |
| `<QuantityField slot="extents[0]" />` (scene-node §8) | `<quantity-field>` element; optional generated PascalCase wrappers | capitalised tags are Svelte components |
| Bare `on:click` "re-dispatches from the component to its parent" (scene-node §9 S2; requests SV9) | callback-prop forwarding (§8.7) | C-2 |
| Isolates are rquickjs runtimes (framework §5.1); decision 9 forks rquickjs for determinism (decisions; overview §4, §6; experiments R7, SX-3) | stock QuickJS-NG; one in-process runtime now, `RootOwner::Isolate` reserved | C-1. decisions already lists the conflict; cadatomic's register should drop the fork row |
| The editor is a framework `Component` in `cadatomic-ui` (editor §4.6, §4.14); the dock is drawn by the component framework (shell §5) | standalone egui crates (C-7), wrapped through EXTEND (§7) | C-7 |
| First-party checkpoint warns at 8 ms and interrupts at 50 ms (docking §6.4) | adopted as `Budgets` (mount 500 ms) | — |
| Hidden roots: mounted, no frames, `visible = false` context (docking §6.4); adaptive unload (D5) | adopted (§5.3); D5 is EM3 after HMR | C-6 sequencing: D5 depends on SV20 |
| Wait-idle is a barrier with busy sources (overview §3 P10; `barrier.rs`) | `BusyProbe` (`Send + Sync`) plus `Busy` reasons | `add_busy_source` requires `Send + Sync`; `Embed` is `!Send` |
| `flush_until_settled(max)` reporting the chain (slint §2, §10.2 SV35) | `EmbedHarness::run_until_settled`; in the app the barrier polls | settling needs egui passes, which only the harness owns |
| Animation demand per root and viewport (slint §5, SV37) | per-root rAF queues, per-viewport repaint routing | the TR2 animator builds on it |
| Measure width-down/height-up; reject width-for-height (slint §4) | `Widget::measure(avail)` contract, tested per kind | — |
| A11y: role, actions, states, caret and selection, ordered events (widgets §5.3, NA3) | `a11y()` fills the node; egui's own widgets keep theirs; text kinds must expose caret/selection | the WG3 reference files test it |
| Rust-native for >1k rows, camera/picking, hot input paths (widgets §2.2) | those are provider kinds; Svelte composes them | — |
| Kind/manifest output generated from the registry and drift-checked (native-ext §6.3) | `svelters::dump` + staleness tests | — |
| Job members run on cadatomic's pool, not svelters' rayon (js-api §3.4a) | bare `async fn` exports on the JS thread awaiting cadatomic futures | E1's executor rule already allows it |
| `svelters:app/*` stays for first-party; published APIs are `cadatomic:api/*` (js-api §4.2) | SC1 uses `svelters:app/task`; `cadatomic:api` later on SV22/SV23 | — |
| Model/row traits `RowSet`/`Model` (models §4.2) | not here: LISTS (SV40/SV41) consumes EXTEND's object handles | out of scope |

## 12. Deliberately deferred

- **Isolates (SV15, SV21).** Only the `RootOwner` field and per-root error origins exist now.
- **HMR and `$state` snapshot (SV20, SV36), D4 compiler identity (SV16 full), adaptive unload (D5, EM3).** C-6.
- **The Rust module loader (SV22).** `load()` takes a built bundle. The embed API names bundles by path, and the
  loader will later take `EntryId`s from a resolver without any API change.
- **The CSS engine, ST3 state blocks and tokens (SV4 style half, SV5, SV11).** `style.rs` and `StateBits` are the
  seam.
- **Event propagation and Rust event filters (SV6, EV1/EV2a), overlays beyond `anchor_rect` (SV7).**
- **Root-created `window` elements inside an embed.** They are drawn as immediate viewports by `windows.rs`, as
  today. cadatomic routes dialogs through its own slots (`HostSlot::Dialog`) instead.
- **A custom winit runner** to avoid the root frame per off-thread wake (§5.2).

## 13. Open decisions

| # | Decision | Options | Recommendation |
|---|---|---|---|
| D1 | How a root follows its pane | (a) free-standing slot drawn from any `Ui` (§7.1); (b) slot reparented in the arena under dock nodes (docking §6.2) | **(a)**; `move_root` covers the slot-kind change |
| D2 | Kind registration | (a) `inventory`, sorted, duplicates fail at startup; (b) explicit `Embed::register::<K>()` | **(a)**, as E0 exports and cadatomic decisions 10/16; an `EmbedConfig::kinds` allow-list can filter per embed |
| D3 | `Widget` shape | (a) `measure`/`ui`/`a11y` + opt-in `cached_shapes`; (b) cadatomic's split `measure`/`paint`/`input`/`a11y` | **(a)**; revisit with framework §8's shape-cache measurement |
| D4 | Provider tag spelling | (a) kebab elements `<quantity-field>`; (b) also generated PascalCase wrapper components | **(a)** now, (b) as an opt-in dump option if cadatomic wants PascalCase |
| D5 | Native behaviour mechanism | (a) attached kinds behind generated actions (§8.5); (b) a compiler-resolved `attach_behavior` op | **(a)**; (b) only if counting shows the mount-time JS matters |
| D6 | Bare `on:` on elements as well as child components | (a) both lower to callback-prop forwarding; (b) components only, elements rejected | **(a)**: Svelte 4 allowed both, and cadatomic's editor example uses element `on:` |
| D7 | Visibility source | (a) host-set (`set_visible`), with debug checks; (b) auto-detected from passes | **(a)**: a deferred viewport that did not repaint is not hidden |
| D8 | Handler-exception policy | (a) report; fault after 5 in 10 s; (b) fault at once; (c) never fault | **(a)**: DOM-like; render and effect errors fault at once |
| D9 | Runtime fault recovery | (a) `restart_runtime` remounts all roots, state lost; (b) process restart | **(a)**; HMR snapshots later make it lossless |
| D10 | Props update semantics | (a) shallow merge, `undefined` deletes; (b) replace | **(a)**, with `Props::replace()` for (b) |
| D11 | Where root info (title, dirty, size, visible) lives | (a) per-root host store behind `svelters:embed`; (b) Rust-only getters plus JS events | **(a)**: reactive in JS, no per-frame JS, readable from Rust |
| D12 | Off-thread wake | (a) `request_repaint()` (lowest latency); (b) `request_repaint_after(1 ms)` (cadatomic's two-frame note, `app.rs:602-609`) | measure in EM2's counters; default (a) |

## Sources

- svelters: `crates/svelters-egui/src/{backend,scene,kinds,render,layout,spec,runner,harness}.rs`;
  `crates/svelters-host/src/{lib,core,engine,quickjs}.rs`; `crates/svelters-host/src/exports/mod.rs`;
  `crates/svelters-macros/src/lib.rs`; `crates/svelters/src/lib.rs`; `crates/svelters/tests/reactive.rs`;
  `crates/svelters-compiler/src/lower/mod.rs`; `crates/svelters-compiler/src/semantics/{backend,overlay}.rs`;
  `crates/svelters-compiler/src/bin/svelters-compile.rs`; `packages/native-runtime/src/{index,svelte}.js`;
  `packages/build/{index,entry}.js`; `plans/research/{EGUI,CADATOMIC,TRACKS,CUSTOM-WIDGETS,QUICKJS-RUNTIME,HMR}.md`;
  `docs/notes/egui-rust-exports.md`; `STATUS.md`.
- egui 0.37.0 / eframe 0.37.0 / accesskit 0.25.1 sources (cargo registry): `egui/src/context.rs` (viewports,
  `accesskit_node_builder`, plugins), `egui/src/memory/mod.rs` (focus per viewport), `egui/src/plugin.rs`,
  `egui/src/id.rs`, `eframe/src/epi.rs` (`App::logic`), `eframe/src/native/epi_integration.rs` (logic only for the
  root viewport), `accesskit/src/lib.rs` (`author_id`).
- cadatomic (read-only, master `92fae3f`): every file under `docs/research/`; `docs/plan/{00-decisions,01-overview,
  04-kernel-requests,05-experiments-and-research}.md`; `SCOPE.md` §6; `crates/cadatomic-app/src/{app.rs,dock/*,
  task_panel/mod.rs}`; `crates/cadatomic-core/src/{jobs/barrier.rs,lifecycle/{acquire,machine}.rs,command/def.rs}`.
