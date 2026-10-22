# ARCH-CRATES: the dock and the code editor as standalone egui crates

Status: architecture proposal, 2026-10-22. Design only. Nothing here is implemented, and no build was run for it.
It answers the user's request "lets do some research and properly architect this first" for the two crates the
user split out of cadatomic.

**User decisions this document builds on (2026-10-21/22):**

- The **dock** (docking, tabs, splits, OS windows) and the **code editor** are separate, independent crates.
- Each crate lives in **its own repository** and depends only on egui. It has no dependency on svelters, cadatomic
  or the kernel crates.
- cadatomic's sessions develop them. Their APIs are coordinated with what svelters and cadatomic need.
- Standing decisions from svelters `CLAUDE.md`:
  - egui stays at 0.37 and is upgraded together with cadatomic;
  - there is one retained scene, owned by svelters-egui;
  - svelters has its own Rust module loader, which compiles `.svelte` files live, release builds included.

**Inputs read:**

- **cadatomic** at `92fae3f`:
  - every file in `docs/research/` (15 files);
  - `docs/plan/00-decisions.md`, `01-overview.md` and `05-experiments-and-research.md`;
  - `crates/cadatomic-app/src/dock/{mod,seam,layout_file,viewport_tabs}.rs` and `crates/cadatomic-app/src/app.rs`;
  - `forks.toml` and `pins.toml`.
- **The local forks** `~/projects/egui_tiles-fork` and `~/projects/egui_mcp-fork` (their `FORK.md`).
- **svelters:**
  - `plans/research/{EGUI,TRACKS,CADATOMIC}.md`;
  - `crates/svelters-egui/src/*`.
- **egui 0.37.0, eframe 0.37.0 and egui_kittest 0.37.0** sources in the local cargo registry, for every API named
  below.
- **crates.io**, queried 2026-10-22, for the versions and licences of the prior-art crates.

How to read the citations:

- cadatomic research files are cited by short name: `docking`, `code-editor`, `ide-shell`, `widget-survey`,
  `workspace`, `live-ide` (js-api-publishing-and-live-ide), `slint`, `command-arch` (2026-10-16
  command-architecture), `models`, `framework` (component-framework), `scene-node`, `native-ext`. Each citation
  adds a section number.
- `kapi-handoff`, `kapi-migration` and `m1-seam-readiness` were read and contain nothing for either crate.
- Claims not checked in source are marked [unverified].

## Decided (user, 2026-10-22)

The user took every recommendation in §7 (O1–O14), to be revisited later: names `berth`
(`berth-core`, `egui_berth`) and `quire` (`quire-core`, `quire-syntax`, `egui_quire`); MIT OR Apache-2.0;
git tags only; host applies intents plus `auto_apply`; immediate viewports first; four pane events; left,
right and bottom regions; highlighter by E5; `<code-editor>` wrapper in cadatomic; host-owned undo for
documents; ropey 1.6 behind `Text`; VS Code keymap as data; cross-window drag without the winit fork
except Wayland; the crate owns the layout format.

Plus the seam decisions and corrections recorded in `ARCH-EMBED.md` "Decided" and cadatomic `ecd49c5`
(R1–R12). Repos: `github.com/akiselev/berth` and `github.com/akiselev/quire` (private). The ThinkPad builds
berth, with behaviour tests derived from KDDockWidgets' test suite (reimplemented, not copied: KDDW is
GPL/commercial) and egui_tiles/egui_dock as further references. The MacBook builds quire on ropey, with VS
Code's editor tests ported (MIT; keep its notice).

**`PaneEvent` is not closed (user, 2026-10-22; cadatomic `b8e074e`).** R1 makes §4.1 the canonical
*definition* of `PaneEvent`; it does not freeze the event set. berth adds a close reason on close events
(user, API or restore), an optional per-pane `max_size`, per-pane host data saved in the layout file, and a
skip-on-restore flag for temporary panes (an active tool's, for example). Consumers match `PaneEvent` and
`CloseReason` with a wildcard arm.

## 0. Summary

1. **Two repositories, each a headless core plus an egui renderer.**
   - The dock is `berth-core`, with no egui, plus `egui_berth`.
   - The editor is `quire-core`, with no egui, plus `quire-syntax` (tree-sitter) and `egui_quire`.
   - These are working names (O1).
   - Each core builds for `wasm32-unknown-unknown` and has no tokio. That is cadatomic's headless-core rule
     (`01-overview` §3.1) applied inside the crates themselves.
2. **One shape for both crates: state, typed commands, a pure reducer and a thin widget.** It is CodeMirror 6's
   state/transaction model, and it is also what cadatomic's rules require:
   - one mutation path;
   - `ui` never mutates (`01-overview` §3.2–3.3);
   - changes are applied in `logic` (`docking` §5.2).

   The widget never changes state while egui is drawing. It returns intents: `DockCommand`s or editor
   `Transaction`s. The host routes them through its command registry and applies them between frames. An
   `auto_apply` convenience covers hosts with no registry.
3. **The dock is the design in cadatomic's docking research, moved out of the scene.**
   - The core holds:
     - the model: per OS window, fixed regions and an n-ary editor-area tree;
     - stable ids, invariants after every command, an integer solver, the drop resolver and the drag state
       machine;
     - layout undo;
     - a versioned layout file with migrations and placeholders for unknown panes.
   - The renderer paints the core's `DockFrame` with plain egui, one window per egui viewport.
   - Pane contents come from **one host callback**, `PaneHost::pane_ui(&mut Ui, &PaneId)`, which keeps the
     user-specified `fn(&mut egui::Ui, PaneId)` shape.
   - **Four lifecycle events** (`Shown`, `Hidden`, `MovedToViewport`, `Closed`) have the per-frame ordering in §4.1.
     A thin cadatomic adapter forwards them to svelters-egui's embed calls one-to-one.
4. **The editor is an egui widget over a rope.**
   - Built on `ropey` 1.6, behind a `Text` facade.
   - Edits are `ChangeSet`s, composable and invertible, with anchors, carried in `Transaction`s.
   - Undo groups are closed by Monaco's rules. Lines are virtualised over a height index, with a per-line galley
     cache.
   - Tree-sitter highlighting covers Svelte, TS and CSS, with injections, off the UI thread.
   - **Diagnostics come in per source and version**, and edits and saves go out as events.
   - The widget has no egui types in its state, separates `measure` from `show_in(rect)`, and builds its own
     AccessKit subtree. A svelters kind provider can therefore wrap it as `<code-editor>` without forking it.
5. **Where the research and the decisions disagree, the decisions win.** §6 lists every such conflict. Three cases:
   - cadatomic's docking research drew the dock in the retained scene (`docking` §5.3); the dock is now plain
     egui;
   - the code-editor research put the text model in `cadatomic-core` and drew the editor as a scene component
     (`code-editor` §4, §4.6, §4.14), and `live-ide` §6.5 put the editor in the framework crate; the editor is now
     its own crate;
   - `command-arch` §3.6 made layout not undoable; D8 now gives layout its own undo.
6. **Sizes.**
   - Dock: about 33–47 lane-days to tear-off, against TRACKS DOCK's 26–38. The difference is the repo, CI and
     egui-only overhead.
   - Editor: about 25–35 lane-days to the end of P0. TRACKS ED1+ED2 is 20–28.
   - Neither crate is on SC1's critical path (`TRACKS` §3.2: the demo uses today's egui_tiles seam).

## 1. Ground rules shared by both crates

### 1.1 Dependencies and versions

| Rule | Why |
|---|---|
| Renderer crates depend on `egui = "=0.37.0"` with `default-features = false` plus what they use. They depend on `accesskit` only through `egui::accesskit`. They never depend on eframe (examples use it as a dev-dependency) | User decision: egui only. One egui copy per graph (`EGUI.md` §5 item 3: two eguis fail as `expected egui::Ui, found egui::Ui`). `native-ext` §6.6: egui types carry no ABI promise, so pin exactly |
| The renderers re-export `egui` | The same pattern as svelters-egui (`crates/svelters-egui/src/lib.rs`) |
| Core crates do not depend on egui, winit, wgpu or tokio, and build for `wasm32-unknown-unknown`. CI checks both (as cadatomic's `check.sh` does, `01-overview` §3.1) | The core is testable without a display. The hub's wasm viewer (`code-editor` §4.4) and remote layouts can reuse it |
| Optional `serde` (on by default) and `schemars` features in the cores | cadatomic registers each dock command and editor command as a registry command whose `Args: Serialize + DeserializeOwned + JsonSchema` has an object root (`command-arch` §2.3, §4.1) |
| No thread creation. Off-thread work goes through a host-supplied `Spawner` | `native-ext` §6.1: "Jobs and threads are host-owned". cadatomic's job pool and svelters' workers stay in charge |
| `unsafe_code = "forbid"` | cadatomic's workspace denies unsafe code (`native-ext` §1). The thread-local viewport trick needs no `unsafe` (`docking` §4.3) |
| MSRV 1.95, the same as egui 0.37 and cadatomic (`01-overview` §4) | |
| egui upgrades: one crate minor per egui minor, upgraded in the same joint lane as svelters-egui and cadatomic | svelters `CLAUDE.md`; `EGUI.md` §5 item 6; `CADATOMIC.md` §6 D5 |

### 1.2 Principles carried over from cadatomic

Each principle is something a host needs from a library crate, so it holds in the crate and not only in cadatomic:

- **Ids, never labels or positions** (`01-overview` §3.4):
  - Panes are host strings.
  - Groups and splits get persisted counters that no operation rewrites.
  - Text positions are only valid with a `Version`, the same rule as `models` §4.2: "a position is valid only
    together with its `ModelVersion`".
- **One mutation path** (`command-arch` §1, §8.4; `docking` §5.2):
  - Model mutators are private.
  - `apply(cmd)` is the only entry point, and availability is a pure function of the state and the policy.
  - This avoids failure class F7 (`docking` §3: "many mutation entry points, each re-implementing policy").
- **Nothing moves during a pass** (`docking` §5.2, F5; `01-overview` §3.3):
  - The widgets queue intents.
  - The host or `auto_apply` applies them before the next pass.
- **Wait-idle is a real barrier** (`01-overview` §3.10). Both crates expose `is_settled()`, which is false while:
  - a drag is in progress;
  - intents are queued;
  - a highlight job is pending;
  - an animation is running.

  cadatomic registers it with `IdleBarrier`, as it does svelters' `settled()` (`CADATOMIC.md` SVP4).
- **Render only what changed** (`01-overview` §3.8; `framework` §8):
  - There is no repaint request when idle.
  - The caret blink stops after a few seconds of idle and under reduced motion (`code-editor` §4.6).
  - Repaints are requested per viewport (`request_repaint_of`), so one spinner does not wake every window
    (`slint` §5).
- **Text comes from the host**:
  - Every user-visible string goes through a text struct the host fills from its catalog. cadatomic's existing
    `DockText` is the model (`dock/mod.rs`).
  - Refusal reasons are ids with arguments, such as `dock.split.too_small{min}`, as in `command-arch` §4.1
    `Availability::Unavailable { reason: ReasonId, args }`.
- **Theming comes from the host**:
  - `DockStyle` and `EditorTheme` are plain structs, with defaults derived from `egui::Visuals`.
  - cadatomic fills them from its role and component tokens (`framework` §2.3). The crates never hard-code
    colours.
- **Accessibility from the first commit** (`docking` F9, §5.8; `framework` §6):
  - Roles and names are built through `Context::accesskit_node_builder` (egui 0.37, `context.rs:4203`).
  - kittest finds things by role and label.
- **Anchors for scenarios, tutorials and MCP** (`framework` §2.1 `AnchorId`; `docking` §5.2):
  - Both widgets answer `anchor_rect(&str)` queries for their own sub-anchors, such as
    `window[main]/editor/group[g3]/tabstrip`, or `line[42]` and `caret` in the editor.
  - cadatomic prefixes them with the anchor of the hosting node.

### 1.3 The common shape: state, command, reducer, widget

```rust
// In each core crate (names per crate below):
pub struct State { /* plain data; Clone is cheap (Arc / rope sharing) */ }
pub enum Command { /* serde, tagged; one variant per operation */ }
impl State {
    pub fn availability(&self, cmd: &Command, policy: &dyn Policy) -> Availability;   // pure
    pub fn apply(&mut self, cmd: Command, policy: &dyn Policy) -> Result<Applied, Refusal>;
    pub fn is_settled(&self) -> bool;
}
// In each renderer crate:
pub struct Widget<'a> { state: &'a State, /* config */ }
impl Widget<'_> { pub fn show(self, ui: &mut egui::Ui, /* host */) -> Output /* intents + events */; }
```

- **Prior art:**
  - CodeMirror 6: `EditorState` is immutable, `view.dispatch(tr)` applies a transaction, and the host can
    intercept with `dispatchTransactions`.
  - Elm and Redux reducers.
  - KDDW's core/view split (`docking` §11.3), in which views never do geometry.
- **The difference from egui_tiles and egui_dock:** they mutate inside `ui` through a `Behavior`/`TabViewer` with
  20–39 hooks (`docking` §2.1), which is the root cause of F5 and F7.
- **Hosts without a registry** call `Widget::auto_apply(true)`, which applies the output's intents itself at the
  start of the next `show`.

## 2. The dock crate (`berth`)

### 2.1 Goals and non-goals

**Goals (v1 = what replaces egui_tiles in cadatomic, then multi-window):**

- A headless, egui-free core with the following parts:
  - the layout model;
  - typed commands;
  - invariants checked after every command (always in debug builds, and on load);
  - an integer logical-px solver;
  - the drop resolver and the drag state machine;
  - layout undo (D8);
  - workspaces as overlays;
  - profiles (D6);
  - a versioned, finite-only layout file with chained migrations, golden files and placeholders for unknown
    panes.
- An egui renderer with these parts:
  - tab strips, splitters and stripes;
  - drop overlays and a drag ghost (a chip, never a second run of the pane's UI);
  - fixed edge regions per window;
  - in-window floating panels (D3) and overlay (auto-hide) panels (D15);
  - one OS window per egui viewport, immediate or deferred;
  - window placement memory with off-screen recovery;
  - AccessKit roles;
  - keyboard paths for every operation, including carry mode (`docking` §5.5).
- The pane seam: the `PaneHost` callback and four lifecycle events, exactly as in §4.1.
- Later: cross-window drag and tear-off through a `DockHost` platform trait, with the Wayland hybrid drag on the
  winit fork (D7, `docking` §11.8 items 6–8).

**Non-goals:**

- Pane contents. The dock never creates or destroys content (`docking` §6.2).
- A command registry, keymap or palette. The dock exposes typed commands; the host binds keys.
- Persistence I/O. The dock serialises to a string; the host writes the file atomically, as `layout_file.rs` does
  today.
- Grids (`docking` §5.1: "No grid container").
- Nested docks (`docking` §11.6, F4).
- Content layout. Taffy and svelters lay out pane contents.
- Native menus.

### 2.2 Prior art and what to take

| Source (licence) | What it does | Take | Do not take |
|---|---|---|---|
| egui_tiles 0.18.0, 2026-10-19 (MIT OR Apache-2.0); cadatomic's fork `~/projects/egui_tiles-fork` at `7e72b21` is the same code on 0.37 | `Tree { root, tiles: HashMap<TileId, Tile> }`, n-ary `Linear` with shares, a 39-hook `Behavior`, `SimplificationOptions` run every `ui`, nearest-centre drop targets, drag through `ctx.dragged_id()` (`docking` §2.1–2.2) | Simplification as named invariants; ids kept through wrapping and pruning (#144, #147); shares; a tab-bar scroll and trailing slot; drag-preview smoothing (`docking` §2.4). Code may be borrowed with its notice | The behaviour trait; mutation in `ui`; recursion over unbounded depth (F12); `f32::INFINITY` in files (F2); nearest-centre targeting |
| egui_dock 0.21.1, 2026-08-06, still egui ^0.36 (MIT) | A binary heap in a `Vec`, serialised `Rect`s, `Surface::Window` as an in-window `egui::Window`, a ~20-hook `TabViewer`, its own DnD (`docking` §2.1–2.2) | Per-tab policy (`allowed_in_windows`, veto by destination #351); hidden tab bars and collapsed groups (#322) | Positional ids (F4); rects in the model (F2/F3); floating that is not multi-window (F6) |
| cadatomic `dock/seam.rs` (today) | Wraps egui_tiles behind `PaneId` strings, `PaneHost` (title, closable, dirty, confirm_close, `ui`, `on_closed`) and `DockEvent::{SaveRequested, Closed}`; sorts the serialised map through `serde_json::Value` so an unchanged layout writes the same file | The **public surface** stays: `open`, `activate`, `close`, `request_close`, `panes`, `active_panes`, `show`, `to_json`, `from_json` (`docking` §5.11 step 2). `PaneId` strings and the `PaneHost` idea stay. The close-guard `Modal` moves to the host (it is a confirmation, `ide-shell` §2 rule 8) | `retain_pane` dropping unknown panes (`docking` §5.9: keep them as placeholders) |
| KDDockWidgets 2.5 (GPL-2/3 or commercial) | A layouting engine separate from views; min sizes propagate up; separator limits from the engine; drop preview by inserting a throwaway item; placeholders; a schema fuzzer; a bad-layout corpus; a Slint prototype that found "hit-testing must be central" and "stable row identity across refreshes" (`docking` §11.1–11.3) | **Designs only, no code** (GPL). These are §11.8 items 1–4 and 15 | Pixel geometry plus percentages persisted (KDDW #595) |
| Qt ADS 5.x (LGPL-2.1) | Real floating top-levels; dry-run restore; the Wayland hybrid drag with `xdg_toplevel_drag_v1` (`docking` §11.4–11.5) | **Designs only.** The hybrid drag recipe (an unmapped window attached before its first commit, an explicit grab offset, last-candidate within 400 ms); restore that validates first and builds once | Top-level overlay windows (#839); perspectives as full snapshots |
| VS Code, JetBrains, Blender | Editor groups; tool windows with stripe slots; full-shell windows (`docking` §4.1; `ide-shell` §2) | Fixed regions plus stable stripe slots (`ide-shell` §2 rules 1–2); MRU per window | VS Code's editor-only windows (D1 overrides) |
| Dear ImGui docking | Multi-viewport, broken on Wayland, buggy on X11 per users (`docking` §4.1) [unverified] | — | Following the pointer with real windows by default |
| Chromium on Wayland | `xdg-toplevel-drag-v1` where supported, else a plain DnD session (`docking` §4.1) | The fallback ladder | — |

### 2.3 Where this design follows or departs from cadatomic's docking research

| `docking` § | Their position | Here | Why |
|---|---|---|---|
| §5.1 model, §11.8 item 1 / D12 | A headless dock core owns the layout and an integer solver | **Agree.** It is now essential: with no retained scene under the dock, the core's `DockFrame` is the only geometry | — |
| §5.3, `ide-shell` §5 | "Drawn by the component framework", tab strips as scene components, taffy for splits | **Superseded.** Plain egui painter and interaction over `DockFrame` rects | User decision (egui-only crate). The research itself had already moved geometry into the core (§11.8 item 1), so little is lost |
| §5.1 "secondary windows are editor-only" | VS Code style | **Superseded by D1:** every window has the same structure. A window either owns its regions or shares the main window's workspace overlay (`RegionsMode::{Own, SharedWithMain}`) | D1 |
| §5.6 "closing a secondary window moves its tabs to the main window" | Move | **Superseded by D2:** close them. The host's close guard runs per pane; panes go `Closed` | D2 |
| §5.4 drag machine, §11.8 item 3 preview by simulation, hysteresis | — | **Agree**, in the core | W12, W14 |
| §5.5 / §11.8 item 6 `DockHost` | Platform trait | **Agree**, with the trait in the core and a default implementation from egui `ViewportInfo` in the renderer. Precise z-order queries (`WindowFromPoint`, `XQueryTree`) are host implementations, because they need winit or OS crates | egui-only |
| §5.9 file | Integers, migrations, placeholders | **Agree** | F2, F3 |
| §11.8 item 4 / D13 | Last positions outside the tree | **Agree** | — |
| §11.8 item 5 / D14 | Affinities as data | **Agree**: `Affinity` sets on panes and containers, checked in availability | — |
| §11.8 item 12 / D15 | `Overlay` panel mode, click-to-open | **Agree** | — |
| D3 | In-window floating panels too | **Adopt**: `Float` surfaces per window, drawn as egui `Area`s on a dock-owned layer that never changes during a drag (F5) | D3 |
| D5 | Adaptive hidden roots | **The host's call.** `Hidden` carries a `reason` and the frame it began, so the host can evict after an idle period | D5 is svelters EM3's job |
| D6 | Per user and per project, with profiles | Profiles are named `Layout`s in a `Profiles` value. Choosing the per-user or per-project file is the host's job | — |
| D8 | Own layout undo | **Adopt**: an `UndoStack` in the core with resize coalesced per gesture | D8 overrides `command-arch` §3.6 ("UI layout, settings: not undoable") |
| D11 | Extensions may tear off and open windows under policy | `Policy::allows(cmd)` is consulted in availability | — |
| §7 hot reload | "The dock is Rust; a change is a restart" (`live-ide` §6.2) | Unchanged. Layout presets and workspaces reload as data through `apply_workspace` | — |
| `live-ide` §6.6 staged apply | A `critical` root first mounts "in a floating preview root" | Supported as a host feature: a host can open a temporary `Float` pane and close it later. The dock needs no special flag | — |

### 2.4 Model

```rust
// berth-core

/// Host-chosen, stable, persisted. cadatomic's grammar is kept: `viewport.<n>`, `doc.<doc>/view.<n>`,
/// `panel.<id>`, `ext.<ext>.<view>` (docking §5.1). Never derived from a label or a position.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct PaneId(Arc<str>);

/// "main" always exists; secondary windows are "w<n>" from the persisted counter.
pub struct WindowId(Arc<str>);
/// Splits and groups; persisted counter, never reused, never rewritten by normalisation (egui_tiles #10, #147).
pub struct NodeId(u64);

pub struct Layout {
    version: u32,                                   // LAYOUT_VERSION of the file format
    windows: IndexMap<WindowId, Window>,            // order = creation order; "main" first
    panels: BTreeMap<PaneId, PanelHome>,            // tool panes: one home stripe slot each (ide-shell §2 rule 2)
    last_positions: BTreeMap<PaneId, LastPosition>, // closed panes reopen in place (D13)
    workspace: Option<WorkspaceRef>,                // active overlay + unsaved diff (docking §5.1)
    next_node: u64, next_window: u32,
}

pub struct Window {
    kind: WindowKind,                // Main | Secondary
    placement: Placement,            // monitor key, normal rect, maximized, fullscreen, scale (docking §11.8 item 11)
    regions: RegionsMode,            // Own(Regions) | SharedWithMain (D1)
    center: Area,                    // the editor area
    floats: Vec<Float>,              // in-window floating panels (D3), z-ordered
    focus: Option<NodeId>,           // focused group
    mru: Vec<PaneId>,                // per-window MRU (Ctrl+Tab)
}

pub struct Regions { left: Region, right: Region, bottom: Region }    // top later if a host needs it
pub struct Region { open: Option<PaneId>, size: LogicalPx, collapsed: bool }
pub struct PanelHome { slot: StripeSlot, mode: PanelMode }            // Docked | Overlay | Floating | Window(WindowId)
pub struct StripeSlot { side: Side, order: u16 }

pub struct Area { root: NodeId, nodes: BTreeMap<NodeId, Node> }
pub enum Node {
    Split { axis: Axis, children: Vec<NodeId>, shares: Vec<u16> },    // per mille; n-ary; sums to 1000
    Group { tabs: Vec<PaneId>, active: PaneId, tab_bar: TabBarMode }, // Shown | Hidden | Collapsed
}
pub struct Float { pane: PaneId, rect: LogicalRect }                  // integers only

/// Integers only in the model and the file (F2).
pub struct LogicalPx(u32);
pub struct LogicalRect { x: i32, y: i32, w: u32, h: u32 }
```

**Per-frame pane facts** come from the host and are never persisted. Changing them needs no command:

```rust
pub struct PaneInfo {
    pub title: String,            // already localised by the host
    pub icon: Option<IconRef>,    // host-defined handle; the renderer asks the host to paint it
    pub closable: bool,
    pub dirty: bool,
    pub affinity: Affinity,       // e.g. {"editor"} for documents, {"panel"} for tool panes, extension sets
    pub min_size: LogicalSize,    // content minimum; feeds the solver (docking §11.8 item 2)
    pub placeholder: bool,        // the host does not (yet) know this pane: show a placeholder tab (docking §5.9)
}
```

**Invariants**, checked by `Layout::check()` after every `apply` in debug builds, on every load, and in proptests.
They extend `docking` §5.1:

1. Every `PaneId` appears in exactly one place across the layout: one group tab, one region, or one float.
2. Every group has at least one tab. The exception: a window's last group may be empty, and then shows its empty
   state.
3. A split has at least two children, with as many shares, each between 1 and 1000, summing to 1000.
4. No split has a child split on the same axis.
5. Depth is at most 8 (`docking` §5.1). Every traversal is iterative (F12).
6. Every `active` is in its group, every `focus` names a live group, and every region's `open` is a pane whose
   home is that side.
7. Every panel has one home slot, and no two panels share one.
8. `main` exists. `SharedWithMain` appears only on secondary windows.
9. No float of a pane that also sits in a region or group.

### 2.5 Commands, availability and undo

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DockCommand {
    Open       { pane: PaneId, at: Option<Target> },          // default: last position (D13), else focused group
    Activate   { pane: PaneId },
    MoveTab    { pane: PaneId, to: Target },                  // behind drag, keyboard, carry mode and "Move to…"
    Split      { group: NodeId, side: Side, pane: Option<PaneId> },
    Close      { pane: PaneId },                              // after the host's close guard (see below)
    Resize     { split: NodeId, shares: Vec<u16>, gesture: Option<GestureId> },
    FocusGroup { window: WindowId, to: FocusTarget },         // a group, a direction, or an index
    NewWindow  { pane: Option<PaneId>, placement: Option<Placement> },
    CloseWindow{ window: WindowId },                          // D2: closes its tabs (the host's guards ran first)
    MergeWindow{ from: WindowId, into: WindowId },
    PanelToggle{ pane: PaneId }, PanelSetMode { pane: PaneId, mode: PanelMode },
    RegionResize{ window: WindowId, side: Side, size: LogicalPx, gesture: Option<GestureId> },
    RegionCollapse{ window: WindowId, side: Side, collapsed: bool },
    FloatMove  { pane: PaneId, rect: LogicalRect, gesture: Option<GestureId> },
    WindowPlaced { window: WindowId, placement: Placement },  // from the OS; not undoable
    ApplyWorkspace { workspace: WorkspaceId }, SaveWorkspace { as_: WorkspaceId }, ResetWorkspace,
    Undo, Redo,
}

pub enum Target {
    Group { window: WindowId, group: NodeId, index: Option<usize> },
    Split { window: WindowId, group: NodeId, side: Side },
    WindowEdge { window: WindowId, side: Side },
    Region { window: WindowId, side: Side },
    Float { window: WindowId, rect: LogicalRect },
    NewWindow { placement: Option<Placement> },
}

pub enum Availability { Available, Unavailable { reason: ReasonId, args: ReasonArgs }, Hidden }
pub struct ReasonId(&'static str);         // "dock.split.too_small", "dock.depth_limit", "dock.affinity", ...

pub trait Policy {                          // host veto, extensions (D11), owner restrictions (docking §5.2)
    fn allows(&self, layout: &Layout, cmd: &DockCommand) -> Result<(), (ReasonId, ReasonArgs)> { Ok(()) }
}

impl Dock {
    pub fn availability(&self, cmd: &DockCommand, policy: &dyn Policy) -> Availability;    // pure, cheap
    pub fn apply(&mut self, cmd: DockCommand, policy: &dyn Policy) -> Result<Applied, Refusal>;
    pub fn queue(&mut self, cmd: DockCommand);                // what the renderer does; drained by `logic`
}
pub struct Applied { pub undoable: bool, pub label: ReasonId /* catalog key for the undo menu */ }
```

**Close guard.** Today the guard is a dock `Modal` with a `SaveRequested` event (`dock/seam.rs`). Here a close
button or `Ctrl+W` emits a `DockOutput::close_requests` entry instead. The host checks dirtiness, shows its own
confirmation, and later issues `Close`. The dock keeps no pending-close state, so there is nothing to go stale
(egui_dock #320).

**Undo** (D8):

- `UndoStack` holds `(label, before: Arc<Layout>, after: Arc<Layout>)` entries.
- Layouts are small, so a whole-value snapshot is simpler than inverse commands. It is also correct across
  invariant normalisation, which inverse commands are not.
- Commands with the same `gesture` coalesce into one entry, so one splitter drag is one entry (`docking` §5.2).
- `WindowPlaced`, `Activate` and `FocusGroup` are not undoable.
- The depth is bounded: 100 entries by default.
- cadatomic registers `Undo` and `Redo` as `View` commands. They never touch document undo (`command-arch` §3.1).

**Mapping to cadatomic's registry:**

- Each variant becomes one `View` command, `view.dock.<op>@1` (`docking` §5.2 ids).
- Its args are the variant's fields, an object root as `command-arch` §2.3 requires.
- Availability is `Dock::availability`.
- Recording follows the "one recording rule" (`01-overview` §3.13):
  - layout goes to the layout file;
  - layout commands go to scenarios by anchor;
  - none go to the document command log.

### 2.6 Solver

- **Inputs:** the window's inner size in logical px, for which `ViewportInfo::inner_rect` is authoritative; per-pane
  `min_size`; the region sizes; tab-strip and gap metrics from `DockStyle`.
- **Rules** (`docking` §11.8 item 2):
  - minimums propagate up: a node's minimum is the sum along its axis plus gaps, and the max across it;
  - a separator move clamps by squeezing every neighbour on that side, in order;
  - the editor area takes all extra space, regions keep their size, and splits are proportional to shares;
  - below the sum of minimums, groups collapse to their tab strip in a fixed order (`docking` §5.3).

  Nothing ever lays out at zero or negative width, and nothing in drawing code asserts on geometry (F1).
- **Output:** one `DockFrame` per window, with integer rects for regions, stripes, groups, tab strips, tab chips,
  separators (hit rect = gap plus grab margin, which never overlaps content, F8), drop zones and the preview.
- The model never stores solver output.
- **Preview by simulation:** apply the candidate `MoveTab` to a clone of the layout and solve it. This is done once
  per target change (`docking` §11.8 item 3).
- **Budget:** a solve takes at most 50 µs for 64 groups (W14).

### 2.7 Drag, drop and cross-window

- **The drag machine lives in the core** (`docking` §5.4):
  - `Idle → Armed → Dragging → Idle`;
  - a 6 px or 300 ms threshold, so a click never starts a drag (egui_tiles #79);
  - cancel on Escape, focus loss, or a vanished tab or window (egui_dock #320);
  - hysteresis of 8 px or 80 ms.
- **Input:** `DockInput` events (`Press`, `Move { window, pos }`, `Release`, `Key`, `WindowGone`).
- **Output:** a `DragView` for the renderer and, at release, one `MoveTab` intent.
- **No egui drag ids.** egui's drag state is per viewport, which is why both existing crates cannot cross windows
  (`docking` §2.3).
- **Drop zones** are computed from the model and the last `DockFrame` of the window under the pointer, never
  during painting (`docking` §5.4 table).
- **`DockHost`** (`docking` §11.8 item 6):

  ```rust
  pub trait DockHost {
      /// Our windows under a screen point, top-most first; `None` when a foreign window covers it.
      fn windows_at(&self, screen: ScreenPos) -> Option<SmallVec<[WindowId; 4]>>;
      fn window_rect(&self, w: &WindowId) -> Option<ScreenRect>;       // None on Wayland (not readable)
      fn pointer_screen_pos(&self) -> Option<ScreenPos>;               // None where the platform cannot say
      fn may_be_obscured(&self, w: &WindowId) -> bool { false }        // KDDW rule (docking §11.4)
      fn caps(&self) -> DragCaps;                                      // cross_window_drop, tear_off_follow, toplevel_drag
      fn start_system_move(&mut self, _w: &WindowId) -> bool { false } // Windows snap (docking §11.8 item 7)
      fn start_toplevel_drag(&mut self, _w: &WindowId, _grab: LogicalVec) -> bool { false } // Wayland hybrid (D7 fork)
  }
  ```

- **`egui_berth::EguiDockHost`** is the default implementation:
  - window rects come from each viewport's `ViewportInfo::outer_rect`;
  - the pointer comes from the source viewport's pointer position;
  - z-order comes from the dock's own activation history.

  That is enough for Windows, macOS and X11, where a pressed button keeps delivering moves to the source window
  (`docking` §4.2) [unverified for winit 0.30: experiment W3].
- **Wayland** reports `caps().cross_window_drop == false`. The renderer then offers the fallbacks (`docking` §5.5):
  - menus: "Move to New Window" and "Move to Window ▸";
  - carry mode;
  - drops on the source window's tab strip.

  After the winit fork (D7), cadatomic supplies a `DockHost` with `start_toplevel_drag`, following ADS's hybrid
  recipe (`docking` §11.4).
- **Tear-off at release** (D17): the ghost and preview stay in the source window's overlay layer, and the new OS
  window is created by `MoveTab { to: NewWindow }` after the release.
- **winit limits:**
  - egui 0.37 uses winit 0.30.13, which cannot start a drag (`docking` §4.2). `start_drag` is in winit 0.31
    [unverified coverage].
  - Wayland positions are neither readable nor settable, so placement on Wayland stores sizes and maximised state
    only (`docking` §5.6).
  - Wayland detection must come from the backend in use, not from `XDG_SESSION_TYPE`, which is empty under WSLg
    (ADS #486, `docking` §11.4).

### 2.8 The egui renderer (`egui_berth`)

**Frame shape.** The renderer follows egui 0.37's `App::logic`/`App::ui` split (`01-overview` §3.3) and svelters'
SVP1 split (`CADATOMIC.md` §2.8):

```rust
pub struct DockUi { style: DockStyle, text: DockText, viewports: ViewportMode, /* caches, drag view */ }

impl DockUi {
    /// Between frames (App::logic): drain queued intents (auto_apply) or hand them out; read every viewport's
    /// ViewportInfo from `ctx.input(|i| i.raw.viewports)`; solve each window; diff visibility; return events.
    pub fn logic(&mut self, ctx: &egui::Context, dock: &mut Dock, host: &dyn PaneHost) -> DockLogic;

    /// The pass (App::ui) for the root window. Secondary windows are shown from here as viewports
    /// (immediate) or registered as deferred viewports (§2.8 "Deferred"). Draws chrome, calls
    /// `host.pane_ui` for every visible pane exactly once, and collects intents.
    pub fn show(&mut self, ui: &mut egui::Ui, dock: &Dock, host: &mut dyn PaneHost) -> DockOutput;

    pub fn is_settled(&self) -> bool;
    pub fn anchor_rect(&self, anchor: &str) -> Option<(WindowId, egui::Rect)>;
}

pub struct DockLogic { pub events: Vec<PaneEvent>, pub intents: Vec<DockCommand> /* when !auto_apply */ }
pub struct DockOutput {
    pub intents: Vec<DockCommand>,               // queued; applied at the next `logic` or by the host's registry
    pub close_requests: Vec<CloseRequest>,       // panes and windows the user asked to close (the host guards)
    pub context_menu: Option<ContextMenuRequest>,// the host draws menus from its registry (docking §5.5 item 1)
}

pub trait PaneHost {
    /// The seam: draw the pane's content into `ui` (its rect is the pane body). Called at most once per pass
    /// per visible pane, in exactly one viewport.
    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: &PaneId);
    /// Facts for chrome; None → placeholder tab ("not available"), never dropped (docking §5.9).
    fn pane_info(&self, pane: &PaneId) -> Option<PaneInfo>;
    /// Optional: paint a tab icon or a stripe button icon.
    fn paint_icon(&self, _painter: &egui::Painter, _rect: egui::Rect, _icon: &IconRef) {}
}
```

**Painting and interaction:**

- Chrome is drawn with `Painter` shapes and `Ui::interact(rect, id, sense)` at the `DockFrame` rects.
- Pane bodies get a child `Ui` built with `UiBuilder::new().max_rect(body).id_salt(("berth.pane", pane))` and
  `.accessibility_parent(tab_panel_id)`.
- Content is registered before chrome hit areas over overlapping rects, so content wins ties (F8).
- Splitter hit areas never cover the content's scrollbar band (egui_tiles #158).
- Overlays (drop zones, preview, ghost) live on one `egui::LayerId` per window,
  `Order::Foreground, Id::new(("berth.overlay", window))`. Pane content never changes layer during a drag
  (egui_dock #317, #278).

**egui ids:**

- `Id::new(("berth", window_id, node_id))` for chrome.
- `Id::new(("berth.pane", pane_id))` for the pane body. It does not depend on the location, so egui memory keyed
  under the pane (scroll, collapsing state) survives moves and reloads. egui `Memory::data` is shared by all
  viewports (`docking` §4.3).
- There is one dock per egui context. A second dock needs `DockUi::with_salt`, which egui_tiles and egui_dock
  required implicitly (F4: #88, #11, #64).

**Viewports.** Each `WindowId` maps to `ViewportId::from_hash_of(("berth", window_id))`, which is stable across
reloads and restarts.

- **Immediate (v1 default):**
  - Secondary windows use `Context::show_viewport_immediate`, which borrows `host` for the duration.
  - Every repaint costs a pass per window (`docking` §4.3).
  - This is fine for two or three windows and needs nothing from the host.
- **Deferred (opt-in, needs the host's cooperation; experiment W2):**
  - `Context::show_viewport_deferred` requires `Fn(&mut Ui, ViewportClass) + Send + Sync + 'static`
    (egui 0.37 `context.rs:4608`), and eframe calls it on the main thread, possibly while the root sleeps.
  - The renderer captures only the `WindowId` in the closure. It reaches the dock and host through a thread-local
    `DeferredSlot` that the host fills once with `Rc<RefCell<dyn DeferredHost>>`, the same trick as svelters EM2's
    scene thread-local (`TRACKS` EMBED EM2).
  - A main-thread assertion turns misuse into a panic. There is no `unsafe` and no fake `Send`.
  - Each window then repaints independently, and the renderer requests repaints per viewport.
- **Embedded:** when `Context::embed_viewports()` is true, the default in egui and in kittest, and false on eframe
  desktop (`eframe/src/native/winit_integration.rs:59`), deferred and immediate viewports are drawn as
  `egui::Window`s inside the root (`ViewportClass::EmbeddedWindow`). The renderer handles this class explicitly:
  - window chrome is drawn inside;
  - the effective egui viewport of those panes is `ViewportId::ROOT` (§4.1).
- **Closing:** when `close_requested()` is set on a secondary viewport, the renderer sends `CancelClose` and puts a
  `CloseRequest::Window` in the output, so the host decides (`docking` §5.6, cooperative close).
- **Focus:** on window activation and after a cross-window move, the renderer requests focus for the moved pane's
  body Ui id. The host restores inner focus, for example svelters' last-focused node (`docking` §5.7, §6.6).

**Keyboard** (`docking` §5.7): the renderer exposes `DockKeys` (F6 regions, Ctrl+Tab MRU, Ctrl+1…9, carry mode)
as data. The host binds them through its keymap, and `DockKeys::default_bindings()` is offered for hosts without
one.

### 2.9 Accessibility

The renderer emits these AccessKit nodes through `accesskit_node_builder`:

- `Window` per OS window: egui keeps one tree per viewport, which matches `accesskit_winit` (`docking` §5.8).
- `TabList` per group, and `Tab` with selected state and a close action.
- `TabPanel` labelled by its tab. The pane body is its accessibility child.
- `Splitter` with a value, plus increment and decrement actions.
- `Toolbar` for stripes.

Moves keep node ids, because they derive from `PaneId`. The dock reports an `Announcement` in `DockOutput`, for
example "Sketch 1 moved to window 2, right group", from `DockText` templates. The host routes it to a live region,
since egui 0.37 has no announce API for widgets [unverified]. Carry mode makes every layout command reachable
without a pointer (`docking` §5.5 item 2). W9 (Orca, NVDA, VoiceOver across two windows) stays cadatomic's
experiment.

### 2.10 Layout file and migration

- The format is `serde_json`, with `{ "version": N, … }`:
  - integers only (shares per mille, logical px);
  - string ids;
  - keys sorted by a `BTreeMap` serialisation (no `Value` round trip needed);
  - no screen rects of content (F2).
- `berth_core::file::{to_string, from_str}`. `from_str` runs parse → migrate → validate:
  - Migration is `migrate_vN_to_vN1(serde_json::Value)` functions, chained, with a golden file per version kept
    forever (F3).
  - A newer file than the build reads is refused.
- The **dry run**: the result is a fresh `Layout`, swapped in as one step by `Dock::replace_layout` (ADS lesson,
  `docking` §11.8 item 10). The host decides what happens on failure. cadatomic keeps `layout.json.bad-<ts>` and
  shows a toast (`docking` §5.9).
- **Unknown panes stay** as placeholders until the user closes them (`docking` §5.9).
- **Restore clamps** (`docking` §11.8 item 11, W19):
  - a missing monitor maps to the primary monitor at the saved logical size, clamped, never zero;
  - after a scale change the rect is clamped to the work area;
  - minimum sizes are re-applied.
- **JSON Schema** comes from `schemars` (feature). A `berth-lint` binary validates a file and prints the first
  violated invariant; it is cadatomic's `cadatomic layout lint` (`docking` §11.8 item 15).
- **v1 → v2 migration for cadatomic** (an egui_tiles tree to this model, `docking` §5.11 step 3) lives in
  cadatomic's seam, not in the crate. It is cadatomic's file history.

### 2.11 Testing

| Layer | Tool | What | Pass |
|---|---|---|---|
| Model | `proptest` over random command sequences | invariants, save→load identity, depth limit, no float in the file, undo then redo is the identity | 10⁶ ops clean (W1) |
| Solver | proptest plus benches | no overlap, no negative size, minimums kept when the window allows, preview equals the post-drop rect | W14 numbers |
| File | a fuzzer (`cargo fuzz` and schema-driven proptest) plus a corpus of bad files | no panic; rejections are typed | W15 |
| Drag | a scripted `DockHost` double (3 overlapping windows, a foreign window between) | the top-most of our windows is found; a foreign window blocks | W16 |
| Renderer | `egui_kittest` 0.37 `Harness` with real pointer events (`drag_at`, `drop_at`, `hover_at`) | tabs, splitters and drops by role and label; a click never drags; target changes only at zone boundaries over 1000 random paths | W12 |
| Seam | kittest with a recording `PaneHost` | the event order of §4.1 over scripted sessions; exactly one `pane_ui` per visible pane per pass | §4.1 rules |
| Windows | kittest draws secondary viewports **embedded**, because the harness only handles the root viewport's output (`egui_kittest/src/lib.rs` `handle_viewport_commands`) and `embed_viewports` defaults to true. Real deferred viewports get one eframe smoke test under Xvfb in the crate's CI, plus W2 in cadatomic | idle frames per viewport equal 0 | W2 |
| Snapshots | kittest `render` (wgpu feature) for the chrome in light, dark and high-contrast | stable images | — |

### 2.12 Milestones

| Id | Content | Size (lane-days) | Maps to |
|---|---|---|---|
| **BD0** | The repo: workspace, licences, CI (check, clippy, test, wasm32 build of the core, MSRV, fuzz smoke), egui pin, a demo example | 1 | — |
| **BD1** | Core: model, commands, availability, invariants, undo, the file with migrations and golden files, profiles and workspaces as data, proptests | 5–7 | TRACKS DK1 |
| **BD2** | Core: solver, drop resolver, drag machine, `DockHost` trait and test double, W14 and W16 | 4–6 | DK1 (its solver half) |
| **BD3** | Renderer, single window: chrome, regions and stripes, the pane seam with events (§4.1), a11y roles, keyboard, kittest suite | 6–8 | DK2's crate half |
| **BD4** | cadatomic adoption (in cadatomic): `dock/seam.rs` over berth, v1→v2 migration, egui_tiles and its fork deleted, `ShellPanes`/`PinnedPanes` (`app.rs`) as `PaneHost` | 3–4 | DK2's cadatomic half |
| **BD5** | Multiple windows: immediate then deferred viewports (W2), placement and off-screen recovery (W13, W19), in-window floats (D3), overlay panels (D15), `SharedWithMain` regions (D1) | 8–12 | DK3 |
| **BD6** | Cross-window drag and tear-off on Windows, macOS and X11 through `EguiDockHost`, carry mode on every platform; later the Wayland hybrid on cadatomic's winit fork (W17) | 6–9, plus 3–5 for Wayland | DK4 |

The total is 33–47 lane-days. BD1–BD2 need nothing from svelters. Using real Svelte roots in panes (W4) needs
svelters EM2 (SV18) only from BD5 on.

### 2.13 Repository layout

```text
berth/                         (own repo; cadatomic sessions own it)
  Cargo.toml                   workspace; egui = "=0.37.0"; rust-version = "1.95"; unsafe_code = "forbid"
  crates/berth-core/           model, commands, solver, drag machine, file, undo  (serde, indexmap, smallvec; schemars opt)
  crates/egui_berth/           renderer, viewports, a11y, EguiDockHost            (egui; berth-core)
  crates/berth-lint/           layout file linter (bin)
  examples/demo/               eframe dev-dependency: two windows, regions, floats
  tests/golden/layout-vN.json  one per file version, kept forever
  fuzz/                        cargo-fuzz targets (file, command sequences)
  LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md   (egui_tiles notice if code is borrowed)
```

### 2.14 Licensing

- **egui_tiles** (MIT OR Apache-2.0) and **egui_dock** (MIT): code may be borrowed, with the notice kept in
  `THIRD_PARTY_NOTICES.md`.
- **KDDW** (GPL) and **ADS** (LGPL-2.1): designs and bug histories only. No code, data files or artwork,
  including KDDW's indicator PNGs. This repeats `docking` §11.
- **VS Code** (MIT): behaviour only.
- The crate's own licence is O2.

## 3. The code-editor crate (`quire`)

### 3.1 Goals and non-goals

**Goals (P0 of `code-editor` §8, split as TRACKS ED1/ED2):**

- **A buffer and edit model in an egui-free core:**
  - a rope (`ropey` 1.6) behind `Text`;
  - `ChangeSet`, composable and invertible, Helix's model (`code-editor` §4.2);
  - `Transaction` (changes, selection, effects, annotations, base version);
  - anchors with bias;
  - `Snapshot`s that clone in O(1) for background work;
  - undo groups with Monaco's closing rules;
  - CRLF and BOM recorded.
- **Editing behaviour as data-driven commands:**
  - cursor and word movement with per-platform word rules (widget-survey pins 1–2);
  - multi-click selection;
  - typing operations: auto-close, auto-surround, overtype, indent on enter, comment toggle;
  - find and replace (`regex-automata` with a size limit, `models` §3);
  - multi-cursor and column selection (ED2);
  - folding ranges (ED2).
- **Syntax:**
  - a `Highlighter` trait;
  - Monaco's compact per-line token store;
  - a tree-sitter implementation for Svelte, TypeScript, JavaScript and CSS with injections, off the UI thread;
  - native CST highlighters from hosts plug into the same trait (`code-editor` §4.3).
- **Diagnostics in, per source and version**, mapped forward through later edits. Squiggles, gutter marks and
  hover text.
- **The egui widget:**
  - virtualised lines, a per-line galley cache and a monospace fast path;
  - gutter, caret and selections;
  - IME preedit inline;
  - clipboard, including Linux PRIMARY;
  - AccessKit paging;
  - a find bar.
- **Performance targets** E1, E2, E3, E5 and E6 (§3.9).
- **A clean wrap as svelters `<code-editor>`** (§4.2).

**Non-goals (owned by the host or later work):**

- An LSP client, language services, file I/O, document revisions in an object store, Problems, and a command
  registry.
  - These are cadatomic's (`code-editor` §4.4, `cadatomic-lsp`) and LIVEIDE's.
  - The editor takes their results through a small surface: diagnostics now, then completions, hover and inlay
    hints as injected text in P1.
- CRDT collaboration (CE9). The editor stays CRDT-agnostic but ready for it (§3.4 "Remote").
- Proportional fonts (CE11: monospace only).
- Bidi (CE10: later).
- The minimap (later).
- The diff and merge editors (P2, `code-editor` §4.11). These are a second widget in the same repo later, on the
  same core.

### 3.2 Prior art and what to take

| Source (licence) | What it is | Take | Do not take |
|---|---|---|---|
| **CodeMirror 6** (MIT) | Immutable `EditorState`, `Transaction` (changes, selection, effects, annotations), `ChangeSet` with `mapPos`, `StateField`s, facets, `dispatch`, `history()` grouping by time and `userEvent`, a viewport with only visible lines rendered, a height map | **The state/transaction model and the host-dispatch hook** (`dispatchTransactions`): how a host intercepts, refuses or rebases every edit. Annotations (`userEvent: "input.type"`, `"undo"`, remote origin) drive undo grouping. Its height map is a source for our height index | The extension/facet system as a public API surface (too open for a typed Rust API; we take fixed fields plus one `Effect` enum) |
| **Monaco / VS Code** (MIT) | `TextModel` piece tree, edit-stack grouping, an interval tree for decorations, view-model line projections, cursor and typing operations, ~3,147 unit tests (`code-editor` §1) | **Algorithms and tests**, literal translation allowed (CE2): grouping rules, the decoration interval tree, `ViewModelLines` projections, cursor and typing ops, the snippet parser, fixtures from `cursor.test.ts` (E11) | The piece tree (a rope gives the same bounds, `code-editor` §1.1); DOM, textarea and EditContext input |
| **Zed** (`text`, `rope`, `editor`: GPL-3.0; `sum_tree`: Apache-2.0) | A CRDT buffer on `SumTree`, anchors as (insertion id, offset), a syntax map of injected layers reparsed on a background thread, multibuffers (`code-editor` §2.1) | **Designs only** for the GPL parts: anchors, snapshots for background threads, syntax layers in a tree. `zed-sum-tree` 0.2.0 is Apache and usable if a summary tree beats our Fenwick index (unlikely in P0) | Any GPL code |
| **Helix** (MPL-2.0); `tree-house` 0.4.0 (MPL-2.0) | `Transaction = ChangeSet + Selection`, composable and invertible changes, a history tree, ropey, tree-house incremental highlighting with injections | The `ChangeSet` model and its laws; tree-house as the candidate highlighter (E5). MPL is file-level copyleft: depending on it is fine; copying its files means keeping them MPL (§3.13) | A terminal-shaped rendering model |
| **xi-editor** (Apache-2.0) | A persistent rope; the 2020 retrospective (`code-editor` §2.1) | Keep editing and the buffer in the UI process with snapshots for workers; async front/back separation was "not a good idea"; CRDTs did not pay for a single user | Process separation; a highlighting-specialised plug-in API |
| **Lapce / floem** (Apache-2.0 / MIT; `floem` 0.2.0, 2024-11) | `lapce-xi-rope` 0.4.0 (2025-12), an editor core split from the view, an LSP client | Confirms the core/view split. `lapce-xi-rope` is a third rope candidate | Floem's reactive runtime (svelters has its own) |
| **cosmic-text** 0.19.0 (MIT OR Apache-2.0); glyphon | `Buffer` of `BufferLine`s, each with its own shape and layout cache, invalidated per line; an `Editor` on top | **Per-line shaping caches** invalidated by line edits: the same idea as our galley cache. A text stack is not needed: egui 0.37 shapes with harfrust and skrifa (`code-editor` §2.2) | Replacing egui's text stack (parley stays the E8 fallback for bidi) |
| **egui `TextEdit`** 0.37 (MIT OR Apache-2.0) | One galley for the whole text, a `String`, one cursor range, time-based undo (`code-editor` §2.3) | Read its IME handling (`IMEOutput` with `purpose`, `rect`, `cursor_rect`, `should_interrupt_composition`; `Memory::owns_ime_events` = focus) and its AccessKit `TextRun` code (`text_selection/accesskit_text.rs`) | It as a base (whole-text galley, no virtualisation) |
| **egui_code_editor** 0.4.2, 2026-10-07, still egui ^0.36 (MIT) | A `TextEdit` plus a layouter with token rules | Nothing structural | It as a base (`code-editor` §2.1; `widget-survey` row 242) |
| **ropey** 1.6.1 (MIT) vs **crop** 0.4.3 (MIT) | A B-tree rope with char, line and UTF-16 indexing vs byte-indexed with cheap clones | ropey first, behind `Text`; E2 decides. Note ropey 2.0 beta (byte-indexed) | Leaking the rope type in the public API |

### 3.3 Where this design follows or departs from cadatomic's research

| Source | Their position | Here | Why |
|---|---|---|---|
| `code-editor` §4 diagram, §4.14 | `TextBuffer`, `ChangeSet`, anchors, `SyntaxProvider` in `cadatomic-core`; `CodeEditor` in the framework (L3) | **Superseded.** Buffer, change sets, anchors, history, the highlighter trait and the tree-sitter provider live in the editor repo (`quire-core`, `quire-syntax`). cadatomic keeps `TextDocumentModel`, the `text.edit` command, `LanguageService` and the LSP client | User decision. `quire-core` keeps cadatomic-core's constraints (no egui or tokio, wasm32), so cadatomic-core can depend on it |
| `code-editor` §4.6, `models` §5.5 | The editor is one scene node, with paint in sub-caches on the retained scene | **Superseded.** An immediate egui widget with its own caches. It is still wrappable as a scene kind (§4.2) | User decision; svelters' scene draws provider kinds by calling their `ui` (`EGUI.md` §3.3) |
| `live-ide` §6.5, §7, J9 | "A code editor component in the framework", with LSP client and Problems inside | **Superseded** for the widget (own repo). The LSP client and Problems stay with the host | User decision |
| `code-editor` §4.1, CE3 | `text.edit` is the one mutating command; one revision per closed undo group | **Agree.** `Transaction`s are what `text.edit` carries. In `UndoMode::Host`, the editor emits `UndoGroupClosed` and leaves revisions and undo (head moves) to the host | CE3 |
| `command-arch` §1 item 8, §3.6 | Text undo is `TextEdit`'s own and "recorded nowhere" | **Superseded by CE3** for code documents. Scratch editors (query boxes, REPL cells) keep a local history (`UndoMode::Local`) | CE3 |
| `code-editor` §4.2 | ropey 1.6 first, crop in E2 | **Agree**, with ropey 2.0 beta and `lapce-xi-rope` added to E2 | — |
| `code-editor` §4.3 | `SyntaxProvider: Send + Sync` with `parse`, `highlights`, `brackets`, `folds`, `outline`, `indent`, `injections` | **Agree in shape.** `quire_core::syntax::Highlighter` keeps those methods, with tree types made associated, so tree-sitter and native CSTs both fit | — |
| `code-editor` §4.7, CE4 | Keys → registry commands via VS Code's keymap and context keys | **Agree, split:** the editor ships `EditorCommand` (VS Code ids) and a default VS Code keymap as data. With `KeyHandling::Host` it does no key lookup itself and the host's layered keymap (`command-arch` §3.5) dispatches `EditorCommand`s | CE4; cadatomic's `TextFocus` layer |
| `code-editor` §4.8 | AccessKit `MultilineTextInput` with `TextRun`s, a 500-line page | **Agree** (§3.8) | — |
| `models` §4.2, §5.4, §5.5 | Versioned positions; stale-while-revalidate jobs tethered to versions; a Fenwick height index; ~2 ms UI budget | **Agree, reimplemented in the crate** (it cannot depend on cadatomic's `RowHeights`) | Independence |
| `models` §3 | `regex-automata` with a compile size limit; "highlight only what is visible" | **Agree** for find | — |
| `live-ide` §6.4 | Unsaved buffer → compile on a 150 ms debounce → diagnostics; save → HMR | **Agree.** The editor supplies versioned snapshots and `Changed` and `SaveRequested` events; the debounce is the host's (§4.2) | — |
| `widget-survey` W4, rows 208/242 | Text fields on `TextEdit`; the code editor "on TextField core" | **Inverted:** the editor core comes first. A one-line mode (`code-editor` §4.12, formulas) and later multi-line fields can reuse it if pins 1–6 show `TextEdit`'s gaps | `code-editor` §2.3 |
| `native-ext` §6.1 | Hosts own threads | **Agree:** a `Spawner` trait; the editor never spawns | — |

### 3.4 Core model (`quire-core`)

```rust
/// Monotonic per EditorState; bumps on every applied change (not on selection-only transactions).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Version(pub u64);

pub struct Text { rope: ropey::Rope, line_ending: LineEnding, bom: bool }    // LF in memory (code-editor §4.1)
impl Text {
    pub fn len_bytes(&self) -> usize; pub fn len_lines(&self) -> usize;
    pub fn line(&self, i: usize) -> TextSlice<'_>; pub fn slice(&self, r: Range<usize>) -> TextSlice<'_>;
    pub fn byte_to_line_col(&self, b: usize) -> (usize, usize); pub fn utf16_to_byte(&self, line: usize, col16: usize) -> usize;
    pub fn chunks(&self) -> impl Iterator<Item = &str>;                       // for regex and the compiler
}

/// Sorted, non-overlapping edits against one base length; compose, invert, map positions (Helix / CM6 laws).
pub struct ChangeSet { /* retain / delete / insert ops */ }
impl ChangeSet {
    pub fn replace(base_len: usize, edits: impl IntoIterator<Item = (Range<usize>, impl Into<Arc<str>>)>) -> Result<Self, ChangeError>;
    pub fn compose(&self, next: &ChangeSet) -> Result<ChangeSet, ChangeError>;
    pub fn invert(&self, before: &Text) -> ChangeSet;
    pub fn map_pos(&self, pos: usize, bias: Bias) -> usize;
    pub fn diff(old: &Text, new: &Text) -> ChangeSet;                         // imara-diff; for reloads (keeps carets)
}

pub struct Selection { ranges: SmallVec<[SelRange; 1]>, primary: usize }     // multi-cursor from day one in the type
pub struct SelRange { pub anchor: usize, pub head: usize, pub goal_column: Option<u32> }

pub struct Transaction {
    pub base: Version,                       // refused if != state.version() unless rebasable (see Remote)
    pub changes: ChangeSet,
    pub selection: Option<Selection>,        // after the changes
    pub effects: SmallVec<[Effect; 2]>,      // fold/unfold, scroll-into-view, set-diagnostics, ...
    pub origin: Origin,                      // Input(InputKind) | Command(&'static str) | Undo | Redo | Remote | Reload
    pub group: GroupHint,                    // Continue | Close | Isolated (Monaco grouping input)
}

pub struct Snapshot { pub text: Text /* Arc-shared */, pub version: Version }   // O(1); Send + Sync

pub struct EditorState {
    text: Text, version: Version, selection: Selection, selection_version: u64,
    history: History, mode: UndoMode,                                 // Local | Host
    recent: RecentChanges,                                            // last N change sets, for mapping stale data
    tokens: TokenStore, diagnostics: Diagnostics, folds: Folds, find: Option<FindState>,
    config: EditorConfig, saved_at: Option<Version>, focused: bool,
}
impl EditorState {
    pub fn new(text: &str, config: EditorConfig) -> Self;
    pub fn version(&self) -> Version; pub fn snapshot(&self) -> Snapshot; pub fn text(&self) -> &Text;
    pub fn selection(&self) -> &Selection;
    pub fn apply(&mut self, tr: Transaction) -> Result<Applied, Refusal>;                 // the only mutation path
    pub fn apply_remote(&mut self, base: Version, changes: ChangeSet, origin: Origin) -> Result<Applied, Refusal>;
    pub fn command(&self, cmd: &EditorCommand) -> Result<Option<Transaction>, Refusal>;   // pure: command → transaction
    pub fn set_diagnostics(&mut self, source: SourceId, at: Version, diags: Vec<Diagnostic>) -> Result<(), Stale>;
    pub fn set_tokens(&mut self, at: Version, lines: Range<usize>, runs: TokenRuns) -> Result<(), Stale>;
    pub fn mark_saved(&mut self, at: Version); pub fn is_dirty(&self) -> bool;
    pub fn is_settled(&self) -> bool;
}
```

- **Positions** are UTF-8 byte offsets (`code-editor` §1.2). LSP's UTF-16 columns are converted at the host's LSP
  boundary with `Text::utf16_to_byte` and its inverse.
- **Stale data:**
  - `RecentChanges` keeps the last 64 change sets (configurable).
  - Diagnostics and tokens computed at an older version are mapped forward through them.
  - Data older than the window is refused as `Stale` and the host recomputes.
  - Until fresh tokens arrive, the previous tokens are shifted by the change set, so nothing flickers
    (`code-editor` §4.3, `models` §5.4).
- **Remote and CRDT readiness** (CE9):
  - The editor holds no CRDT. A collaborative host merges with Eg-walker or yrs (`code-editor` §6) and feeds the
    result in as `apply_remote(base, changes, Origin::Remote)`.
  - Selections, anchors, diagnostics and folds map through.
  - Local undo skips remote changes (per-user undo by origin, as in CodeMirror's history).
  - Anchors are the only persistent positions the crate hands out (`code-editor` §4.1 "anchors").
- **Undo groups** (`code-editor` §4.1; Monaco `editStack.ts`): typing coalesces until one of these closes the
  group:
  - a word boundary, per the platform policy;
  - a cursor jump;
  - a pause of more than 1 s;
  - a non-typing command;
  - a focus change;
  - `GroupHint::Close`.

  When a group closes, the output carries `UndoGroupClosed { from: Version, to: Version, changes: ChangeSet,
  selection_before, selection_after }`:
  - In `UndoMode::Host` this is the CE3 revision boundary. `Undo` and `Redo` are emitted as intents, and the host
    answers with transactions whose `origin` is `Undo` or `Redo`.
  - In `UndoMode::Local` the editor applies its own history.
- **Large-file policy:** above thresholds measured by E3 (Monaco's are 20 MB or 300k lines, and lines over 10k
  characters), tokenisation, folding and brackets turn off, and long lines render a prefix. The state reports
  `Degraded { reasons }`, so the host can say so.

### 3.5 Commands

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(JsonSchema))]
#[serde(tag = "id")]
pub enum EditorCommand {
    #[serde(rename = "cursorMove")]      CursorMove { to: Motion, select: bool },     // Motion: Left, WordRight, LineEnd, PageDown, …
    #[serde(rename = "type")]            Type { text: String },
    #[serde(rename = "deleteLeft")]      DeleteLeft, /* deleteWordLeft, deleteRight, … */
    #[serde(rename = "editor.action.clipboardPasteAction")] Paste { text: String, per_cursor: bool },
    #[serde(rename = "editor.action.commentLine")]          CommentLine,
    #[serde(rename = "editor.action.insertCursorBelow")]    InsertCursorBelow,  // ED2
    #[serde(rename = "editor.action.addSelectionToNextFindMatch")] AddNextMatch, // ED2
    #[serde(rename = "actions.find")]    Find { query: FindQuery },
    #[serde(rename = "undo")] Undo, #[serde(rename = "redo")] Redo,
    #[serde(rename = "workbench.action.files.save")] Save,                        // emits SaveRequested only
    /* fold / unfold (ED2), indent / outdent, selectAll, … */
}
```

- Ids follow VS Code wherever one exists, so imported keymaps and agents recognise them (`code-editor` §4.9, CE4).
- `EditorState::command` turns a command into a transaction without applying it. That is the pure
  preview/validate step `command-arch` §3.3 needs.
- **cadatomic's mapping:**
  - cursor and selection commands are `View` commands, recorded by scenarios only;
  - text-changing commands resolve to one `text.edit` with the resulting `ChangeSet`, an `Edit` command that is
    logged as resolved, not as keystrokes (`code-editor` §4.1).

### 3.6 Syntax (`quire-core::syntax` + `quire-syntax`)

```rust
pub trait Highlighter: Send + Sync + 'static {
    type Tree: Send + Sync + 'static;
    fn language(&self) -> &str;
    fn parse(&self, snap: &Snapshot, prev: Option<(&Self::Tree, &ChangeSet)>, cancel: &CancelFlag) -> Option<Self::Tree>;
    fn highlights(&self, tree: &Self::Tree, snap: &Snapshot, lines: Range<usize>) -> TokenRuns;   // compact, per line
    fn brackets(&self, _t: &Self::Tree, _lines: Range<usize>) -> Vec<BracketPair> { vec![] }
    fn folds(&self, _t: &Self::Tree) -> Vec<FoldRange> { vec![] }
    fn indent(&self, _t: &Self::Tree, _snap: &Snapshot, _line: usize) -> Option<IndentHint> { None }
}
pub trait Spawner { fn spawn(&self, job: Box<dyn FnOnce() + Send>); }   // host-owned threads (native-ext §6.1)
pub struct SyntaxDriver<H: Highlighter> { /* debounces, spawns parse jobs on snapshots, applies results by version */ }
```

- **Token store** (Monaco's design, `code-editor` §1.1, §4.3):
  - per line, `(start_byte: u32, style: u32)`;
  - `style` packs the token type (from a fixed list aligned with LSP semantic token types), modifiers and the
    language;
  - theme resolution happens at paint time, so a theme switch repaints without re-tokenising.
- **`quire-syntax`:**
  - `tree-sitter` 0.27 with grammars `tree-sitter-svelte-ng` 1.0.2, `-typescript` 0.23.2 and `-css` 0.25.0 (all
    MIT);
  - Svelte's `<script lang="ts">` and `<style>` injections;
  - highlight queries in Helix's directory layout;
  - one Cargo feature per language, because grammars are C builds.
- **Highlighter engine choice** (O8): E5 picks between `tree-house` 0.4 and our own query runner over
  `tree-sitter`'s `QueryCursor`.
- **Grammar ABI:** the grammars use `tree-sitter-language` 0.1, which works with tree-sitter 0.27 through
  `LanguageFn` [unverified: E5 checks the ABI].
- **UI-thread budget:**
  - the UI thread never parses;
  - parse and highlight run through `Spawner`, and results apply only if their version still matches or can be
    shifted;
  - E4's inline exception (a native reparse under 0.5 ms) is the host's choice through a synchronous `Spawner`.

### 3.7 The widget (`egui_quire`)

```rust
pub struct CodeEditor<'a> { state: &'a EditorState, id_salt: egui::Id, theme: &'a EditorTheme, keys: KeyHandling, a11y_name: Option<&'a str> }
pub enum KeyHandling { Builtin(Keymap /* VS Code default */), Host }

impl<'a> CodeEditor<'a> {
    pub fn new(state: &'a EditorState, id_salt: impl std::hash::Hash) -> Self;
    /// Size for layout without drawing (lines × line height, gutter width), for svelters' measure (§4.2).
    pub fn measure(&self, ctx: &egui::Context, available: egui::Vec2) -> egui::Vec2;
    /// Draw and interact inside `rect` (the scene gives the rect); never mutates `state`.
    pub fn show_in(self, ui: &mut egui::Ui, rect: egui::Rect) -> EditorOutput;
    /// Convenience: allocate available space and `show_in`.
    pub fn show(self, ui: &mut egui::Ui) -> EditorOutput;
}

pub struct EditorOutput {
    pub response: egui::Response,
    pub transactions: Vec<Transaction>,          // intents; host dispatches (or EditorState::apply directly)
    pub commands: Vec<EditorCommand>,            // unresolved commands when the host wants them (palette, macros)
    pub events: Vec<EditorEvent>,
    pub caret_rect: Option<egui::Rect>,          // anchors, tutorials, IME placement
}
pub enum EditorEvent {
    SaveRequested,
    UndoRequested, RedoRequested,                // UndoMode::Host
    FocusChanged(bool),
    HoverAt { pos: usize, version: Version },    // host may answer with hover content (P1)
    LinkActivated { range: Range<usize>, version: Version }, // Ctrl+click (go to definition, P1)
    ContextMenu { pos: usize },                  // host draws the menu from its registry
    DiagnosticAction { command_id: String, args: serde_json::Value }, // a diagnostic's action chosen (§4.2)
}
// Applying transactions yields (from EditorState::apply → Applied):
pub struct Applied { pub version: Version, pub changes: ChangeSet, pub selection_changed: bool, pub closed_group: Option<UndoGroup> }
```

**Rendering** (`code-editor` §4.6):

- **Visible lines** come from the scroll offset through a Fenwick height index (`models` §5.5):
  - uniform heights without wrap;
  - measured heights by line with wrap, which is ED2.
- **Per-line galley cache:**
  - the key is (line content hash, token style hash, font id, pixels per point, wrap width);
  - built from an `egui::text::LayoutJob` with one section per token run, through `ctx.fonts_mut`;
  - LRU-bounded;
  - only lines entering the viewport are shaped.
- **Monospace fast path** (CE11): column→x is arithmetic for ASCII. Tabs, wide CJK, emoji and combining marks use
  galley positions.
- **Scrolling** translates existing shapes. `Context::transform_layer_shapes` (0.37, `context.rs:3596`) is the
  option for a dedicated layer. E1 measures whether it pays.
- **Overlays:** caret, selections, squiggles, find matches and bracket highlights are cheap rect and path shapes
  each frame they change.
- **The caret blink** uses `request_repaint_after` on the editor's viewport only, stops after 5 s idle, and stays
  still under reduced motion.

### 3.8 Input, IME, clipboard and accessibility

- **Keys:**
  - With `KeyHandling::Builtin`, the keymap maps chords to `EditorCommand`s, and `Event::Text` maps to
    `Type { text }`.
  - With `KeyHandling::Host`, keys are not consumed, except `Event::Text` and IME events while focused. The host's
    `TextFocus` layer (`command-arch` §3.5) dispatches commands.
  - `KeyboardShortcut::conflicts_with_text_editing` (0.37) lets the host drop conflicting chords from lower layers.
- **IME** (egui 0.37 `ImeEvent`, `data/input/ime_event.rs`):
  - `Preedit { text, active_range_chars }` renders as injected text at each cursor with the active range
    underlined, and changes no buffer.
  - `Commit(text)` becomes one transaction and one undo group.
  - `DeleteSurrounding { before_chars, after_chars }` becomes a delete transaction.
  - Escape cancels the preedit only (widget-survey pin 6).
  - While focused (`Memory::owns_ime_events` = has focus), the widget writes
    `PlatformOutput::ime = IMEOutput { purpose, rect, cursor_rect, should_interrupt_composition }` with the caret
    rect transformed to screen space, exactly as `TextEdit` does (`widgets/text_edit/builder.rs` ~1050).
  - Reconversion and surrounding text are not exposed by winit 0.30 [unverified]. They are winit fork candidates
    (D7 set).
- **Clipboard:**
  - `Event::Copy/Cut/Paste` and `ctx.copy_text`;
  - multi-cursor paste distributes lines when the counts match;
  - Linux PRIMARY selection follows egui 0.37's support (W5 decides the policy).
- **Mouse:**
  - click, plus double- and triple-click by word and line, with drag extending by unit;
  - Alt+click adds a cursor, and Alt+Shift+drag makes a column selection (ED2);
  - Ctrl+click emits `LinkActivated`.
- **Accessibility** (`code-editor` §4.8, E9):
  - The widget's node is `Role::MultilineTextInput` with the host-supplied name.
  - Its children are `Role::TextRun` nodes for a **page** of lines around the caret: 500 when a screen reader is
    active, else 50. The pattern is egui's own `accesskit_text.rs`, with node ids `Id::new((editor_id, "run",
    line))`.
  - `text_selection` maps the primary selection into the page.
  - When the caret leaves the page, the page shifts and the line number is announced.
  - `SetValue` and `ReplaceSelectedText` actions become transactions.
  - kittest 0.37 fails an unnamed input, so the name is required.

### 3.9 Performance targets (from `code-editor` §9 and `live-ide` §9)

| Id | Target | Where measured |
|---|---|---|
| E1 | 100k-line file: p99 frame ≤ 8.3 ms while scrolling and typing; editor CPU ≤ 2 ms per frame; open to first paint ≤ 300 ms | `benches/` headless with kittest plus a wgpu run on a GPU machine |
| E2 | Rope: memory ≤ 1.5× the file size; edit p99 ≤ 50 µs at 100 MB; line fetch ≤ 1 µs | criterion in `quire-core` |
| E3 | Large-file thresholds set from measurements; 60 fps degraded, never frozen | benches |
| E5 | Re-highlight after an edit ≤ 2 ms off-thread on a 20k-line Svelte file; injections correct | `quire-syntax` benches and fixtures |
| E6 | Keypress to presented frame ≤ 1 frame plus the compositor | cadatomic hardware run (outside crate CI) |
| AP6 | In-process svelters diagnostics per keystroke ≤ 20 ms | host side (svelters CX1/SV27); the editor's part is `snapshot()` in O(1) |
| — | Idle: 0 frames after the blink stops; no allocation per frame for clean lines | kittest frame counter |

### 3.10 Testing

| Layer | What |
|---|---|
| Core laws | proptest: `compose` is associative; `apply(invert(c))` restores the text; `map_pos` is monotone; anchors survive random edit streams; undo then redo is the identity; remote changes interleaved with local edits keep selections valid |
| Monaco fixtures (E11) | `cursor.test.ts` (263 tests), typing, word and find-replace tests translated to data (text, cursors, keystrokes, expected text and cursors) and run against `EditorState::command`/`apply` with no egui. P0 gate ≥ 90 %; each failure marked "ours" or a bug. MIT notice kept |
| Native pins | widget-survey pins 1–6 run through cadatomic's NA2 harness; the crate exposes the observables (text, selection with direction, focus, preedit) through `EditorState` getters |
| Widget | `egui_kittest`: typing, clipboard events, IME event injection (`Harness::event(Event::Ime(...))`), multi-click, a11y tree page shifts, find bar; snapshot images per theme |
| Syntax | grammar fixtures with expected token runs; injection fixtures for Svelte |
| Perf | criterion benches E1 (headless part), E2, E5; CI fails on > 20 % regression against the stored baseline |

### 3.11 Milestones

| Id | Content | Size (lane-days) | Maps to |
|---|---|---|---|
| **QE0** | Repo and CI (as BD0); `quire-core` text, `ChangeSet`, `Transaction`, selection, history (local and host modes), versions, snapshots, recent-changes mapping; proptests; E2 | 4–5 | ED1 |
| **QE1** | `egui_quire`: virtualised lines, galley cache, one cursor and selection, VS Code keymap (`Builtin`) and `Host` mode, clipboard, find (`regex-automata`), gutter, diagnostics (squiggles, gutter, hover text), save and dirty, a11y node with a first page, kittest suite | 5–7 | ED1 |
| **QE2** | `quire-syntax`: tree-sitter Svelte, TS, JS and CSS with injections, token store, `SyntaxDriver` on `Spawner`, E5 | 3–4 | ED1 |
| **QE3** | cadatomic wiring (in cadatomic): a Rust dock pane hosting the editor on files under `ui/`, svelters `compile()` diagnostics (CX1 later), `UndoMode::Host` with `text.edit` — the TRACKS ED1 demo editor | 2–3 | ED1, LI1 prerequisite |
| **QE4** | Rest of P0: multi-cursor and column selection, folding, IME complete (E7), AccessKit paging (E9), Monaco fixtures ≥ 90 % (E11), large-file policy (E3), soft wrap with measured heights | 10–14 | ED2 |
| **QE5** | `<code-editor>` kind in cadatomic's framework crate over SV1 (§4.2) | 1–2 | ED2 (needs RA1) |

The total is 25–35 lane-days. QE0–QE2 need nothing from svelters. QE5 needs RUSTAPI RA1. P1 (completion, hover,
inlay hints as injected text, semantic tokens) and P2 (diff and merge) are later milestones on the same core.

### 3.12 Repository layout

```text
quire/                          (own repo; cadatomic sessions own it)
  crates/quire-core/            text, changes, selection, history, commands, find, folds, tokens, diagnostics, height index
                                (ropey, smallvec, regex-automata, imara-diff; serde/schemars opt)   no egui, wasm32-clean
  crates/quire-syntax/          Highlighter impls on tree-sitter 0.27; grammars and queries behind features   (C build)
  crates/egui_quire/            widget, galley cache, input, IME, clipboard, a11y, find bar                   (egui)
  fixtures/monaco/              translated test fixtures (MIT notice)
  queries/<lang>/*.scm          highlight/injection/fold queries (licence per file, see §3.13)
  benches/  examples/playground (eframe dev-dep)
  LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES.md
```

### 3.13 Licensing of borrowed designs and code

- **VS Code and Monaco (MIT):**
  - literal translation is allowed (CE2), keeping Microsoft's copyright and the MIT text in the translated files
    and in `THIRD_PARTY_NOTICES.md` (`code-editor` §1.2);
  - the fixtures carry the same notice.
- **CodeMirror 6 (MIT):** design, with a notice if any code is translated.
- **Zed's `text`, `rope`, `editor` and `multi_buffer` (GPL-3.0-or-later):**
  - designs only, nothing copied;
  - `zed-sum-tree` (Apache-2.0) may be used as a dependency.
- **Helix and `tree-house` (MPL-2.0):**
  - depending on `tree-house` is fine;
  - copied query files stay MPL-2.0 as separate files with their header, since MPL is file-level copyleft;
  - nvim-treesitter queries (Apache-2.0) are the alternative source;
  - check each file.
- **Grammars:** `tree-sitter-svelte-ng`, `-typescript` and `-css` are MIT.
- **ropey and crop** are MIT. **xi and Lapce** are Apache-2.0.

## 4. The seams (keep exactly consistent with the svelters-side embed architecture)

### 4.1 Dock → host: the pane callback and lifecycle events

```rust
/// Emitted by DockUi::logic, after intents are applied and windows solved, before the pass.
#[derive(Clone, Debug, PartialEq)]
pub enum PaneEvent {
    /// The pane will be drawn in this frame's pass (its pane_ui will be called), and was not drawn last frame.
    Shown { pane: PaneId, window: WindowId, viewport: egui::ViewportId },
    /// The pane will not be drawn this frame but was drawn last frame. Not a close: the pane is still in the layout.
    Hidden { pane: PaneId, reason: HiddenReason, since_frame: u64 },
    /// The pane's window changed (any state, visible or hidden). `viewport` is the egui viewport whose pass
    /// will call pane_ui (ROOT when the window is embedded). Emitted before any Shown for the same pane.
    MovedToViewport { pane: PaneId, from: WindowId, to: WindowId, from_viewport: egui::ViewportId, to_viewport: egui::ViewportId },
    /// The pane left the layout. Final for this lifecycle; a later Open of the same PaneId starts a new one.
    Closed { pane: PaneId, reason: CloseReason },
}
pub enum HiddenReason { InactiveTab, RegionCollapsed, OverlayClosed, CollapsedBySolver, WindowMinimized }
pub enum CloseReason { User, WindowClosed, LayoutReplaced, Host }
```

**Per-frame order.** `DockUi::logic` runs once per frame, not per pass:

1. Apply the queued intents.
2. Solve each window.
3. Compute this frame's visible set: each pane mapped to its effective viewport.
4. Diff the visible set against the last frame's and emit, in this order:
   - all `Closed`;
   - then all `MovedToViewport`;
   - then all `Hidden`;
   - then all `Shown`.

   Within each class, events follow layout order: windows in creation order, then depth-first through regions,
   area and floats.

Guarantees:

- **One lifecycle per pane id:**
  - `Absent → Shown → (Hidden ⇄ Shown)* → Closed`;
  - `MovedToViewport` can occur in any non-absent state.
  - Every `pane_ui` call falls between a `Shown` and the next `Hidden` or `Closed`.
  - There is never a `pane_ui` call for a pane whose last event was `Hidden` or `Closed`.
  - A pane opened and closed within one frame produces no events.
- **One place per pass:**
  - Each visible pane's `pane_ui` is called **at most once per pass and in exactly one viewport**: the one named
    by its last `Shown` or `MovedToViewport`.
  - The drag ghost is a chip and never calls `pane_ui` (`docking` §5.4).
  - Under egui multipass (`request_discard`), `pane_ui` may run again in the next pass of the same frame, but no
    new events are emitted. svelters' walk is already pass-safe (`EGUI.md` §1.2: "the walk must not have side
    effects beyond the outbox").
- **Moves within a viewport** (another group, region or float in the same window) emit **no** lifecycle event. The
  pane's body `Ui` simply has a new rect, and the next `pane_ui` call happens there. Keeping slot identity per pane
  is what makes this free.
- **Stable ids:**
  - `PaneId` is the host's persisted string. It is unchanged by every dock command, by window moves, and by
    `replace_layout` (profile switches, workspace application, reload from file).
  - `WindowId` is persisted.
  - The `egui::ViewportId` is `ViewportId::from_hash_of(("berth", window_id))`, so it is stable across reloads
    and restarts.
  - `replace_layout(new)` diffs old and new by `PaneId`:
    - panes in both keep their lifecycle (move or show events only);
    - panes only in the old layout get `Closed { reason: LayoutReplaced }`.
- **Minimised or closed windows:**
  - A window whose `ViewportInfo::minimized == Some(true)` (read in `logic` from `ctx.input(|i| i.raw.viewports)`)
    makes all its visible panes `Hidden { reason: WindowMinimized }`. Restoring the window makes them `Shown`.
  - Occlusion (`ViewportInfo::occluded`) does not hide panes in v1, because it is unreliable across platforms.
  - There is no state in which a pane belongs to a window that does not exist. Closing a window either closes its
    panes (D2: `Closed { reason: WindowClosed }`, after the host's guards) or, under another host policy, moves
    them (`MovedToViewport`).
  - A newly created window's panes get `Shown` in the frame whose pass first shows that viewport.
- **Embedded windows** (kittest, or platforms without multi-viewport): the effective viewport is `ROOT`. A move
  between two embedded windows emits `MovedToViewport` with `from_viewport == to_viewport == ROOT`, so the host
  still learns about the window change.

**Required host order in `App::logic`:** `dock_ui.logic(ctx, …)` → forward the events → svelters `embed.logic(ctx)`
→ the rest. Every root is then in its right slot, viewport and visibility before the pass. One frame of latency
from a tab click to its effect is accepted (`docking` §5.2: "Applied in `logic`").

**The cadatomic adapter maps the events one-to-one onto svelters-egui embed calls** (TRACKS EMBED EM1/EM2; SV17,
SV18, SV19). The method names below are this document's placeholders; the svelters embed architecture fixes them.

| Dock | Adapter action (svelters-egui embed) | Notes |
|---|---|---|
| first `Shown` of a pane with a Svelte root | `create_slot(pane_key)`; `mount(entry, export, slot, props) -> RootId` (SV17), or `set_root_visible(root, true)` if already mounted | Lazy mount: a restored layout mounts only what is shown (D5-friendly). The slot is keyed by `PaneId`, one slot per pane, for the pane's whole life |
| later `Shown` | `set_root_visible(root, true)` (SV19) | Frames and rAF resume |
| `Hidden { reason, since_frame }` | `set_root_visible(root, false)` (SV19) | Under memory pressure or after idle, the adapter may evict via EM3 snapshot and unmount, and remount on the next `Shown` (D5) |
| `MovedToViewport { to_viewport, … }` | `move_root(root, to_viewport)` (SV18: reparent across viewports, no remount; focus restore; scale-change cache invalidation) | The slot stays the same. Only the viewport the slot is drawn in changes |
| `Closed` | `unmount(root)`; `drop_slot(slot)` | After the host's own close guards |
| `PaneHost::pane_ui(ui, pane)` | `embed.show_slot(ui, slot)` (SVP1) | The `ui` max rect is the pane body. The root's taffy root gets a definite size from it (`EGUI.md` §1.3 guard) |

**Deferred viewports:** the adapter's `pane_ui` must reach the embed handle. svelters' handle is an `Rc` on the UI
thread (SVP3 owned embed handle), so the host keeps it in the same `DeferredHost` the dock's thread-local holds
(§2.8). Both thread-locals assert the main thread.

### 4.2 Editor → host, and `<code-editor>`

**Edits out:**

- Every applied transaction yields `Applied { version, changes, selection_changed, closed_group }`.
- `EditorState::snapshot()` is O(1) and `Send`.

**The live module loader** (`live-ide` §6.4; svelters LOADER Q1 "source overlay: in-memory file contents by path,
read before disk"):

- On `Applied` with a new version, the host debounces (150 ms) and calls the loader's overlay with
  `(path, snapshot.text, snapshot.version)`.
- The compile's diagnostics come back tagged with that version, through
  `state.set_diagnostics("svelters", version, diags)`. Diagnostics are mapped forward if the buffer moved on, and
  refused as `Stale` if they fall outside the window.
- **Save:** `EditorEvent::SaveRequested`. The host writes the file or the layer revision (`live-ide` §6.3), calls
  `state.mark_saved(version)`, and triggers HMR. Styles and tokens may apply on type (`live-ide` §6.4).
- **External change** (an agent's `ide.sources.write`, `live-ide` §6.8; a file watcher):
  - the host calls `apply_remote(base, ChangeSet::diff(old, new), Origin::Reload)`, which keeps carets;
  - when both sides changed, the host's three-way merge decides first.

**Diagnostics in:**

```rust
pub struct Diagnostic {
    pub range: Range<usize>,            // bytes, at the version passed to set_diagnostics
    pub severity: Severity,             // Error | Warning | Info | Hint
    pub message: String,                // host-localised or the server's text, labelled by source
    pub code: Option<String>,           // e.g. "unknown_element", "cad.slot_unknown"
    pub id: Option<String>,             // host's stable id (code-editor §4.10) for Problems ↔ editor sync
    pub tags: DiagTags,                 // unnecessary, deprecated
    pub related: Vec<(Range<usize>, String)>,
    pub actions: Vec<DiagAction>,       // { title, command_id: String, args: serde_json::Value }, e.g. "Grant…" (live-ide §5.2)
}
pub struct SourceId(Arc<str>);          // "svelters", "tsgo", "oxc", "runtime" (live-ide §6.4: merged, tagged by source)
```

Sources replace their own set atomically. The editor never interprets `actions`. Choosing one emits
`EditorEvent::DiagnosticAction { command_id, args }` (§3.7), and the host runs it through its registry.

**What the state and widget expose for a svelters `<code-editor>` kind** (SV1 `#[svelters::widget]` struct form,
RUSTAPI RA1; `EGUI.md` §3.3):

| Kind facet | Editor side |
|---|---|
| Node-attached Rust state | `EditorState`: `Send + 'static`, no egui types. It lives in the provider block on the node (RA1 "typed per-provider data on the node") or behind a host handle (`doc` prop). It survives prop updates and moves between slots and viewports |
| Props | `language: string`; `readonly: bool`; `options: EditorConfig` (serde: tab size, insert spaces, line numbers, word wrap, font size token, undo mode); `aria-label`; **`value: string`** (scratch mode, small buffers only); **`doc: DocRef`** (a host handle to a shared `EditorState` for files, so typing never round-trips text through JS, the same precedent as `models` §6.3 `SearchField` native binding); `selection` (bindable) |
| Exact observation (SV10) | Every value the widget changes is reported by `Applied`/`EditorOutput` the frame it changes: `version`, `selection_version`, `is_dirty`, focus. The kind wrapper calls `cx.set` for `value` and `selection` and `cx.emit` for events, so `bind:` is `exact` |
| Events | `onchange { version, changes }` (coalesced per checkpoint), `onselectionchange`, `onsave`, `onfocus` and `onblur`, `ondiagnosticaction`, `oncontextmenu` |
| Methods | `focus()`, `revealLine(n)`, `setSelection(sel)`, `executeCommand(EditorCommand)`, `setDiagnostics(source, version, diags)` |
| Measure | `CodeEditor::measure(ctx, available)`: intrinsic size from line count and line height (capped by an `options.maxLines`), or fill. No `request_discard` in steady state ("measured equals rendered", `framework` §2.2) |
| Paint and interact | `CodeEditor::show_in(ui, rect)`: the scene passes the taffy rect. All mutation leaves as `transactions`, which the wrapper applies after the pass (svelters' rule: JS never runs during the pass, `EGUI.md` §1.4) |
| Identity | The `id_salt` is the node's salted egui id (SV16 anchor or identity), so caret, scroll and IME ownership survive reparenting (SV18) and, with ID1, hot reload |
| A11y | Its own `MultilineTextInput` subtree under the node's id. The name comes from `aria-label` or the label prop |
| Settle | `EditorState::is_settled()` feeds the node's contribution to svelters' `settled()` (SVP4) |

**Where the kind wrapper lives** is O9. The recommendation is cadatomic's framework crate first, since it is the
consumer (`01-overview` §3.12 "traits and crates only with a consumer").

## 5. Ownership and process

- **Ownership:**
  - cadatomic's sessions own both repositories: code, CI, releases and the egui upgrade lane.
  - svelters sessions do not edit them. svelters' needs (for example a hook the `<code-editor>` wrapper is
    missing) go to cadatomic's request table, as the reverse of the SV process.
  - API changes that touch §4 are announced to both coordinators (`svelters-agent`, `main-cadatomic`) before they
    merge.
- **Consumption by cadatomic:**
  - git dependencies pinned at a tag or sha, never floating (`01-overview` §3.16);
  - a `[patch]` path to a sibling checkout during development;
  - a `pins.toml`-style entry so `check.sh` sees the shas.
  - These are first-party crates, not forks, so `forks.toml` does not apply.
- **Builds:** the repos are small, but their CI and local gates still go through the machine's shared build slots
  (`tools/dev/cargo-slot` or cadatomic's `scripts/cargo.sh`), as svelters `CLAUDE.md` requires for every cargo
  command.
- **CI:** five jobs:
  - check and clippy with `-D warnings`;
  - nextest with kittest;
  - a wasm32 build of the cores;
  - MSRV 1.95;
  - fuzz and proptest smoke.

  Linux Xvfb runs the eframe viewport smoke test (dock). Windows and macOS jobs build and test, without gating on
  OS-input experiments (`01-overview` §6).
- **Retiring egui_tiles:**
  - BD4 deletes egui_tiles, its fork (`~/projects/egui_tiles-fork`) and its `forks.toml` row.
  - The egui_mcp fork is unaffected.
  - The svelters `CLAUDE.md` note on egui_tiles and egui_mcp versions is already flagged stale (`TRACKS` §6
    item 7).

## 6. Conflicts between the research and the decisions (the decisions win)

| # | Research says | Decision | Effect here |
|---|---|---|---|
| C1 | `docking` §5.3, `ide-shell` §5: the dock is drawn by the component framework in the retained scene | Dock crate is egui-only, own repo | Plain egui renderer over the core's `DockFrame` (§2.8) |
| C2 | `docking` §5.1, `native-ext` §6.2, `live-ide` §6.2: dock model types "in `cadatomic-core`'s view layer or the UI crate" | Own repo | The model is in `berth-core`; cadatomic wraps it |
| C3 | `code-editor` §4 and §4.14: `TextBuffer`, `ChangeSet`, anchors and `SyntaxProvider` in `cadatomic-core`; `CodeEditor` in the framework | Editor crate is egui-only, own repo | In `quire-core`, `quire-syntax` and `egui_quire`; cadatomic-core may depend on `quire-core` (it is headless and wasm32-clean) |
| C4 | `code-editor` §4.6, `models` §5.5/§7: editor and virtualisation as scene nodes | Same | Immediate widget, wrappable as a kind (§4.2) |
| C5 | `live-ide` §6.5, §7, J9: the editor component in the framework, with LSP client and Problems | Same | Widget external; LSP and Problems stay in cadatomic |
| C6 | `docking` §5.1: secondary windows editor-only | D1: full shells | `RegionsMode` per window |
| C7 | `docking` §5.6: closing a window moves its tabs | D2: close them | `CloseReason::WindowClosed` |
| C8 | `command-arch` §3.6: layout not undoable | D8 | Layout `UndoStack` |
| C9 | `command-arch` §1 item 8: text undo recorded nowhere | CE3 | `UndoMode::Host` with `UndoGroupClosed` |
| C10 | `docking` §7.5 and `live-ide` J6: no IDE or HMR in release vs release editing | svelters `CLAUDE.md`: the module loader works in release builds too | The editor has no dev-only paths; nothing in either crate is gated on debug builds except invariant checks |
| C11 | `widget-survey` row 242: the code editor builds on the TextField core | `code-editor` §2.3 and §7 (own core); the user's editor decision | Inverted (§3.3) |
| C12 | `TRACKS` DOCK/EDITOR "Files": `dock/*`, `cadatomic-core` text model, `Cargo.toml (ropey, tree-sitter)` in cadatomic | Own repos | TRACKS' DOCK and EDITOR rows need a note that the crates are external; the coordinator updates TRACKS |

## 7. Open decisions, each with a recommendation

| # | Decision | Options | Recommendation |
|---|---|---|---|
| O1 | Names | working names `berth` / `quire`; something plainer (`egui_docking`, `egui_code`) | Any name free on crates.io as `<name>-core` plus `egui_<name>` (checked 2026-10-22: `berth-core`, `egui_berth`, `quire-core` and `egui_quire` are free; `berth` and `quire` themselves are taken). Decide before BD0 and QE0 |
| O2 | Licence of the two repos | MIT OR Apache-2.0; MIT; proprietary | **MIT OR Apache-2.0**, the egui ecosystem's norm. It allows upstreaming and borrowing from egui_tiles. cadatomic's own licence was never checked (`slint` SQ6), so the user should confirm |
| O3 | Publish on crates.io or git only | publish; git tags only | **Git tags only until the API settles** (after BD5 and QE4), then publish. Publishing forces semver discipline the joint egui lane already provides |
| O4 | Who applies intents | always the host; `auto_apply` option | **Both**: the host routes through its registry (cadatomic); `auto_apply` for tests and other apps |
| O5 | Deferred viewports in v1 | immediate only; deferred from BD5 | **Immediate in BD3, deferred in BD5** behind W2. The thread-local design must match svelters EM2's |
| O6 | Pane lifecycle granularity | four events (spec); add `Opened` and `Moved` (same viewport) | **Four events**, as specified. A same-viewport move needs no slot work, and `Opened` without `Shown` would mount unseen panes |
| O7 | Top region | left, right and bottom only (`ide-shell` §2); add top | **Left, right and bottom.** Add top only when a host asks |
| O8 | Highlighter engine | `tree-house` 0.4 (MPL, API "may still see breaking changes"); own runner over `tree-sitter` `QueryCursor` | **Decide by E5.** Lean to our own runner if tree-house's API churns: it is a few hundred lines, and our injection needs are three languages |
| O9 | Where the `<code-editor>` kind wrapper lives | cadatomic framework crate; a svelters pack crate (`svelters-egui-pack`); a third crate in the quire repo depending on svelters | **cadatomic framework crate first** (consumer-driven). Move it to a svelters pack only if a second app wants it; it must never live in the quire repo, which stays svelters-free |
| O10 | Undo ownership for documents | the editor's history; the host's head moves | **The host's for documents** (`UndoMode::Host`, CE3); the editor's for scratch buffers |
| O11 | Rope crate | ropey 1.6; ropey 2.0 beta; crop; lapce-xi-rope | **ropey 1.6 behind `Text`**, with E2 deciding. The rope type never appears in the public API |
| O12 | Built-in keymap | none (host only); VS Code default | **VS Code default as data** (CE4), switchable to `KeyHandling::Host` |
| O13 | Cross-window drag without the winit fork | Windows, macOS and X11 via `EguiDockHost`; wait for the fork everywhere | **Ship Windows, macOS and X11 in BD6 without the fork**; Wayland uses fallbacks until the D7 fork lands |
| O14 | Layout file owner | crate format, host file; host format | **The crate owns the format and its migrations; the host owns the file, its location (D6) and its history.** cadatomic's v1 (egui_tiles) migration stays in cadatomic |

## 8. Risks

- **The deferred-viewport thread-local** (dock and svelters both). If W2 fails, everything stays immediate: N
  passes per repaint for N windows. That is acceptable for two or three windows.
- **Cross-window pointer facts through egui** (`EguiDockHost`) may be clamped or missing on some platforms
  [unverified]. W3 decides whether cadatomic must supply a native `DockHost` in BD6.
- **egui churn** every 2–3 months (`EGUI.md` §5). The renderers touch only `Painter`, `Ui::interact`, `UiBuilder`,
  viewports, `accesskit_node_builder`, IME output and fonts, which is a small surface. The cores are immune.
- **tree-sitter grammar staleness:** `-svelte-ng` dates from 2024-09, and its Svelte 5 rune coverage is unverified
  (`code-editor` §5). Mitigation: fixtures, plus a fork of the grammar under the fork policy if needed.
- **AccessKit text in egui** is "basic" (`code-editor` §0). E9 may show gaps that need egui patches (a candidate
  for an egui fork set).
- **Two owners of one seam:** a mismatch between §4 and the svelters embed architecture would break roots in
  panes. Mitigation: a shared kittest in cadatomic (two roots in two dock panes, a move across embedded windows,
  minimise, close) as BD5's acceptance test.

## Sources

**cadatomic `92fae3f`:**

- `docs/research/`, all 15 files:
  - `2026-10-16-command-architecture.md`;
  - `2026-10-17-{kapi-handoff-from-cadabra3,kapi-migration,m1-seam-readiness}.md`;
  - `2026-10-22-{code-editor-and-lsp,component-framework,docking-windows-and-svelters-roots,ide-shell-and-ux,js-api-publishing-and-live-ide,models-filtering-and-dynamic-data,native-extensions-abi-and-engines,scene-node-and-svelte-semantics,slint-lessons,widget-survey-and-native-acceptance,workspace-document-versioning}.md`.
- `docs/plan/{00-decisions,01-overview,05-experiments-and-research}.md`.
- `crates/cadatomic-app/src/dock/{mod,seam,layout_file,viewport_tabs}.rs` and `crates/cadatomic-app/src/app.rs`.
- `forks.toml` and `pins.toml`.

**Forks:** `~/projects/egui_tiles-fork/FORK.md` (base `bd46e92`, port to 0.37, no source change) and
`~/projects/egui_mcp-fork/FORK.md`.

**svelters:**

- `plans/research/{EGUI,TRACKS,CADATOMIC}.md`;
- `crates/svelters-egui/src/{lib,backend,runner,harness,scene}.rs`;
- `CLAUDE.md`.

**egui 0.37.0** (local registry):

- `context.rs`:
  - `show_viewport_deferred` at 4608, with its `Fn(&mut Ui, ViewportClass) + Send + Sync + 'static` bound;
  - `show_viewport_immediate` at 4662;
  - `embed_viewports`, true by default (797);
  - `request_repaint_of`, `accesskit_node_builder` at 4203, `transform_layer_shapes` at 3596 and
    `request_discard`.
- `data/input/viewport_info.rs`: `minimized`, `focused`, `occluded`, `inner_rect`, `outer_rect`.
- `data/input/raw_input.rs`: `viewports`.
- `data/input/ime_event.rs`.
- `data/output.rs`: `IMEOutput`.
- `memory/mod.rs`: `owns_ime_events`.
- `text_selection/accesskit_text.rs`.
- `widgets/text_edit/builder.rs`.

**eframe 0.37.0:** `native/winit_integration.rs:59` (`set_embed_viewports(!IS_DESKTOP)`).

**egui_kittest 0.37.0:** `lib.rs` (`Harness`; root-viewport command handling only).

**crates.io API, 2026-10-22** (versions, dates and licences):

| Crate | Version | Date | Licence | Notes |
|---|---|---|---|---|
| egui_dock | 0.21.1 | 2026-08-06 | MIT | |
| egui_tiles | 0.18.0 | 2026-10-19 | MIT OR Apache-2.0 | |
| egui_code_editor | 0.4.2 | 2026-10-07 | MIT | egui ^0.36 |
| ropey | 1.6.1 | | MIT | |
| crop | 0.4.3 | | MIT | |
| tree-house | 0.4.0 | | MPL-2.0 | |
| cosmic-text | 0.19.0 | | MIT OR Apache-2.0 | |
| floem | 0.2.0 | | MIT | |
| lapce-xi-rope | 0.4.0 | | Apache-2.0 | |
| zed-sum-tree | 0.2.0 | | Apache-2.0 | |
| tree-sitter | 0.27.1 | | | |
| tree-sitter-svelte-ng | 1.0.2 | | MIT | uses tree-sitter-language ^0.1 |
| tree-sitter-typescript | 0.23.2 | | MIT | |
| tree-sitter-css | 0.25.0 | | MIT | |
| parley | 0.12.0 | | | |
| imara-diff | 0.2.0 | | | |
| accesskit | 0.25.1 | | MIT OR Apache-2.0 | |
| egui_kittest | 0.37.0 | | MIT OR Apache-2.0 | |

**Design references, not opened in this session:**

- the CodeMirror 6 reference manual (state, transactions, `dispatchTransactions`, history);
- the xi-editor retrospective (Levien 2020);
- the Zed blog on CRDTs and syntax-aware editing;
- the Helix 25.07 release notes.

All of them are cited through `code-editor` §2 and §9, where they were read.
