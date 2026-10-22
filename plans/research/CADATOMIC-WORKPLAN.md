# CADATOMIC-WORKPLAN: cadatomic's UI on svelters-egui, from today to the IDE shell

Status: plan proposal, 2026-10-22, written by a svelters planning agent for main-cadatomic, which adopts or amends
it. Nothing here is implemented, and nothing here is decided for cadatomic: cadatomic's decision register
(`docs/plan/00-decisions.md`) wins wherever this document disagrees with it. Lane ids that cadatomic already uses
keep their meaning; new ids are proposals (§3.1).

Read at:

- **svelters `ceb5b0be`** (origin/master, 2026-10-22): `STATUS.md`; `plans/research/{TRACKS, CADATOMIC, ARCH-CRATES,
  ARCH-EMBED, ARCH-SCENE, DESIGN, E2E-INSPECTOR, ICONS, ZED-LESSONS, QT-SURVEY, ASSETS}.md`; the milestone records
  `Q1`, `H1`, `H3`, `DX1`, `EV3`, `DS1`; `crates/svelters-egui/{Cargo.toml, src/embed/mod.rs}`,
  `crates/svelters-modules/src/source/mod.rs`. In-flight lanes as `STATUS.md` lists them, plus the coordinator's
  list (EV1, CX3, RA3, FM1, SG6, ZM1, WG6, AU2).
- **cadatomic `1e94d9d`** (master, read only): `CLAUDE.md`, `STATUS.md`, `docs/plan/00-decisions.md` (all
  2026-10-22 entries), `docs/plan/04-kernel-requests.md` (SV table), `docs/plan/02-phase-a.md` (order),
  `docs/research/2026-10-22-{ide-shell-and-ux, component-framework, js-api-publishing-and-live-ide,
  models-filtering-and-dynamic-data, docking-windows-and-svelters-roots}.md` (the sections cited), and the code:
  `crates/cadatomic-app/src/{svelte, views, theme, shortcuts, dock, shell, palette, problems, tree, task_panel,
  forms}`, `crates/cadatomic-core/src/{settings, keymap}.rs`, `ui/`, `scripts/build-ui.sh`, `Cargo.toml`.
- cadatomic's svelters pin is **`a52d437c`** (the LISTS merge). Everything merged on svelters master after it is
  listed in §1.3; a pin advance is the first commit of the lanes that need it.

## 0. Summary

- **cadatomic uses the SC1 slice of svelters-egui and little else.** It embeds one `Embed` with one slot and one
  root (the task panel), exports one module through E0/E3, registers one RA1 kind (`<quantity-field>`, with
  provider state bits and primary selection) and registers the embed's `BusyProbe` with its `IdleBarrier`. Of the
  roughly twenty capabilities merged since, it uses none yet (§1). Three of the gaps are real workarounds, not
  just unused features:
  1. the Svelte panel is **built by Vite** into `ui/dist/embed.js` (`scripts/build-ui.sh` links `node_modules` into
     the sibling svelters checkout), where the Rust loader (Q1), TS (Q2), CSS (Q3w) and HMR (H1/H3) now load the
     same sources live;
  2. the keymap's `TextFocus` layer is fed by `ctx.egui_wants_keyboard_input()` and strips matched keys in a
     begin-pass hook, where EV3's `shortcut_override` and `focus_info` give the real claim, including IME
     composition;
  3. the look is cadatomic's own `theme::Tokens` and Phosphor, where DS1's `Theme` gives one token map to egui
     `Style`, the CSS cascade and typed getters for berth and quire.
- **Target architecture (§2).** berth is the host shell. One first-party `Embed` (one QuickJS runtime) holds every
  first-party root, one per pane, mounted lazily on the first `Shown` and keyed by `PaneId`. A second, protected
  tooling `Embed` holds the inspector. Rust keeps the shell: command strip, menus, stripes, tabs, status bar frame,
  the 3D viewport and the editor surface of the live IDE. Svelte draws panel contents: properties and task panel
  (one root that switches), Problems, Jobs, Output, palette, settings, model tree, status sections and overlays.
  The command registry stays the single source; Svelte reaches it through exports modules, never around it.
- **A new framework crate, `cadatomic-ui`** (decision C1/S6: "its own crate in cadatomic"), takes the Svelte host,
  the kinds, behaviours, providers, exports modules, the theme adapter and the icon registry out of
  `cadatomic-app`. It cuts build times and hot spots before the panel lanes start (§2.2, lane UI0).
- **Lane plan (§3).** Keep the existing queues and fill four gaps:
  - **WSL:** K1 → UI0 → LI1 (DEMO1) → FW2 → CH2 → FW3 → FW4.
  - **ThinkPad:** BD4 → **SH1** (new: theme, keys and focus, settings and UI-state backends, motion, platform) →
    CH1 (with ZM1 zoom) → CH3 → CH4.
  - **MacBook:** quire follow-ups → **MD1 → MD2 → MD3** (moved from the end of WSL's queue, which would have kept
    CH4 waiting about a week) → quire P1.
  - **Slot-ins** when their svelters lane merges: **AT1** (automation mount, after AU1/AU2), **ICR** (icon ids,
    after DS5). The art lane **IC2** is drawing work with no cadatomic build.
- **DEMO1 is reachable now.** Every svelters prerequisite is merged (Q1, H1, H1b, H3, H3c, CX1, Q2, Q3w, CS5e). Its
  critical path is K1 (running) → LI1 (3–5 lane-days). DEMO1b (a `<style>` edit) probably comes free, because a
  component swap carries its style.
- **Deletions (§4):** the Vite build and `node_modules` links, the `ui-compiler` feature, the one-slot
  `SvelteTaskPanel` host, egui_tiles, `egui_wants_keyboard_input` as the text-focus signal, `theme::Tokens` and
  Phosphor, the egui task panel after CH2 reaches parity, and the hand-painted palette, Problems and tree rows as
  their Svelte replacements land.
- **svelters gaps that cadatomic will hit (§5.2):** LS2 (Rust row models), Q4 (release bundle), EM3 (adaptive
  hidden roots), DS5 (icon ids) and ISO1 (isolates) are unscheduled or queued without a slot. LS2 is the first one
  cadatomic will block on (MD4, large Problems and BOM lists).
- **Open questions (§5.3):** 18, each with a recommendation. Five go to the user (release sources J6,
  extension panels before isolates, typed settings and migrations, the deterministic test scheduler,
  keyboard-layout work). The rest are the coordinators' calls.

## 1. Where cadatomic stands against svelters-egui's merged capabilities

### 1.1 Capability table

Verdicts: **uses**; **ignores** (no consumer yet, nothing in its way); **works around** (cadatomic does the same job
another way, and that way should go). "Lane" is where §3 picks it up.

| Capability (svelters milestone, master sha) | What it gives | cadatomic today (`1e94d9d`) | Verdict | Lane |
|---|---|---|---|---|
| **Embed core** (EM1 `ff04be32`): slots, roots, `logic`/`show_slot`, anchors, faults, `settled`/`BusyProbe` | many roots in a host frame | `svelte/panel.rs`: one `Embed`, one slot `panel.task`, one root; JS runs only in `App::logic` (`prepass_plugin: false`); `BusyProbe` registered with `IdleBarrier` | **uses** (one root) | BD4 (many roots), UI0 |
| **Deferred viewports** (EM2 `2b0d144f`): `set_viewport`, `show_viewport_passes`, `harness::dock::SvelteDock` | roots in secondary windows | not used; secondary windows wait for BD4 | **ignores** | BD4 |
| **RA1 kinds** (`12f34937`): `#[svelters::widget]`, typed props/events, `ExtKey`, `state_bits!`, spec fragments | Rust widgets as Svelte elements | `svelte/quantity_field.rs`: `<quantity-field>` with state bits; spec fragment dumped to `ui/.svelters/app` | **uses** | K1 (`<code-editor>`), FW2, FW4 |
| **E0/E3 exports** (stores, `#[svelters::export]`) | Rust data and calls in JS | `svelte/exports.rs`: `svelters:app/task` store and `setField`/`accept`/`cancel`/`rebase` | **uses** | CH2 generalises it |
| **RA2 `Json<T>` + SV48** (`78d33256`, `caba7b67`) | cadatomic-core serde types across the boundary | not used: the task view-model is a hand-built `svelters::Record` | **ignores** | CH2 (command args, refusals) |
| **Scene SG1–SG3** (`8ce6d7d0`, `db7f62c5`, SG3): node API, ext columns, journal, queries, hit lists | Rust reads and marks scene nodes | not used | **ignores** | FW3 (typed picking), AT1 (queries) |
| **SG4 provider passes** (StateProvider, ScenePass, overlays, `position: absolute`) | per-frame Rust passes and overlay roots | not used | **ignores** | FW2 (Op states), FW3, VP1 |
| **EV2b native behaviours** (`747392a3`): `use:` resolved to Rust | `pick`, `shortcut`, `contextMenu`, `anchor` as Rust | not used | **ignores** | FW3 |
| **Widget library INT1** (WG1, WG2a, WG2b, WG4a, WG4b; `becbeb89`) | Tier 0 and Tier 1 kinds | uses `collapsing-header`, `toggle-button`, `separator` (the WG1 swap) | **uses** (three kinds) | every panel lane |
| **CSS engine CS0 + CS5e cascade** (`1b6a754f`, `9edf8e24`), **CS5g** shadows (`ceb5b0be`) | `<style>`, `:state()`, tokens, layout | `ui/` has no `<style>`; layout comes from kind props | **ignores** | FW2, CH2 |
| **Theme DS1** (`f1f5810b`): `Theme`, `set_theme`, `egui_style()`, typed getters, VS Code Modern + HC, density | one look for Rust and Svelte | `theme/mod.rs` builds egui `Visuals` from its own `Tokens` (dark/light), Inter + Phosphor; no `--sv-*` feed | **works around** | SH1 |
| **TR2 motion** (`45c3a331`): `set_motion` | reduced and slow motion | not called (register: "a later framework package") | **ignores** | SH1 |
| **EV3+ focus and keys** (`c41dca7a`): scopes, restore, `focus_info`, `shortcut_override`, `text_command`, default/cancel buttons | Rust keymap and Svelte fields agree on keys; Edit menu reaches Svelte fields | `shortcuts/mod.rs` sets `TEXT_FOCUS` from `ctx.egui_wants_keyboard_input()` and removes matched keys in a begin-pass hook; Enter/Escape are keymap intents (`session.accept`/`cancel`) | **works around** | SH1 |
| **LS1 virtual lists** (`a52d437c`): `virtual-list`, `RowPool` | big lists with no JS per scrolled row | tree, palette and Problems are hand-painted egui rows | **ignores** | CH3, CH4 |
| **WG5a** list keyboard, selection, cell editors; **WG5b** tree-view (both in flight) | keyboard tables and trees, editors in cells | — | (pending) | CH3, CH4, FW4 |
| **Q1 loader** (`6dbee841`), **LD1** hooks (`7e8909bb`), **Q2** TS (`1786957f`), **Q3w** CSS in `compile()` | `.svelte`/`.ts`/CSS from source, no Node | `scripts/build-ui.sh` runs Vite over `ui/` into `dist/embed.js`; `node_modules` are links into the svelters checkout; `svelters-compiler` is an optional dependency behind `ui-compiler` | **works around** | LI1 |
| **H1/H1b HMR, H3/H3c `$state` kept** (H1, `41b81e63`, H3) | edit → swap, state kept | not used (bundle loaded once) | **ignores** | LI1 (DEMO1) |
| **CX1 diagnostics** (`check()`, codes, fix-its) | per-keystroke diagnostics for the editor | not used | **ignores** | LI1 |
| **DX1 settings, UI state, paths, logs** (`16da721d`) | `svelters:settings`, `uiState`, `tracing` | no `Dx` supplied, so `svelters:settings` answers `settings/unavailable` (register, WG1 swap) | **ignores** | SH1 |
| **DX2 clipboard and shell** (`dcfdcc71`) | one clipboard for egui, JS, quire | the panel runs on `Platform::headless()`, so Svelte code has no system clipboard or shell | **works around** (deliberately) | SH1 (§5.3 Q6) |
| **AS1 assets** (in flight, `ci/as1`) | SVG icons with tint, `asset://` | — | (pending) | ICR |
| **AU1 automation** (in flight, `ci/au1`); AU2 queued | `scene.*`/`a11y.*`/`wait.*`, Driver | cadatomic's own A8a socket and egui_mcp pixel tier (`ui.tree`) | (pending) | AT1 |
| **TK1 cases** (`bf82674f`) | one `.svelte` case on every backend | not used | **ignores** | CH2+ (component tests) |
| **SVP7/SVP8** feature hygiene, QuickJS opt-level | — | both in cadatomic's manifests | **uses** | — |
| **ZM1 zoom** (queued, WSL) | host zoom API, scale factor | egui zoom shortcuts already off (`shortcuts::disable_defaults`) | (pending) | CH1 |
| **DS5 icons** (queued after VIS1), **IC1** toolchain (in flight), **IC0** style (in flight) | icon ids, codicons, lint, contact sheets | 33 Phosphor glyph consts (`theme/icons.rs`) | (pending) | ICR, IC2 |

### 1.2 What the pattern says

- **Everything cadatomic uses came in SC1.** That was the plan (TRACKS §3.2). The waves since were built for
  cadatomic's next surfaces, but no cadatomic lane has consumed them, because cadatomic's builders have been on
  berth, quire, K1 and the kernel advance.
- **The three workarounds compound.** The Vite path pins `node`, `pnpm install` in the svelters checkout and a
  link-copying snapshot script (`svelters-snapshot.sh`, which had to be fixed for a leak) into cadatomic's gate.
  The `egui_wants_keyboard_input` signal is wrong while an IME composes and for keys a Svelte widget claims
  (sliders, lists). The private `Tokens` mean every Svelte panel would draw in svelters' default look next to
  cadatomic's Rust chrome.
- **Nothing in cadatomic blocks adoption.** The seams (one `App::logic`, `IdleBarrier`, the view registry) already
  have the shapes ARCH-EMBED assumed. The adoption lanes are small (1–4 lane-days each), and most of the cost is the
  svelters pin advance plus `check.sh`.

### 1.3 Merged on svelters master after cadatomic's pin `a52d437c`

`41b81e63` H1b + H3c; `c41dca7a` EV3+; `f1f5810b` DS1; `1786957f` Q2; `ceb5b0be` CS5g; plus the research merges
(E2E-INSPECTOR, ICONS). LI1 wants H1b/H3c and Q2; SH1 needs EV3 and DS1. **Proposal:** the first of LI1 and SH1 to
start does one svelters pin advance to the master sha of that day (through `scripts/svelters-snapshot.sh`, one
`check.sh`), and the other lane rebases on it. After that, advances happen at the start of a lane that needs a
newly merged svelters milestone, at most one a day (§5.1 R3).

## 2. Target architecture

### 2.1 The picture

```
cadatomic process, UI thread
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│ App::logic (once per frame)                                                                       │
│   1 berth DockUi::logic ──► PaneEvents ──► PaneHost adapter ──► AppEmbed (slots, roots, visibility)│
│   2 keymap: strokes ─ shortcut_override? ─► Modal > TextFocus > Session > Viewport > Global         │
│   3 AppEmbed::logic  (first-party QuickJS runtime: every first-party root; JS runs only here)       │
│   4 exports queues ──► command registry / session machines / jobs   (the one command path)         │
│   5 stores published (task, commands, problems, jobs, output, selection, settings)                  │
│ App::ui (passes, per viewport)                                                                    │
│   Rust chrome: title/menu, command strip, stripes, tab strips (berth), status bar frame            │
│   pane bodies: Rust ViewClass (3D viewport, editor surface, inspection views)                       │
│              or SvelteView ─► embed.show_slot(ui, slot)                                             │
│   PaneHost::viewport_done ─► embed.show_viewport_passes(ui)   (R21)                                 │
│ Tooling Embed (protected bundle): inspector, agent panel later   (E2E-INSPECTOR §1.11)              │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
crates:  cadatomic-core (headless, wasm32)  ◄─  cadatomic-ui (framework: kinds, behaviours, providers,
         exports modules, theme adapter, icon registry, Svelte host)  ◄─  cadatomic-app (shell, berth host,
         viewport, quire panes, RPC/MCP wiring)
sources: ui/panels/<panel>/*.svelte, ui/lib/*.ts, ui/.svelters/app (dumped typings and spec fragments)
```

Rules carried over unchanged: `cadatomic-core` stays headless; one kernel seam; one command path; ids, never
labels; JS never runs inside `App::ui` (ARCH-SCENE R17: the scene refuses writes in the `Ui` phase).

### 2.2 Crates and the Svelte host

- **`cadatomic-ui` (new, lane UI0).** It holds what `cadatomic-app/src/svelte/` and `views/svelte_view.rs` hold
  today, plus K1's `<code-editor>` wrapper (ARCH-CRATES O9 puts it in "cadatomic's framework crate"), and later
  FW2's `Op` bindings, FW3's pick provider, FW4's `PropertyGrid`, the theme adapter (SH1) and the icon registry
  (ICR). It depends on `cadatomic-core`, `svelters`, `svelters-host`, `svelters-egui` (no default features, plus
  `source`), egui, berth's and quire's egui crates. It does not depend on wgpu or the kernel seam.
  - Why now: `cadatomic-app` is one crate of about 18k lines (`src/` only) whose every change rebuilds it and
    relinks the app; the panel lanes
    (FW2, CH2, CH3, FW3, FW4) would all edit it. A separate crate keeps their incremental builds small and their
    merges apart.
- **`AppEmbed`** replaces `SvelteTaskPanel`'s private embed. One `Embed` per process for first-party roots
  (EM1's "one runtime, many roots"), created at start, loading `ui/` through Q1's `SourceProvider` (LI1). It owns:
  - a slot table keyed by `PaneId` (berth) or by a fixed key for non-pane roots (`status.<section>`,
    `overlay.palette`, `overlay.viewport.<id>`);
  - the root table (`RootId` ↔ entry, export, props);
  - the fault policy (a faulted root shows an error state with "Revert" and "Open in editor", js-api §6.6);
  - the reload policy (§2.13).
- **Panel registry.** cadatomic's `ViewRegistry` gains Svelte view classes declared as data:
  `{ class_id: "panel.problems", entry: "ui/panels/problems/Problems.svelte", export: "Problems", title_id,
  stripe_slot, icon }`. BD4 makes `SvelteView` a tab; the registry makes every Svelte panel one line.

### 2.3 berth host and pane roots

ARCH-CRATES §4.1 fixes the event contract; the adapter maps it one to one (EM1/EM2 names):

| berth | AppEmbed | Note |
|---|---|---|
| first `Shown` of a Svelte pane | `create_slot(pane_key)`, `mount(entry, export, slot, props)` | lazy mount; restored layouts mount only what is shown (register, seam item 3) |
| later `Shown` / `Hidden` | `set_visible(slot, true/false)` | rAF and frames pause while hidden (SV19) |
| `MovedToViewport` | `set_viewport(slot, viewport)` | no remount (R6); focus restored through `PaneHost::restore_focus` (R7) |
| `Closed` | `unmount(root)`, `destroy_slot(slot)` | after the host's dirty guard (`request_close` → guard → `DockCommand::Close`) |
| `pane_ui(ui, pane)` | `embed.show_slot(ui, slot)` | the pane body is the root's definite size |
| `viewport_done(ui, window)` | `embed.show_viewport_passes(ui)` | R21; deferred windows reach the embed through EM2's thread-local |

- **Pane state.** A pane's `uiState` (DX1: split positions, column widths, open sections) is stored in berth's
  opaque per-pane host data string (register: "opaque string that the host encodes itself"). `RootKey` = `PaneId`,
  so the same pane restores its UI state across restarts and windows (§5.3 Q14).
- **Hidden roots** stay mounted until svelters EM3 exists (D5 asks for adaptive unmount under pressure). With
  fewer than about 30 panels, that is fine; EM3 adoption is a later lane (EM3a).
- **Temporary panes** (an active tool's pane) carry berth's skip-on-restore flag.

### 2.4 Commands, the properties panel and task panels

- **The registry stays the source of truth.** A new exports module `svelters:app/commands` (in `cadatomic-ui`)
  publishes:
  - a store of command entries (id, title id, icon id, shortcut text, menu path) and their **availability with the
    reason id and catalog text**, updated when context keys change;
  - `invoke(id, partialArgs)` and `start(id)`, which push the same `PartialInvocation` the keymap and palette push;
  - `describe(id)` (help metadata, U4) for tooltips and the palette.
- **One properties root.** The right region's properties pane hosts one root, `Properties.svelte`, which shows the
  selection's `PropertyGrid` and becomes the task panel during a session in the same frame (ide-shell §2 rule 4:
  "the properties panel keeps its frame"). Today's split between the egui properties panel and the Svelte task
  panel goes away in CH2.
- **Task panels for every command (CH2).** The panel is generated from the command's input schema, as `forms/`
  does in Rust today. The view-model stays computed in Rust (register, FW1 answers) and crosses as `Json<T>` over
  cadatomic-core types (RA2 + SV48). Slots bind through CX3's draft bindings:
  `<quantity-field bind:value={slot('extents[0]')|commit} />` lowers to draft events, so typing runs no JS per
  keystroke and commits on Enter, blur or release.
- **Waiting states (FW2).** `Op<T>` (framework §3) is projected onto provider state bits on the nodes that present
  a command, slot or job: `:state(cad.pending)`, `:state(cad.stale)`, `:state(cad.failed)`, with the 150 ms delay
  and 300 ms minimum display done in Rust (a `StateProvider`, SG4). Styles react with no JS (CS5e). A
  `command=` behaviour (EV2b) on `button` takes title, icon, shortcut, enabled state and reason from the registry.

### 2.5 Problems, Jobs, Output and the status bar

- **Problems** (bottom region): a Svelte root over `virtual-list`, grouped by document and source with sticky
  section headers (QW17), keyboard navigation and selection by key (WG5a), each row carrying its fix commands with
  their applicability (ide-shell §3). Rows come from cadatomic's diagnostics through a Rust model: a JSON store
  (LS1's T1 tier) first, the LS2 row-model trait when svelters builds it (MD4).
- **Jobs:** the job registry's states as `Op` rows with progress, cancel (`job.cancel` is a command) and links to
  Problems.
- **Output:** an append-only log per channel (kernel, jobs, extensions, `js` tracing). The first version is a
  `virtual-list` of lines with a bounded ring; a read-only quire view is the later option (QW25).
- **Review** (U3 proposals) is a later tab in the same region.
- **Status bar:** the bar and its fixed layout are Rust; sections that change often or come from extensions (tool
  prompt, solve state, units, selection summary) are small Svelte roots keyed `status.<section>` (TRACKS CH3).

### 2.6 Palette and search

- An overlay root (`overlay.palette`) opened by the keymap's palette command. It is a `focus-scope="dialog"` box
  (EV3), so Tab is contained and focus returns to the opener on close.
- Filtering runs in Rust as MD1's tethered filter job over the registry and U4 metadata; JS gets a page of rows.
  Results show the shortcut, the menu path, the disabled reason and a fix (ide-shell §6). "Show me where" highlights
  the real control by anchor through an SG4 overlay pass.
- Semantic search (U4) plugs in behind the same job later; its placement (in process, worker, hub) is open in
  cadatomic's register.

### 2.7 PropertyGrid

- A provider kind `cad:property-grid` (ARCH-SCENE §10.2: `PREPARE | CUSTOM_LAYOUT | VIRTUAL | FOCUSABLE`), with
  rows realised through `RowPool`, so a 2k-row grid scrolls with no JS per row. Editors come from WG5a's cell edit
  protocol (F2, Enter commits, Escape reverts, Tab to the next cell); `QuantityField` is the first editor kind.
- It shows mixed values, reset to default, a modified marker and grouped sections (QW19), and it presents each row's
  value for typed picking (FW3).
- The settings page and the keymap editor are PropertyGrid instances over schemas (§2.9, §2.10).

### 2.8 The 3D viewport

- **The viewport stays a Rust `ViewClass`** that renders through cadatomic's wgpu callback. It is not a Svelte root.
- **Overlay roots over it** (VP1): HUD, dimension labels and the slot prompt at the cursor (ide-shell §6) are a
  Svelte root in an overlay slot over the viewport's rect. Rust writes projected label positions as `left`/`top` of
  absolutely positioned boxes after a camera change (SG4 `position: absolute`), and the overlay passes the pointer
  through except where a node senses input.
- **Typed picking** (FW3, ARCH-SCENE §10.1): a `cad-viewport` provider kind or the viewport's `hit_test` answers
  `Hit::Part(pick_id)` from the last GPU pick readback (register, ARCH-SCENE review); a `PickProvider` marks
  `:state(cad.pick-target)` on presenting nodes; EV1's event filters (SV6) gate input while a slot acquires.
- **Scale.** The viewport reads `ctx.pixels_per_point()` (register, zoom), so zoom (§2.11) changes UI size, not the
  camera.

### 2.9 Settings and the settings UI

- **Backend (SH1).** cadatomic implements DX1's `SettingsBackend` over its A1b layers (defaults < user < workspace
  < document < environment; team policy later), so `setting(key)` in Svelte returns `{ value, source, locked }` with
  cadatomic's provenance. `UiStateBackend` writes into berth's per-pane host data. `AppId` + `Paths` replace any
  hand-made paths. JS `console` and `logger(category)` reach cadatomic's `tracing` subscriber.
- **Typed settings** (Zed lesson 5: typed groups, a generated schema, per-key error tolerance, dated migrations)
  are a user question when the package is planned (register, Zed lessons). Without them, the settings page reads
  `settings::DEFINITIONS` (kinds `Bool`/`Text` only), which is too thin for a real page (§5.3 Q11).
- **The page (SET2)** is a Svelte panel, not a modal (ide-shell §3: "no modal settings dialogs"): a search field
  over a PropertyGrid generated from the schema, with each row's layer and lock shown. Settings without a field stay
  file-only. The automation switch (E2E-INSPECTOR Q1, "off by default, turned on with a warning") lives here.

### 2.10 Keymap

- **cadatomic's keymap stays the authority** (`keymap.rs`: layers `Modal > TextFocus > Session > Viewport >
  Global`, context expressions, chords, presets).
- **SH1 changes its input:** `TEXT_FOCUS` comes from `embed.focus_info(ctx)` (text input, multiline, composing,
  scope) for Svelte, from quire's focus and `composing()` for the editor, and from egui for any remaining Rust
  `TextEdit`. Before a stroke is consumed by a binding, the keymap asks `embed.shortcut_override(ctx, chord)` (Qt's
  ShortcutOverride); a `true` lets the focused widget have it. While composing, every key is the field's.
- **Edit menu** (Rust): Undo, Redo, Cut, Copy, Paste and Select All go to `embed.text_command(ctx, cmd)` when a
  Svelte field has (or last had) focus, to quire when the editor has focus, else to the document command.
- **Enter and Escape** in Svelte panels use EV3's `default`/`cancel` buttons in the panel's scope; the keymap's
  `session.accept`/`session.cancel` intents remain for the viewport and the egui panel until CH2 removes it.
- **Keymap editor (KM1):** a PropertyGrid-style table of commands, contexts and chords, with conflicts, presets
  (SolidWorks, Altium, KiCad, Blender) and the VS Code base for text (CE4). It needs WG6's `key-sequence-edit`.

### 2.11 Zoom (ZM1 seam)

Decided (coordinator, 2026-10-22, cadatomic register "UI zoom like VS Code"): the svelters API takes a **scale
factor**; cadatomic owns the levels. Concretely:

- Commands `view.zoomIn`, `view.zoomOut`, `view.zoomReset` (Ctrl+=, Ctrl+-, Ctrl+0; Cmd on macOS; Ctrl+Shift+= and
  the numpad keys as aliases), bound in the keymap's `Global` layer.
- Setting `window.zoomLevel` (an integer, user layer). Scale = 1.2^level, clamped to a range CH1 picks (VS Code's
  own clamp is the reference; tests cover 80–150 % per ZM1).
- CH1 calls ZM1's host API with the scale; svelters relayouts on the `pixels_per_point` change and evicts SVG
  textures across levels. egui's own zoom keys are already off (`shortcuts::disable_defaults`; ZM1 makes the same
  the `Embed` default).
- Theme metrics are in points, so berth's tab strip, quire's gutter and the command strip scale with the rest.
  Icons pick their optical size from the box size times the scale (§2.12). The viewport reads
  `ctx.pixels_per_point()`.
- Open, coordinator: zoom per OS window or global (VS Code: per window, with a global default). Recommendation:
  global first, per window when D1's secondary windows carry their own settings.

### 2.12 Icons

- **Ids name icons** (`<domain>.<group>.<name>`: `chrome.close`, `mcad.extrude`, `status.error`; ICONS §5.2). One
  table per icon theme maps an id to a glyph (family, code point) or an SVG with sizes. The fallback chain is
  theme → cadatomic's own set → codicons (interim) → Phosphor (until removed) → the missing-glyph box.
- **One registry for Rust and Svelte.** IC1 generates a `.d.ts` union for Svelte (`<icon name="mcad.extrud">` is a
  type error) and Rust consts for Rust panels. The command registry gives each command its icon id, so the command
  strip, menus, palette and `command=` buttons agree.
- **Optical sizes** (user, 2026-10-22): 12, 14, 16, 20, 24 (maybe 32+), each drawn with its own stroke weight.
  The registry entry lists the sizes it has; the renderer picks the nearest size at or below the box size × zoom
  and never scales a 24 master down to 12. The status bar and tree use 12 and 14.
- **Our own style everywhere** (I1 amended): codicons are interim placeholders in the chrome too. IC0 defines the
  style (running as a svelters agent); IC2 (MCAD, about 70), IC3 (ECAD), IC4 (simulation) and chrome replacements
  follow as batches the user approves at contact-sheet level (I2).
- **Delivery:** SVG through AS1 (resvg, tinted from `color`, rasterised at the exact pixel size). Rust panels before
  AS1 use egui_extras' SVG loader with IC1's tint-ready (`#fff`) copies (ICONS §5.3).

### 2.13 The live IDE loop (DEMO1, then DEMO2)

- **Loading.** `AppEmbed` loads `ui/` from source: `SourceConfig { dev, hmr }`, the spec fragments from
  `ui/package.json` (`svelters.specFragments`, already declared). No Vite, no Node at run time.
- **Editor surface.** The IDE's editing surface is Rust (js-api §6.6: "a Svelte edit cannot take away the tools
  needed to undo it"). DEMO1's editor is therefore a Rust `EditorView` pane over egui_quire, sharing `EditorState`
  with K1's `<code-editor>` kind, which serves Svelte panels (consoles, spec editors). (§5.3 Q2)
- **Loop.** quire `Applied` → debounce 150 ms → loader source overlay `(path, text, version)` → CX1 `check()` →
  `set_diagnostics("svelters", version, diags)` and Problems. Save → write the file → `Host::apply_changes([path])`
  (H1) → `ReloadReport`. In embed mode a change that cannot be swapped in place is not turned into a full reload
  (H1 record): AppEmbed remounts only the roots of the affected entry with their last props, and shows the error
  state on failure.
- **DEMO2** adds release builds (Q4 bundle, LI2) and identity-based focus and caret (ID1).

### 2.14 Extension panels

- First-party Svelte runs in process (D4); third-party Svelte runs in isolates (decision 2026-10-22, ISOLATES
  track). svelters' isolate work (ISO1–ISO3) is not scheduled, so **no third-party panel can run before it** without
  contradicting D4.
- What can be built now, and is cheap: the **manifest and the slot**. An extension declares panels with an id, an
  icon id and a requested stripe region; cadatomic assigns a stripe slot once (ide-shell §4) and records it in the
  layout. The loader already confines an extension's imports to its package (`SourceConfig::for_extension`) and
  LD1's visibility filter defaults extension hosts to `Allow([])`. Principal ids go where ARCH-SECURITY §5 says they
  are cheap now (user, "leave room").
- Whether *dev* extensions (written by the user, from a local folder) may run in process before ISO1 is a user
  question (§5.3 Q9).

### 2.15 Automation, tests and the inspector

- **AT1** mounts svelters-automation's method families (`scene.*`, `svelte.*`, `style.*`, `a11y.*`, `input.*`,
  `wait.*`, `clock.*`, `journal.*`, `capture.*`, `reload.*`) into cadatomic's A8a socket server and MCP sidecar.
  cadatomic keeps its domain methods and its egui_mcp `ui.*` pixel tier for Rust-drawn egui (register, automation
  decision). `wait.*` answers through cadatomic's `IdleBarrier`, which includes the embed's `BusyProbe` (agreed).
- cadatomic's `KittestDrive` wraps svelters' in-process `Driver`; `RpcDrive` sends svelters methods unchanged
  (E2E-INSPECTOR §1.10). ndjson scenarios stay the domain recording.
- `dev.ui_inspector` is bound to Ctrl+Shift+I (Cmd+Option+I); F12 stays on Developer tools. The inspector runs in
  the protected tooling `Embed` (AU4, later).

## 3. Lane plan

### 3.1 Conventions

- **Sizes** are lane-days of one agent with cadatomic's gates (`scripts/check.sh` once at the end), in the style of
  TRACKS §2. They are planning estimates by a svelters agent; the cadatomic lanes re-size.
- **Machines:** one cadatomic builder per machine checkout (register, "quire and berth in svelters"). berth and
  quire work counts against the same machine. svelters builds share the same slots on the WSL PC and the ThinkPad.
- **Ids.** Kept: K1, BD4, LI1–LI3, FW2–FW4, CH1–CH4, MD1–MD4, WS1, IC2–IC4, quire P1. New proposals: **UI0**
  (framework crate and AppEmbed), **SH1** (shell adoption), **AT1** (automation mount), **ICR** (icon registry
  switch-over), **SET2** (settings page), **KM1** (keymap editor), **VP1** (viewport overlay root), **EX1**
  (extension panel slots), **EM3a** (adaptive hidden roots adoption), **APP1** (app-level gaps). FW0 as defined in
  TRACKS ("tokens and themes on `set_tokens`") is absorbed by SH1, which uses DS1 instead.
- **Every lane's acceptance** includes: `check.sh` green once at the end on a frozen svelters snapshot; idle draws
  0 frames; `wait_idle` settles; new UI is reachable by role and accessible name (kittest) and by anchor; the
  svelters pin sha appears in scenario and diagnostic-bundle headers.

### 3.2 svelters lanes cadatomic depends on

| svelters lane | State (`STATUS.md`, coordinator) | What it unblocks in cadatomic | Hard or soft |
|---|---|---|---|
| RA3 (methods, container slots, `cached_shapes`, SV23) | in flight, macbook-svelters, methods first | K1's `focus()`, `revealLine`, `setDiagnostics` methods; LI3 (SV23 dynamic modules) | K1 soft (props/events first), LI3 hard |
| CX3 (bind-target lowering, `\|commit`, draft bindings) | in flight, WSL | CH2's draft bindings | CH2 hard for full acceptance; soft start on `onchange` events |
| EV1 (capture/bubbling, SV6 filters, SV7 overlay trait) | in flight, WSL | FW3 typed picking, VP1 input gating | FW3 hard |
| WG5a (list keyboard, selection, cell editors), WG5b (tree-view) | in flight | CH3 (keyboard Problems), CH4 (model tree), FW4 (cell editors) | CH3 soft, FW4 hard (WG5a) |
| FM1 (drag-value commit, submit/pattern/read-only, header state, button order, Menu key) | queued, ThinkPad | CH2 (commit-only numeric entry), FW4 | soft |
| WG6 (disabled tooltips, snippet tooltips, elision, SVG, cursor, key-sequence-edit, combo type-ahead, file drops) | in flight, ThinkPad | disable-with-reason in Svelte panels (P0), KM1, drop a file on a panel | KM1 hard; others soft |
| SG6 (scene memory, paint cache, culling) | queued, ThinkPad, after FM1 | frame budget with many roots and big panels | soft (measure in CH3/FW4) |
| ZM1 (host zoom API) | queued, WSL | CH1's zoom commands | CH1 zoom part hard; CH1 starts without it |
| AU1 (protocol core); AU2 (IPC, headless, Driver::Ipc); AU3 (reload-aware) | AU1 in flight (ThinkPad), AU2–AU3 queued | AT1; agents in the DEMO1 loop (AU3) | AT1 hard on AU1; RPC half on AU2 |
| AS1 (assets, SVG loader with tint) | in flight, ThinkPad | ICR (SVG icons in Svelte); IC2's display | ICR hard for SVG |
| DS5 (codicons, icon ids, per-theme tables, `.d.ts`); IC1 (toolchain) | DS5 queued after VIS1; IC1 in flight | ICR; IC2 lint and sheets | hard |
| DS2/DS3 (outline, cursor, per-side borders, transitions, disabled reasons; affordance sheet, focus helpers) | queued after VIS1 | consistent focus rings and tab markers in Rust and Svelte | soft |
| IN1 (Intl via ICU4X, locale numbers) | in flight, MacBook | `units` display in Svelte fields with decimal comma | soft |
| LS2 (Rust row-model trait, columns, tree rows, key multi-selection) | in the unordered backlog | MD4; Problems/BOM beyond ~10k rows without JS arrays | MD4 hard |
| Q4 (release bundle, compiler linked in production) | not scheduled (ASSETS extends it) | LI2, release builds of DEMO2 | hard |
| EM3 (adaptive hidden roots, SV35) | backlog | EM3a (D5) | hard |
| ID1 (compiler identity, native UI state by identity) | backlog | DEMO2 focus and caret kept; stable `component=` selectors | soft |
| ISO1–ISO3 (isolates) | not scheduled | third-party extension panels | hard |
| CX2 (provider checks, typed slots), CX4 (E5 types, SV26, SV45) | backlog | slot-path and command-id checks in the editor (SV8); typed model typings | soft |

### 3.3 Waves

```
            week 1            week 2            week 3            week 4            later
WSL         K1 ─► UI0 ─► LI1 ═══► DEMO1 ─► FW2 ───────► CH2 ──────────► FW3 ─► FW4 ─► SET2
ThinkPad    BD4 ──────► SH1 ────► CH1 (+ZM1) ───► CH3 ──────────► CH4 ─► KM1
MacBook     quire follow-ups ─► MD1 ─► MD2 ─► MD3 ─────────► quire P1 ─► (MD4 after LS2)
slot-ins    AT1 (after AU1/AU2) · ICR (after DS5 + AS1) · IC2 art lane (after IC0 approval; no cadatomic build)
later       LI2 (Q4) · LI3 (RA3, SV23) · VP1 (FW3) · WS1 · EX1 · EM3a (EM3) · APP1 · ED3 (CX4)
```

The weeks are indicative: about five lane-days each. The order inside a machine is the proposal; the dependencies
in §3.4 are hard.

**Why MD1–MD3 move to the MacBook.** In the existing queue, MD1 starts on the WSL PC after K1, LI1, FW2 and CH2,
about 16–22 lane-days from now, while CH4 on the ThinkPad needs MD1 after BD4, CH1 and CH3, about 15–19 lane-days
from now plus SH1. CH4 would wait about a week. MD1–MD3 are headless (`cadatomic-core` and a new `cadatomic-model`
crate, no GPU, wasm32-clean) and touch files no other lane is in (`palette.rs`, `inspect/selection.rs`, `tree/`),
so they suit the MacBook between quire's follow-ups and quire P1. quire P1 (completion, hover, signature help,
inlay hints) is first needed by ED3, which waits for svelters CX4.

**Why SH1 goes before CH1.** CH1 draws stripes, the command strip with disabled reasons and the header grammar; it
should draw them with DS1's metrics and colours, through the keymap's corrected text-focus signal, and with the
zoom commands. Doing SH1 first means CH1 is written once.

### 3.4 Lanes

Each lane lists: machine, size, depends (hard / soft), files, content, acceptance.

#### K1 `<code-editor>` (running, WSL)

- **Depends:** quire QE4 (done); RA1 (done); RA3 methods (soft: props and events first, methods when RA3 lands).
- **Files:** `cadatomic-app/src/svelte/code_editor.rs` (moves to `cadatomic-ui` in UI0), `ui/.svelters/app`.
- **Content:** ARCH-CRATES §4.2's facet table: `EditorState` in the provider block or behind a `doc` handle;
  props `language`, `readonly`, `options`, `value` (scratch), `doc` (handle), bindable `selection`; events
  `change`, `selectionchange`, `save`, `focus`/`blur`, `diagnosticaction`, `contextmenu`; `consume_key` and
  `composing()` (R11, EV3's provider seam) so the keymap and default buttons stay out of the editor's keys;
  `busy()` feeding `settled()` (R8).
- **Acceptance:** a kittest Svelte root with a `<code-editor doc={…}>` edits, undoes and saves through cadatomic's
  `text.edit` command at undo-group close (seam item 9); `bind:selection` is exact (one change per frame); Ctrl+Z in
  the editor never reaches the document keymap (`shortcut_override` true); an IME preedit blocks Enter from the
  default button; the editor's caret survives a move of its pane to another window (EM2).

#### UI0 framework crate and AppEmbed (new; WSL, after K1 and BD4 merge; 1–1.5)

- **Depends:** K1 and BD4 merged (both edit what moves). If BD4 is later than K1, LI1 starts first and UI0 follows
  it on the same machine.
- **Files:** new `crates/cadatomic-ui`; moves `cadatomic-app/src/svelte/*`, `views/svelte_view.rs`, K1's wrapper;
  BD4's adapter keeps its call sites. `ui/src/*` → `ui/panels/task/*`, `ui/lib/*` (TRACKS §3.3's paths).
- **Content:** the crate (§2.2); `AppEmbed` (one embed, slot and root tables keyed by `PaneId` or fixed keys, fault
  policy); the panel registry entries as data; `svelters-egui` with `source` on.
- **Acceptance:** no behaviour change: every FW1 and BD4 test passes unchanged; `cadatomic-ui` builds without
  wgpu and the kernel seam (`cargo tree -p cadatomic-ui` shows neither); a touch of a file in `cadatomic-ui`
  rebuilds `cadatomic-ui` and the app only (measured once, recorded).

#### LI1 the dev edit loop and DEMO1 (WSL, 3–5)

- **Depends:** hard on Q1, H1, H1b, H3, H3c, CX1, Q2 (all merged; needs the pin advance of §1.3) and K1 (the editor
  state model); soft on BD4 (open the editor as a berth pane; otherwise the egui_tiles seam, as TRACKS §3.3 allowed).
- **Files:** `cadatomic-ui` (AppEmbed loading and reload policy), `cadatomic-app` (`EditorView` pane, the open-file
  command, Problems feed), `scripts/check.sh`, `scripts/build-ui.sh` (deleted), `ui/vite.config.js` (deleted).
- **Content:** §2.13. `AppEmbed` loads `ui/` with `SourceProvider` (dev and hmr on); `EditorView` over egui_quire;
  `ide.open_source` command; overlay → `check()` → diagnostics; save → `apply_changes`; the remount-on-fallback
  policy; edit-to-pixels timing behind a dev flag. The LI1 builder writes its own milestone file (register).
- **Acceptance (DEMO1, TRACKS §3.3):** in a dev build, with the `body.box` task panel open, typed but uncommitted
  values and Placement expanded:
  1. opening `ui/panels/task/TaskPanel.svelte` and typing a syntax error shows inline diagnostics within one
     debounce, and the old panel keeps running;
  2. after a label change plus a new element is saved, both reach the screen, the typed values are still there,
     Placement is still expanded, and the root was not remounted (root id unchanged, H1 stats show a swap);
  3. edit-to-pixels is measured and reported against dock-W7's 300 ms target (no other builder running);
  4. a `<style>` edit (DEMO1b) applies through the same swap with the state kept, or the lane records why not;
  5. `check.sh` no longer runs `node`, `pnpm` or Vite; `ui/dist` is gone.
  Steps 1–2 are a kittest scenario as well as a manual run.

#### BD4 berth adoption (running, ThinkPad)

- As in the register: berth replaces egui_tiles; panes host svelters slots; `PaneHost::viewport_done` forwards to
  `show_viewport_passes`; `SvelteView` becomes a tab; command literals get `initial_size: None`.
- **Acceptance additions proposed here:** the adapter table of §2.3 has one test per row (kittest, two windows
  embedded); a Svelte pane moved to another window keeps its `$state` and its root id; closing a dirty pane runs the
  host guard before `Closed`; berth's `DockStyle` is filled from cadatomic's tokens in one adapter function (SH1
  later switches its source to DS1).

#### SH1 shell adoption: theme, keys and focus, settings, motion, platform (new; ThinkPad after BD4; 3–4)

- **Depends:** hard on DS1, EV3, DX1, DX2, TR2 (all merged; pin advance §1.3).
- **Files:** `cadatomic-app/src/{theme, shortcuts, shell/chrome.rs (colour reads only), app.rs (setup)}`,
  `cadatomic-core/src/{settings.rs, keymap.rs}`, `cadatomic-ui` theme adapter (or `cadatomic-app` if UI0 has not
  merged).
- **Content:**
  1. **Theme.** `svelters_egui::theme::Theme` replaces `theme::Tokens`: `Theme::vscode(scheme, contrast)` with
     cadatomic overrides (accent, the viewport canvas sets as `--cad-*` pass-through tokens, density `Compact` as
     the CAD default); `ui.theme` gains `hc-dark`/`hc-light`; `backend.set_theme` on change; berth `DockStyle` and
     quire `EditorTheme` filled from the typed getters in one adapter; `forms::AXIS_COLORS` moved into tokens.
     Fonts: Inter registered as the text family, the semibold face mapped through WG1's font-weight mapping (CS5f in
     flight adds measure and paint).
  2. **Keys and focus.** §2.10: `TEXT_FOCUS` from `focus_info`; `shortcut_override` before a binding consumes a
     stroke; the Edit menu through `text_command`; `default`/`cancel` buttons on the Svelte task panel's
     Accept/Cancel.
  3. **Settings.** `SettingsBackend` over the A1b layers (with `source` = layer), `UiStateBackend` into berth host
     data, `AppId`/`Paths`, `tracing` for `js` targets; `Host::set_dx` on `AppEmbed`.
  4. **Motion.** The OS reduced-motion preference (and a `ui.reduceMotion` setting) → `set_motion`.
  5. **Platform.** The first-party embed gets the desktop platform (DX2 `SystemClipboard`), so Svelte, quire and
     egui share one clipboard (§5.3 Q6).
- **Acceptance:**
  - one kittest frame holds a Rust panel button, a berth tab strip and a Svelte button: switching Dark Modern →
    Light Modern → HC dark moves all three to the same token values (DS1's test shape, in cadatomic);
  - Ctrl+Z in a focused Svelte `text-edit` undoes the field, not the document; with nothing focused it undoes the
    document; during an IME preedit, Enter does not accept the session;
  - Edit → Paste from the Rust menu pastes into the last focused Svelte field;
  - a Svelte `setting('units.display_length')` shows the layer it comes from, and a write to an environment-locked
    key is refused with `settings/locked`;
  - a split position inside a Svelte pane survives an app restart (UI state through berth host data);
  - reduced motion turns TR2 transitions into jumps (virtual clock);
  - no hard-coded colour remains in `shell/` (a lint, as the framework research asks).

#### CH1 shell regions, stripes, disable-with-reason, zoom (ThinkPad, 4–6)

- **Depends:** BD4, SH1; ZM1 for the zoom part (hard for that part only).
- **Files:** `shell/*`, `app.rs`, `action_bar/`, `cadatomic-core/src/{view, context}.rs`.
- **Content:** ide-shell §8 items 1–2: fixed regions, stripes with fixed slots, the editor header grammar, the
  command strip from the registry with disabled-with-reason tooltips (egui `on_disabled_hover_text` in Rust; WG6's
  disabled tooltips in Svelte); zoom per §2.11.
- **Acceptance:** a stripe slot per panel is stable across document kinds and workspaces (kittest over two kinds);
  every disabled strip item has a hover reason from the catalog; Ctrl+= three times gives scale 1.2³ (±1e-6), the
  setting persists in the user layer, and the viewport, berth and a Svelte panel all relayout at 80 %, 100 % and
  150 % with no clipped text in the gallery panel (ZM1's test shape); Ctrl+= zooms while a Svelte `text-edit`
  has focus (VS Code does the same: the field does not claim it), and the test pins that rule.

#### FW2 `Op<T>` waiting states (WSL after LI1, 4–6)

- **Depends:** CS5e, RA1 state bits, SG4 `StateProvider` (merged).
- **Files:** `cadatomic-ui` (`op/`), `ui/lib`, `cadatomic-core` (`Op` enum and the binding derivation, headless).
- **Content:** framework §3 in full: `Op<T>` derived from session, job and query machines; a `StateProvider`
  that sets `cad.pending`, `cad.stale`, `cad.failed`, `cad.pick-target` bits with the 150 ms delay and 300 ms minimum
  display; `command=` behaviour on `button` (title, icon id, shortcut, availability and reason); `JobChip`,
  `PendingMark`, `StaleBanner` as Svelte components styled through `:state()`; live regions for `Op` changes
  (catalog text).
- **Acceptance:** a 50 ms job never shows a pending mark; a 400 ms job shows it from 150 ms to at least 450 ms
  (virtual clock); the `button` with `command="body.boolean"` shows the registry's disabled reason as tooltip and
  a11y description; hovering a pending control runs no JS (H1 stats or a JS-call counter); `wait_idle` still
  returns while a cosmetic pending tick animates (it waits for the job, not the tick).

#### CH2 a Svelte task panel for every command (WSL after FW2, 5–7)

- **Depends:** hard on SV48 and RA2 (merged) and on CX3 for draft bindings (start against `onchange` events; switch
  when CX3 merges); soft on FM1 (commit-only `drag-value`) and WG6.
- **Files:** `cadatomic-ui` exports (`svelters:app/commands`, `svelters:app/task` generalised), `ui/panels/
  properties/*`, `task_panel/` and `forms/` (deleted at the end, §4).
- **Content:** §2.4: one `Properties.svelte` root; every input kind the egui form draws (quantities, vectors,
  directions, enums as segmented buttons, booleans, tagged unions with variant fields in declaration order,
  selection boxes with cardinality, collapsed groups); inline refusals at slot paths; the view-model in Rust
  (unchanged rule) crossing as `Json<T>`.
- **Acceptance:** every command in the registry opens the Svelte panel (no egui fallback left); every
  `task_forms` scenario passes on it with identical command-log lines; a typed value commits on Enter, blur or
  release and not per keystroke (draft-event counter with CX3); the egui panel, `forms/` and the `ui.task_panel`
  setting are deleted, or kept only behind safe mode if the coordinator decides so (§5.3 Q4).

#### CH3 Problems, Jobs, Output, status sections (ThinkPad after CH1, 6–8)

- **Depends:** hard on LS1 (merged); soft on WG5a (keyboard, sticky sections), SG6.
- **Files:** `problems/` (replaced), new `ui/panels/{problems, jobs, output, status}`, `cadatomic-ui` exports
  (`svelters:app/problems`, `jobs`, `output`).
- **Content:** §2.5. Rows from Rust stores (JSON pages over LS1's T1 tier until LS2), fix commands with
  applicability, `job.cancel`, Output channels with a bounded ring.
- **Acceptance:** 10k diagnostics scroll with no frame over budget and no rows built during a fling (LS1's
  RowPool assertion); keyboard-only navigation, Enter opens the source location, the Menu key opens the row's
  context menu; a fix command runs through the registry and logs one invocation; the status tool prompt updates in
  the same frame as the session step.

#### CH4 palette and model tree (ThinkPad after CH3, 5–7)

- **Depends:** hard on MD1–MD3 (MacBook); WG5b tree-view for the tree.
- **Files:** `palette/` (replaced), `cadatomic-core/src/palette.rs` (MD1's filter job), `tree/` (replaced), `ui/
  panels/{palette, tree}`.
- **Content:** §2.6 and the model tree on `TreeFlatten` through `tree-view`; U4 metadata lint (§5.3 Q16).
- **Acceptance:** palette results show shortcut, menu path, disabled reason and a fix; typing filters 5k commands
  within one frame of the filter job's result; "show me where" outlines the real strip button; the tree scrolls 10k
  nodes with no JS per scrolled row (counted) and keeps selection by key across a rebuild.

#### MD1–MD3 models (MacBook after the quire follow-ups; 4–6 each)

- As in the models research §11 steps 1–3: `RowSet`, keys, deltas and `Filter`/`Sort` proxies, with the palette's
  worker as a tethered filter job (MD1); the query AST and search-bar parser over reflected schemas, `cadatomic
  query` (MD2); `TreeFlatten` and the selection model, with the A6a tree moved onto them (MD3).
- **Acceptance:** the research's MX3/MX10 tests; the egui palette and tree keep working on the new models before
  CH4 replaces their views; `cadatomic-core` and the new crate still build for wasm32.

#### AT1 automation mount (slot-in after AU1; RPC half after AU2; 2–3)

- **Content:** §2.15; the settings switch (default off in user builds); `KittestDrive` over `Driver::InProcess`;
  one existing scenario rewritten to use locators for its Svelte steps.
- **Acceptance:** the same selector resolves to the same node through kittest and through the socket (SL5);
  `wait.settled` returns only after cadatomic's `IdleBarrier` (a job in flight holds it); with the switch off, the
  methods answer "disabled" and no socket opens.

#### ICR icon registry switch-over (slot-in after DS5 and AS1; 1)

- **Content:** §2.12: icon ids in the command registry and the type registry; `theme/icons.rs` consts replaced by
  ids (23 to codicons, 10 to the custom set as drawn, ICONS §1.2); the optical-size selection hook; the licence
  credits in `assets/licenses.toml`.
- **Acceptance:** no code point literal remains outside the generated table; a 12 px tree icon and a 24 px strip
  icon for the same id come from different masters; at 150 % zoom a 14 px tree box picks the 20 px master (the
  nearest at or below 14 × 1.5 = 21 px), never a scaled 24.

#### IC2 MCAD icon batch (art lane; after the user approves IC0's sheet; about 1 lane plus review)

- Drawn to IC0's guide in the optical sizes the registry needs, linted by IC1, delivered as contact sheets for the
  user (I2). No cadatomic build; the snapshot test lands with ICR or the next lane that builds.

#### FW3 typed picking (WSL after CH2, 6–8)

- **Depends:** hard on EV1 (filters, overlay trait); the type registry (framework §4) in `cadatomic-core`.
- **Content:** ARCH-SCENE §10.1: `PRESENTS` ext key, `use:presents`, `PickProvider`, `PickOverlay` pass, the
  viewport's `Hit::Part` from the GPU pick readback, clicks emitting draft events.
- **Acceptance:** while a slot of type `Face` acquires, every presenting row in the tree, the PropertyGrid and the
  viewport shows `pick-target` or `pick-rejected`; hovering one outlines every place showing the same value; a
  click fills the slot through the command path (one draft event); per hover frame no JS runs.

#### FW4 PropertyGrid and EmptyState (WSL after FW3, 6–8)

- **Depends:** hard on WG5a (cell edit protocol); soft on SG6, FM1.
- **Content:** §2.7.
- **Acceptance:** a 2k-row grid scrolls with no JS per row and edits a quantity cell through F2/Enter/Escape;
  mixed values and reset to default work on a multi-selection; the gallery app shows every state in four themes.

#### Later lanes (sized when they are next)

| Lane | Content | Depends | Size |
|---|---|---|---|
| **SET2** settings page | §2.9 on FW4; typed settings if the user agrees (§5.3 Q11) | FW4, SH1 | 3–5 |
| **KM1** keymap editor | §2.10; conflicts, presets, chords | FW4, WG6 `key-sequence-edit` | 3–4 |
| **VP1** viewport overlay root | §2.8: HUD, labels, slot prompt at the cursor | FW3, SG4 | 4–6 |
| **LI2** release loop | source layers, overlays with base hash, safe mode, last good, staged apply (js-api §6.3–6.6, AP7/AP8) | svelters Q4; J6 answered | 6–8 |
| **LI3** API publishing | `ApiRegistry`, `cadatomic:api/*`, typings, MCP `ide.*`/`api.*` | RA3 (SV23), LD1 | 8–10 |
| **EX1** extension panel slots | §2.14: manifest, stripe slot assignment, confinement, principal ids | §5.3 Q9 | 3–4 |
| **EM3a** adaptive hidden roots | D5 through svelters EM3 | EM3 | 2–3 |
| **MD4** lists on Rust models | SV40/SV41 adapters, then T3 | svelters LS2 | 3–5 |
| **WS1** workspace and document model split | register, P1 | — | 6–8 |
| **APP1** app-level gaps | design pass first (register: each gets questions for the user), then recent files, single instance, files changed on disk, About with licences | — | 1 (design) + 3–5 |
| **ED3** language services | `cadatomic-lsp`, svelters-ls over SV33 | svelters CX4 | P1 scale |

## 4. Where cadatomic should delete its own workarounds

| # | Workaround (file) | Why it exists | Replaced by | Delete in | Precondition |
|---|---|---|---|---|---|
| 1 | Vite build: `scripts/build-ui.sh`, `ui/vite.config.js`, `ui/node_modules` links, `ui/dist/embed.js`, `CADATOMIC_UI_BUNDLE` | FW1 predates Q1/Q3w/Q2 | `AppEmbed::load_from(SourceProvider)` (svelters-egui `source`) | LI1 | pin ≥ `1786957f` (Q2) |
| 2 | `svelters-compiler` as an optional dependency of the app behind `ui-compiler` | the script needed the compiler binary | the compiler linked through `svelters-egui/source` (live compile, decision 5) | LI1 | 1 |
| 3 | The node_modules-copying half of `scripts/svelters-snapshot.sh` | Vite resolved `@svelters/*` into the live checkout | nothing (no JS toolchain in the gate); the cargo half stays | LI1 | 1; svelte-check, if kept, becomes an optional dev tool |
| 4 | `SvelteTaskPanel`'s private embed and `SharedSlot` (`Rc<RefCell<Option<(Embed, SlotId)>>>`), `SvelteView` outside the `ViewRegistry` | one panel, one slot (FW1 Q2) | `AppEmbed` + panel registry | BD4 / UI0 | — |
| 5 | egui_tiles, `dock/seam.rs`, `dock/viewport_tabs.rs`, layout file v1 | decision 14 | berth | BD4 | — |
| 6 | `TEXT_FOCUS = ctx.egui_wants_keyboard_input()` | no claim API existed | `focus_info`, `shortcut_override`, quire `composing()` | SH1 | pin ≥ `c41dca7a` (EV3) |
| 7 | `session.accept`/`session.cancel` keymap intents on Enter/Escape for the Svelte panel | no default buttons | EV3 `default`/`cancel` buttons (keep the intents for the viewport) | SH1 | 6 |
| 8 | `theme::Tokens`, the hand-built `Visuals`, `forms::AXIS_COLORS` | cadatomic's look predates DS1 | `Theme` + cadatomic overrides | SH1 | pin ≥ `f1f5810b` (DS1) |
| 9 | `Platform::headless()` on the first-party embed | no reason for Svelte to reach the clipboard yet | the desktop platform for first-party roots; headless stays for isolates | SH1 | §5.3 Q6 |
| 10 | No `Dx`: `svelters:settings` answers `settings/unavailable` | not needed by the task panel | `SettingsBackend`/`UiStateBackend` over A1b and berth host data | SH1 | — |
| 11 | Phosphor font and `theme/icons.rs` code points | interim icon set | icon ids, DS5 table, cadatomic's own set | ICR | DS5, AS1 |
| 12 | `ui.task_panel` setting, the egui task panel (`task_panel/`), `forms/` (1.3k lines) and the "fall back to egui for inputs Svelte cannot draw" rule | FW1 shipped one command's inputs | CH2 | CH2 | §5.3 Q4 |
| 13 | Hand-painted egui rows in `problems/`, `palette/`, `tree/` | A6a, A2b predate svelters | Svelte roots on `virtual-list`/`tree-view` | CH3, CH4 | MD1–MD3 for CH4 |
| 14 | Framework research's own-scene plan (`KindAdapter`, "svelters arena behind the `Component` trait", §2, §12 step 5) and the Rust `ListItem` component (§12 step 4) | written before C2 was settled | the svelters scene with providers; Svelte rows | doc edit, now | — (TRACKS §5.6 item 2 already asks for it) |
| 15 | egui_mcp `ui.tree` as the way tests reach Svelte nodes | no svelters automation | `scene.*`/`a11y.*` locators for Svelte; egui_mcp stays for Rust-drawn egui | AT1 | AU1 |

Not workarounds, so kept: the Rust-computed task view-model (register, FW1 answers); the Rust command strip, menus
and status bar frame (user split); the Rust editor surface (js-api §6.6); egui's zoom keys off.

## 5. Risks and open questions

### 5.1 Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | **Build load.** Three cadatomic builders (one per machine) plus svelters lanes on the same slots; cadatomic worktree targets of 18–51 GB; the WSL disk is a file on `C:` | MD lanes on the MacBook build only `cadatomic-core` and the model crate until CH4; delete merged worktree targets at once; UI0's crate split shrinks rebuilds; watch `df -h /mnt/c` |
| R2 | **Hot spots.** `app.rs`, `shell/*`, `views/` and the Svelte host are touched by BD4, SH1, CH1, LI1 and UI0 | the order of §3.3 (BD4 → SH1 → CH1 on one machine; K1 → UI0 → LI1 on another); UI0 lands as soon as both K1 and BD4 merge; SH1 reads colours in `shell/chrome.rs` but changes no layout |
| R3 | **svelters churn.** svelters master merges several lanes a day; each cadatomic pin advance costs a `check.sh` | advance only at the start of a lane that needs a new milestone, at most once a day, through `svelters-snapshot.sh`; ARCH-SCENE §16 lists the stable API, and cadatomic does not enable `unstable-scene` |
| R4 | **Frame budget with many roots.** One QuickJS runtime runs every first-party root's JS in `logic`; a heavy panel delays all | per-root budget attribution (ARCH-EMBED §0.3) shown in a dev overlay; SG6's paint cache and culling; hidden panes paused (SV19); measure in CH3 and FW4 with 10k rows |
| R5 | **HMR in embed mode** does not fall back to a full reload by itself (H1 record) | LI1's policy: remount only the affected entry's roots with their last props; error state with Revert; the H1 swap budget (200) forces a remount of all roots, recorded |
| R6 | **Accessibility across Rust and Svelte.** Two producers of AccessKit nodes in one tree; berth, quire and Svelte must not clash on ids | anchors from domain ids; `RootKey` = `PaneId`; TK3's audit in cadatomic's kittest once it exists; screen-reader checks across windows (dock-W9) with AU6 later |
| R7 | **Key routing regressions** when SH1 moves the text-focus signal | SH1's acceptance list covers Ctrl+Z, Edit menu, IME and Enter/Escape; the existing shortcut tests stay green |
| R8 | **DEMO1 regresses to "works in the demo only."** | LI1 makes the loader the only path (the bundle is deleted), so every later panel lane runs on it |
| R9 | **Unscheduled svelters work** (LS2, Q4, EM3, ISO1, DS5) blocks cadatomic lanes later | §5.2 asks svelters-agent to order them; cadatomic lanes start against the stubs TRACKS lists (JSON pages for LS2) |
| R10 | **Icon volume** (about 460 icons in several optical sizes) and the user's review time | batches in the order surfaces need them (chrome visible in CH1–CH4 first, then MCAD); sizes 12/14 only for icons used in tree and status bar |

### 5.2 Requests to svelters-agent (scheduling, its call)

1. **LS2** (row-model trait, columns, tree rows, key multi-selection) before CH3 needs more than JS pages, i.e. in
   about three weeks. Recommendation: schedule after WG5a/WG5b merge.
2. **Q4** (release bundle with the compiler linked in) before LI2; it is also what makes release builds of DEMO2
   possible. Recommendation: after AS1, as ASSETS orders it.
3. **DS5** is queued behind VIS1; ICR and every icon-bearing panel wait for it. Recommendation: let DS5's id table
   land independently of VIS1's tuning (it changes no look).
4. **EM3** (D5) and **ISO1** stay later; no cadatomic lane needs them in the next four weeks.
5. **ZM1** before CH1's zoom part (about two weeks out).

### 5.3 Open questions

Marked **U** (architecture or a reversal: the coordinator takes it to the user with the recommendation) or **C**
(a coordinator decides; the deciding coordinator is named).

| # | Question | Recommendation | Who |
|---|---|---|---|
| Q1 | One first-party `Embed` (one runtime, many roots) or one embed per pane? | **One**, plus a protected tooling embed and later one per isolate. Per-pane runtimes multiply memory and break shared stores; ARCH-EMBED's model is one runtime, many roots | C (main-cadatomic; ARCH-EMBED already assumes it) |
| Q2 | DEMO1's editor: a Rust `EditorView` pane over egui_quire, or `<code-editor>` inside a Svelte root? | **Rust pane** for the IDE's own editing surface (js-api §6.6), sharing `EditorState` with K1's kind; `<code-editor>` for Svelte panels | C (main-cadatomic; implementation inside the agreed js-api design) |
| Q3 | Create `cadatomic-ui` now (UI0)? | **Yes**, right after K1 and BD4: C1 already put the framework in its own crate, and it shortens rebuilds | C (main-cadatomic) |
| Q4 | Delete the egui task panel and `forms/` when CH2 reaches parity, or keep it for safe mode? | **Delete.** Safe mode (js-api §6.6) protects the editing surface, not panels; the first-party task panel is built in and not overlay-editable unless the user edits it, in which case "Revert" applies | C (main-cadatomic; FW1's default was a coordinator choice) |
| Q5 | Release builds: first-party sources in the binary or fetched by build hash (J6), and the live IDE in release | **IDE and HMR in release, sources fetched on demand** (js-api J6 proposal). Until Q4 exists, release builds load from source like dev builds | U (J6 is open in cadatomic's research) |
| Q6 | The first-party embed's platform: desktop clipboard and shell, or stay headless? | **Desktop** for first-party roots (one clipboard for Svelte, quire, egui; EV3's Edit-menu paste needs it); headless stays the default for isolates | C (main-cadatomic; "security must not slow development", first-party code is trusted by D4) |
| Q7 | Problems/Jobs/Output data path before LS2: JSON pages in stores (T1) or wait? | **T1 now**, switching at MD4 | C (main-cadatomic) |
| Q8 | Zoom per window or global | **Global first**, per window when D1 windows get per-window settings | C (main-cadatomic; inside the zoom decision) |
| Q9 | May the user's own dev extensions (local folder) run Svelte panels in process before ISO1? D4 says isolates are for third-party extensions | **Yes for dev extensions the user writes**, under the loader's confinement, with a trust prompt per folder (Zed lesson 7); third-party packages wait for ISO1 | U (touches D4 and the extension policy) |
| Q10 | Optical sizes to draw per icon: all five, or only where used? | **Only where used**: 16 everywhere, 12/14 for tree and status icons, 20/24 for the command strip; the registry records which exist | C (main-cadatomic); the user approves each batch (I2) |
| Q11 | Typed settings with a generated schema, per-key error tolerance and dated migrations (Zed lesson 5) before SET2 | **Yes**, as part of SET2's first half; today's `Bool`/`Text` kinds cannot drive a settings page | U (register: Zed adoptions go to the user when planned) |
| Q12 | Adopt svelters' deterministic test scheduler (SCHED1, Zed lesson 1) for cadatomic's jobs and HMR tests | **Yes, when SCHED1 merges**, starting with `Jobs::drain` and LI1's reload tests | U (same rule) |
| Q13 | Keyboard-layout correctness (non-US layouts, IME first, Option-letter policy; Zed lesson 6) | **Defer** until KM1; SH1 already puts IME first through `composing()` | U (same rule) |
| Q14 | Pane identity for UI state: `RootKey` = `PaneId`, anchors inside the root | **Yes**; a pane closed and reopened with the same id gets its state back | C (main-cadatomic) |
| Q15 | Op states styled through provider state bits (`:state(cad.pending)`) or kind props | **State bits** (no JS on change; CS5e matches them); kind props only for kinds that paint themselves | C (main-cadatomic) |
| Q16 | Enforce U4's "synonyms and an example scenario before a command appears in the palette" in CH4 | **Warn in CH4, enforce in `check.sh` once the registry's commands have the metadata**; otherwise most commands vanish from the palette on day one | C (main-cadatomic; inside the user's U4 answer) |
| Q17 | MD1–MD3 on the MacBook, ahead of quire P1 | **Yes** (§3.3) | C (main-cadatomic) |
| Q18 | svelters scheduling of LS2, Q4, DS5 independent of VIS1, ZM1 timing (§5.2) | as §5.2 | C (svelters-agent) |

## Sources

- svelters `ceb5b0be`: `STATUS.md`; `plans/research/TRACKS.md` §2.2, §3.2–§3.3, §4.3, §5; `CADATOMIC.md` §2–§3.1;
  `ARCH-CRATES.md` §4.1–§4.2, §5; `ARCH-EMBED.md` §7; `ARCH-SCENE.md` §10, §16; `DESIGN.md` §3–§5, §7;
  `E2E-INSPECTOR.md` §1.10–§1.11, §5–§6; `ICONS.md` §5–§6; `ZED-LESSONS.md` §4, §9; `QT-SURVEY.md` §1–§3;
  `ASSETS.md` (lane table); `plans/milestones/{Q1-module-loader, H1-component-hmr, H3-state-preservation,
  DX1-settings-diagnostics, EV3-focus-keys, DS1-theme-module}.md`; `crates/svelters-egui/Cargo.toml` (features),
  `src/embed/mod.rs` (`load`, `load_from`, `load_app_from`); `crates/svelters-modules/src/source/mod.rs`
  (`spec_fragments`); `git log --first-parent a52d437c..ceb5b0be`.
- cadatomic `1e94d9d`: `CLAUDE.md`; `STATUS.md` (handoff); `docs/plan/00-decisions.md` (entries from "Product
  direction and UI framework" to "UI zoom like VS Code"); `docs/plan/04-kernel-requests.md` (SV rows);
  `docs/plan/02-phase-a.md` (order); `docs/research/2026-10-22-ide-shell-and-ux.md` §2–§9;
  `2026-10-22-component-framework.md` §2–§3, §9, §11–§12; `2026-10-22-js-api-publishing-and-live-ide.md` §6.3–§6.7,
  §9–§10; `2026-10-22-models-filtering-and-dynamic-data.md` §11; `crates/cadatomic-app/src/svelte/{mod, panel}.rs`,
  `views/svelte_view.rs`, `theme/{mod, icons}.rs`, `shortcuts/mod.rs`; `crates/cadatomic-core/src/{settings,
  keymap}.rs`; `crates/cadatomic-app/Cargo.toml`; `ui/{package.json, vite.config.js, src/*}`; `scripts/build-ui.sh`.
