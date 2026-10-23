# DOCS-PLAN: documentation for svelters-egui

Status: research and plan, docs only. Nothing is decided. 2026-10-22, svelters master `424ebcf4`, cadatomic
master as read on 2026-10-22 (pin `a52d437c`, 39 commits behind svelters master). Request (user, via
svelters-agent): plan the documentation for svelters on egui, so that cadatomic agents know what to do. It
covers the Rust integration side, the Svelte side, the widgets, CSS and the other features.

Samples written in full: `plans/research/docs-samples/` (§8).

## 0. Summary

1. **The primary readers are AI agents working in cadatomic,** and later extension authors. They need three
   things: the right API found fast, used correctly, and not reinvented. Today none of the three is reliable.
   cadatomic's agents read `plans/research/*` design documents as if they were API docs. Some of those
   documents describe the code of seven days ago. As a result the agents:
   - passed JSON text through exports after `Json<T>` had landed;
   - listed features as "to file" that existed at their pin;
   - painted state outlines by hand instead of using `:state()`;
   - wrote a test settle loop and a linker-retention hack of their own (§2).
2. **The facts are mostly there already, in the wrong shape.**
   - `metadata/semantics/egui.json` documents every kind's props, signals and methods (47 kinds).
   - `types/egui.d.ts` is generated, with JSDoc.
   - `codes.rs` registers 172 diagnostic codes and already generates a docs page.
   - The style tables are generated.
   - The rustdoc of `svelters-egui` and `svelters` is long and accurate, but written in lane voice ("Writer
     lane", "§8.5", milestone ids).
   - What's missing: one entry point, task-shaped pages, a status table, and any check that keeps prose and
     examples true.
3. **Where the docs live:**
   - hand-written pages go in `docs/egui/`, a plain Markdown tree in the repo, versioned with the code;
   - generated reference pages are committed and freshness-checked, like `diagnostics.md` and `egui.json`;
   - rustdoc stays the Rust API reference;
   - the Starlight site renders `docs/egui/`.

   cadatomic builds against the sibling checkout `../svelters`, so agents read the docs at exactly their pin.
4. **Entry points:**
   - `docs/egui/START-HERE.md`, a CLAUDE.md-style page: the model in six lines, the frame contract, a do/don't
     table built from cadatomic's actual mistakes, an "it already exists" table and a task→page map;
   - `docs/egui/llms.txt`, a generated index;
   - `docs/egui/status.md`, every SV/SVP request with its state and the sha that delivered it;
   - `svelters docs egui <tag>[.<member>]`, an offline lookup through the existing CL0 CLI once the egui pack
     exists.
5. **The information architecture runs to about 130 pages in 11 sections (§5):**
   - concepts;
   - Rust integration (28 pages);
   - Svelte side (20);
   - widget reference (48, mostly generated);
   - CSS (9);
   - icons;
   - recipes (12);
   - diagnostics;
   - troubleshooting;
   - status;
   - contributor pages.
6. **Generation (§6):**
   - the widget reference, from `egui.json` plus four new per-kind fields (`category`, `states`, `a11y`,
     `paint`);
   - the CSS property matrix, the selector and state table and the token list, from the style and theme
     tables;
   - the diagnostics index, from `codes.rs`, with rustc-style explanations;
   - the module catalogue, from the `.d.ts` files.

   Hand-written text lives between generated regions of the same file, so writers and generators don't fight.
7. **Checking (§6.3):**
   - every code block in a page is a *snippet* of a file that CI compiles: Rust in a new `examples/egui-docs`
     crate, Svelte as testkit cases that run on egui;
   - rustdoc gets `cargo doc -D broken_intra_doc_links` and `cargo test --doc`. CI runs neither today:
     nextest skips doctests, and 12 of svelters-egui's examples are `ignore`;
   - a generated `doc_api.rs` names every Rust item the pages cite, so renaming one breaks the build;
   - an owner map flags API changes that touch no page.
8. **Lanes (§7):** 13 doc lanes, about 21 lane-days, in four waves over three machines. That is roughly a
   week of calendar time.
   - Agent-critical pages come first: START-HERE, status and the stub map on day 1; then the frame order, data
     across the bridge, setup, testing, events and bindings, and the CSS cookbook.
   - Feature lanes still in flight write their own pages, under a new CLAUDE.md rule (§9). AS1, AU1, DS5/IC1,
     ZM1, EV1, CX3 and RA3 are among them.
9. **Ten open questions with recommendations (§10).** All of them are procedural, so svelters-agent can decide
   them. The exception is Q8 (publishing the site publicly), which is the user's.

## 1. What exists (verified on `424ebcf4`)

| Source | What it has | Shape for an agent |
|---|---|---|
| rustdoc, `crates/svelters-egui` (30k lines, 87 `.rs` files) | module docs for every file. `Embed`, `Widget`, `Behavior`, focus, theme, style, dnd and dialogs are explained well (`embed/mod.rs:1-40`, `provider/mod.rs:1-40`, `focus.rs`, `theme/mod.rs`) | lane voice (milestone ids, `§` refs, "Writer lane"). 12 ` ```ignore` blocks, never compiled. **No doctests in CI**: nextest doesn't run them, and no workflow runs `cargo test --doc` or `cargo doc`. Stale spots: `menus.rs:10-11` says focus restore is "EV3's (not here yet)", but EV3 merged |
| rustdoc, `crates/svelters/src/lib.rs:1-150` | the best single text on exports, offload, buffers, stores, services, `Json<T>`, enums, widgets and behaviours | accurate. cadatomic's agents never found it (§2, item 1) |
| `metadata/semantics/egui.json` (generated by `spec.rs`, freshness test `tests/spec.rs`) | 47 kinds / 45 tags: props (type, default, observe class, doc), signals (args, doc), methods, `bindings`, `chain`, `style` (47 table properties with tags) | complete for props, events and methods. **Missing:** which states a kind sets, its a11y role and name source, whether egui or svelters paints it (so which CSS properties apply), a category, examples |
| `packages/native-runtime/types/egui.d.ts` (925 lines, generated) | `IntrinsicElements` with JSDoc per prop and handler | good for svelte-check. No method typings: `call('request-focus')` is untyped |
| `crates/svelters-style/src/tables.generated.rs` | the egui table: 47 properties, `via: setter` (layout) or `css` (engine paint) | no page shows it for egui. The site's styling page reads GTK/Qt only |
| `crates/svelters-compiler/src/codes.rs` | 172 codes, 62 of them `svelters::*` (style 14, bind 11, slot 10, element 6, event 6, list 6, …), a summary and fix-it each; `svelters-compile --explain`, `--list-codes`. Generates `docs/site/src/content/docs/guides/diagnostics.md` (`cx1_codes` test, `UPDATE=1`) | one line per code. No "why" text, no example, and nothing checks the summary against what triggers the code |
| `docs/site/` (Astro Starlight, D1) | 13 GTK/Qt-voiced guides (events, bindings, styling, rust-exports, …), about 880 generated GTK/Qt pages, link checker; `pnpm docs:build` (not in CI), `docs:test` (in CI) | egui is listed as `planned` (`site.config.mjs:17`). No embedding page. The guides' facts are mostly true on egui, but their examples use GTK tags |
| `svelters docs` CLI (CL0) | offline lookup `svelters docs gtk button.clicked`, `--json`, `-k` search, a docs pack built from the IR | GTK and Qt only. An egui IR frontend would make it the fastest lookup an agent has |
| `tests/cases/*.svelte` (TK1) | one-file cases with `@test` steps, run on egui (kittest) and four other backends, with snapshots on lavapipe | the right vehicle for checked Svelte examples |
| Fixtures and examples | `fixtures/gallery-egui` (one page per widget family), `fixtures/embed-egui` (TaskPanel, Pane, Panel), `examples/egui-kinds` (a kind, a behaviour, the dump flow) | accurate, but nothing points at them. `examples/egui-kinds/src/lib.rs:5` documents `onaccepted={(e, v) => …}` while the app uses `(v) => …` |
| `plans/milestones/*.md` (88 files) | the as-built record of each lane: decisions, limits, follow-ups | precise, but organised by lane, not by task |
| `plans/research/*.md` (22 files, 15k lines) | design: ARCH-EMBED, ARCH-SCENE (§16 is the stability contract), CSS, HMR, ASSETS, DESIGN, ICONS, E2E-INSPECTOR | design history, with proposals and decisions mixed. **This is what cadatomic sends its agents to read** (§2) |
| `MATRIX.md` | the feature matrix | says "egui (M7) is planned and not started" (line 15). Stale since G1 |
| `packages/native-runtime/types/embed.d.ts:5` | tells apps to use `"types": ["@svelters/native-runtime/types", …]` | that is the GTK vocabulary. egui apps need `types/egui` (STATUS handoff "to revisit") |
| DX1 modules (`svelters:settings`, `svelters:paths`, `svelters:log`) | runtime built (`svelters-host/src/dx/*.js`) | **no `.d.ts`**, so an import is a type error under svelte-check. This is a code gap the doc lanes should file |

## 2. What cadatomic's agents got wrong (read-only survey of `~/projects/cadatomic`)

cadatomic uses `Embed` (with `prepass_plugin: false`), one `#[svelters::widget]` (`quantity-field`), a
`#[svelters::store]` and exports. Its Svelte is `ui/src/TaskPanel.svelte`, `SlotRow.svelte` and `lib/task.ts`.
It uses no behaviours, no state providers, no `<style>`, no theme, no testkit and no focus API. Its agents get
their svelters knowledge from `CLAUDE.md:125-135` ("SV1–SV47 answers are in `plans/research/CADATOMIC.md`") and
from cadatomic's own research notes. Each gap below is something a page in §5 has to prevent.

| # | What happened | Evidence (cadatomic) | Root cause | Page that fixes it |
|---|---|---|---|---|
| 1 | Records crossed as JSON text with `JSON.parse` in Svelte, "until SV48 lands". SV48 was already an ancestor of the pin | `crates/cadatomic-app/src/svelte/exports.rs:13-14,63,65,193,245`; `ui/src/TaskPanel.svelte:9-10` | nobody told them what crosses natively | `rust/data-across-the-bridge.md`, START-HERE row 2 |
| 2 | The SV table lists SV1, SV2, SV4, SV17, SV18, SV19, SV40 and SV48 as "to file" or open, though each is merged at the pin | `docs/plan/04-kernel-requests.md:92-140` | no status table keyed to SV ids and shas | `status.md` |
| 3 | Their research notes say `#[svelters::widget]` is "designed but not built" and that `class:`/`style:` are rejected | `docs/research/2026-10-22-component-framework.md:26`, `scene-node-and-svelte-semantics.md:12,229` | snapshot knowledge with no expiry | `status.md`, the `verified_at` header on every page |
| 4 | Invalid state painted by hand (`rect_stroke` in `error_fg_color`) instead of a `:state(cad.invalid)` rule. Columns aligned with fixed-width spacer boxes instead of grid | `src/svelte/quantity_field.rs:141-147`, `ui/src/SlotRow.svelte:16` | no egui styling guide | `css/states.md`, `css/cookbook.md`, `recipes/invalid-field.md` |
| 5 | A `reopen` counter forces a `bind:open` getter to be re-read (a controlled `collapsing-header`) | `TaskPanel.svelte:18-27` | no "controlled prop on a self-toggling element" pattern | `svelte/components.md#controlled-props` |
| 6 | Renamed a prop after finding by trial that `slot` is reserved | `SlotRow.svelte:8` | reserved names are undocumented | `svelte/components.md#reserved-names` |
| 7 | Host shortcuts gate on `ctx.egui_wants_keyboard_input()`. `Embed::focus_info`/`shortcut_override` (EV3) are unused, though their rustdoc says "cadatomic's TextFocus layer reads it" | `src/shortcuts/mod.rs:60` | rustdoc isn't a place agents look | `rust/focus-and-keys.md` |
| 8 | Test settle by running `h.run()` 4 times. Text fields found by `format!("{:?}", role).contains("TextInput")`. testkit unused | `tests/task_forms_svelte.rs:117,140` | `EmbedHarness::run_until_settled` and anchors are undocumented | `rust/testing.md` |
| 9 | `std::hint::black_box(...)` to keep `inventory` registrations linked, found by trial | `src/svelte/mod.rs:56-61`, `quantity_field.rs:162-165` | the idiom exists only in `examples/egui-kinds/src/lib.rs:231` | `rust/setup.md#linking`, START-HERE |
| 10 | Consumer toolchain found by trial: `build-ui.sh` symlinks svelters' `node_modules`; features (SVP7); `rquickjs-sys` opt-level (SVP8); `cargo fmt --all` reaching path deps; licence fallout | `scripts/build-ui.sh`; `Cargo.toml:57-60`; `docs/plan/00-decisions.md:1017` | no consumer setup page | `rust/setup.md`, `svelte/project-setup.md` |
| 11 | A `thread_local!` bridge lets free `#[export]` fns reach the panel's store, forcing one panel per thread | `exports.rs:127` | the app-state patterns (object in props, store, service) are undocumented | `rust/data-across-the-bridge.md#app-state` |
| 12 | `<code-editor>` can't expose `revealLine` etc. (RA3 in progress); there's no interim advice | worktree `k1-code-editor`, `code_editor/core.rs:12-14` | in-progress features have no page | `status.md` rows give the interim pattern |
| 13 | cadatomic's CLAUDE.md says svelters plans are on branch `research/cadatomic`, but they're on master | `CLAUDE.md:127` | — | the snippet for cadatomic's CLAUDE.md (§3.3) |

None of this is cadatomic's agents being careless. They read what they were pointed at, and what they were
pointed at was design history.

## 3. Readers, entry points, locations

### 3.1 Readers

| Reader | Arrives with | Needs |
|---|---|---|
| **cadatomic agent, Svelte panel** (most frequent) | "build the sketch-constraints panel" | tags, props, events, `bind:`, layout, CSS, how to get data from Rust, how to test |
| **cadatomic agent, Rust integration** | "host this panel in a pane", "add a kind" | the frame order, `Embed` calls, kinds, behaviours, state, the scene API, theme, focus |
| **cadatomic coordinator** (main-cadatomic) | "is SVxx done? what pin do we need?" | `status.md` with shas |
| **extension author** (later) | "write a panel for cadatomic" | the Svelte side, the recipes, and the extension recipe's limits (security deferred) |
| **svelters lane agent** | changing an API | the owner map, `CONVENTIONS.md`, the generators |

### 3.2 Entry points

1. **`docs/egui/START-HERE.md`** (sample: `docs-samples/START-HERE.md`). One screen and a half:
   - the model in six lines;
   - the frame contract in 15 lines of Rust;
   - a do/don't table built from §2;
   - "check that it doesn't exist" with a status column;
   - a task→page map;
   - the status words.

   It has to stay under 150 lines. Every row links to a page that carries the detail.
2. **`docs/egui/llms.txt`**, in the llmstxt.org format: a title, a one-paragraph summary, then one
   `- [title](path): description` line per page, grouped by section. It is generated from the pages' front
   matter (`tools/docs/egui/gen_index.py`, checked in `docs:test`). No `llms-full.txt`: the pages are short
   and an agent opens the two it needs.
3. **`docs/egui/status.md`**. Every SV1–SV48 and SVP1–SVP9 row, plus the features cadatomic asks about, each
   with: state, the sha that delivered it, the page, and the interim pattern while it's in progress. The
   coordinator updates it in each merge commit (STATUS's "Requests from cadatomic" table shrinks to a link).
4. **`svelters docs egui <tag>[.<member>]`**: the CL0 CLI with an egui pack (lane DOC1). It answers in about
   15 ms with no file reading. `--json` is for tools.
5. **rustdoc**: `tools/dev/cargo-slot cargo doc -p svelters-egui -p svelters --no-deps`. Pages link to items
   by path, and `doc_api.rs` (§6.3) checks those paths.

### 3.3 Where docs live

| Kind | Location | Why |
|---|---|---|
| Hand-written guides, recipes, concepts | `docs/egui/**/*.md`, plain Markdown with Starlight-compatible front matter | cadatomic reads `../svelters/docs/egui/` at its pin with a plain file read. It's short to type and greppable. The site is a view of it |
| Generated reference (widgets, CSS matrix, tokens, modules, diagnostics) | committed inside the same tree, between `BEGIN/END GENERATED` markers | agents grep a pinned checkout without building. The diff shows API changes in review. Same pattern as `diagnostics.md` and `egui.json` |
| Rust API detail | rustdoc in the crates | it's compiled with the code. Pages summarise and link to it |
| Checked examples | `examples/egui-docs/` (Rust), `docs/egui/cases/*.svelte` (testkit cases) | CI compiles and runs them. Pages carry literal copies kept in sync (§6.3) |
| Design history | `plans/research/`, `plans/milestones/` (unchanged) | DOC0 adds a one-line banner to the ARCH/EGUI/CSS/HMR/ASSETS/DESIGN/ICONS docs: "Design record. For usage see `docs/egui/...`" |
| Site | `docs/site` renders `docs/egui/` as a content collection (DOC11) | humans; search |

**Snippet for cadatomic's CLAUDE.md** (main-cadatomic owns that file; svelters-agent sends it):

```markdown
## svelters (the UI framework)
Before any UI work read `../svelters/docs/egui/START-HERE.md` in the checkout we build against (its sha is
`pins.toml`'s). `docs/egui/status.md` says what exists at that sha. `plans/research/*` in svelters is design
history, not API docs. Missing features go in `docs/plan/04-kernel-requests.md`; don't work around them in silence.
```

## 4. Principles for agent-facing pages

1. **Truth at the pin.** Every page has `status:` and `verified_at:` in its front matter. A feature that isn't
   merged gets a stub page that says so, gives the interim pattern, and says "don't build your own".
2. **Facts are generated; prose explains intent.** Props, events, types, defaults, CSS properties, tokens and
   codes come from their source tables. Hand-written text covers when to use something, how parts combine,
   gotchas and mistakes.
3. **Every code block compiles in CI**, or is marked `<!-- illustrative -->`. A page may have at most one
   illustrative block, and recipes may have none.
4. **One answer per question.** Where two APIs could do a job, the page says which one to use and why. There
   are "use X, not Y" tables everywhere §2 shows a mistake.
5. **Task-shaped and short.** A page opens with when to use it and when not to. It stays under 300 lines
   (generated reference pages are exempt), and it ends with "Mistakes we have seen" when history has some.
6. **No lane voice in the body.** No milestone ids, no `§` references into `plans/`, no "Writer lane". Lane
   ids appear only in status columns and in an optional `History` footer.
7. **Predictable paths and front matter** (`tag:`, `kind:`, `api:`, `cases:`, `sources:`), so that both
   `grep -rl "tag: combo-box" docs/egui` and the generators work.
8. **Stability is visible.** Items listed in ARCH-SCENE §16 as unstable (taffy types, `Dirty`, `SceneStats`,
   the dump format) carry an "unstable" badge. Internal items are never documented as API.

## 5. Information architecture (page level)

Columns: **Gen**: `full` (generated), `part` (generated regions plus hand-written text), `hand`. **Feature**:
the state of what the page documents. A page is written when the feature is available; otherwise it's a status
stub. **Sources** are the facts the writer works from.

### 5.0 Top level

| Page | Covers | Gen | Feature | Sources |
|---|---|---|---|---|
| `START-HERE.md` | §3.2 item 1 | hand | — | this plan, §2 |
| `llms.txt` | index | full | — | front matter |
| `status.md` | SV/SVP and feature status with shas, interim patterns | hand (coordinator-kept) | — | STATUS, CADATOMIC.md §2, milestones |
| `troubleshooting.md` | symptom → cause → fix (§5.9) | hand | — | §2, milestone "limits", issues seen |
| `glossary.md` | slot, root, embed, kind, behaviour, provider, anchor, pass, logic, phase, observe class, token, … | hand | — | ARCH-EMBED, ARCH-SCENE |

### 5.1 Concepts (`concepts/`)

| Page | Covers | Gen | Feature | Sources |
|---|---|---|---|---|
| `architecture.md` | the retained scene, host ops, the kind table, taffy, the walk, the CSS engine, QuickJS host and loader; one diagram | hand | available | `lib.rs`, ARCH-SCENE §0/§4, EGUI.md §1 |
| `frame-lifecycle.md` | phases (Logic/Ui/Idle), what may run where, the event queue, repaint and 0-frame idle, `settled` | hand | available | `embed/mod.rs` `logic`, ARCH-SCENE §5.2/§7.2, ARCH-EMBED §5 |
| `identity.md` | slot keys, root keys, `anchor`, egui ids, AccessKit author ids, UI-state ids, HMR identity; why indexes break things | hand | available | `ids.rs`, ARCH-EMBED §4.4, DX1 |
| `stability.md` | stable/unstable/internal, the pin cycle, deprecation, `#[non_exhaustive]` | hand | available | ARCH-SCENE §16 |
| `threading.md` | UI thread only, `Send` rules, offload, `BusyProbe` across threads, ZU3 enforcement (in progress) | hand | available / ZU3 in progress | `svelters` lib docs (offload), ARCH-EMBED §6.1 |

### 5.2 Rust integration (`rust/`)

| Page | Covers | Gen | Feature | Sources |
|---|---|---|---|---|
| `setup.md` | Cargo deps and features (`eframe`, `kittest`, `source`, `dialogs`; SVP7 `default-features = false`), profile overrides (SVP8), **`link()` and `inventory`**, `cargo fmt` and path deps, licences pulled in, the UI build (Vite or source loader), dump staleness test | hand | available | `Cargo.toml` of svelters-egui, `examples/egui-kinds`, cadatomic `build-ui.sh` |
| `versions.md` | egui `=0.37.0`, taffy `=0.15.0`, egui_extras/egui_table same-week rule; use `svelters_egui::{egui, taffy, egui_extras}`; upgrades go together with cadatomic | hand | available | `Cargo.toml` comments, CLAUDE.md decision |
| `embed.md` | `Embed::new`, `EmbedConfig` (salt, budgets, faults, `timer_horizon`, `prepass_plugin`), `load`/`load_from`/`load_app`, `EmbedError` | hand | available | `embed/mod.rs` |
| `frame-order.md` | the per-frame contract, `prepass_plugin` true vs false, deferred viewports, `show_viewport_passes`, `take_events`/`take_warnings`, stats | hand | available | `embed/mod.rs`, `current.rs`, EM2 |
| `slots-and-roots.md` | `create_slot`/`SlotOptions`/`FaultUi`, `mount`/`MountSpec`/`Props`, `set_root_props`, `unmount`, `request_close`→`CloseDecided`, `restart_root`, `move_root`, `RootInfo`, `EmbedEvent`, faults and `FaultPolicy` | hand | available | `embed/{mod,roots,slots}.rs` |
| `viewports-and-windows.md` | `set_viewport`, `SlotPainter`, `with_current`, the `window` kind under the desktop runner, `show_windows` | hand | available | EM2, `windows.rs` |
| `data-across-the-bridge.md` | **one table of what crosses** (scalars, `Record`, `Enum` incl. tagged, `Vec`/`Option`/maps, `Json<T>`/`JsonAny<T>`, `Buffer<T>`/`Shared<[T]>`, objects, `Obj<T>`); errors with paths; stores (`writable`, `proportional`), services, `Emitter`, `Stream`/`Sink`; **app state patterns** (object in mount props vs store vs service; no thread-locals); "never JSON text" | hand | available | `crates/svelters/src/lib.rs:1-150`, E0–E3, RA2, SV48 |
| `loading-code.md` | `BundleProvider` vs `SourceProvider` (live compile in release), TS entries and the bytecode cache, loader hooks, native module providers, `svelters:app/*` | hand | available | `svelters-modules` docs, Q1, Q2, LD1 |
| `hot-reload.md` | what survives (Rust services, stores, `persist`), H1/H3 behaviour as seen from Rust, full-reload fallback | hand | available | H1, H3, HMR.md (summary only) |
| `custom-kinds.md` | `#[svelters::widget]`, `#[prop]`/`#[prop(bind)]`/`#[event]`/`#[state]`, `Caps`, `Widget` (`measure`, `ui`, `a11y`, `layout`, `prepare`, `hit_test`, `consume_key`, `composing`), `WidgetCx`, the dump (`kinds.json`, `kinds.d.ts`), linking; methods and fn form | part (generated: `Caps` names, cx methods list) | available; methods, fn form and slots are RA3 (in progress) | `provider/mod.rs`, RA1, `examples/egui-kinds` |
| `behaviours.md` | `#[svelters::behavior]`, `Behavior` (`sense`, `after_ui`), `BehaviorCx`, host state, the `svelters:behaviors/<ns>` module; event filters (EV1, in progress) | hand | available / EV1 in progress | `provider/behavior.rs`, EV2b |
| `states-and-providers.md` | built-in bits ↔ CSS pseudo-classes (generated table), `state_bits!`, `WidgetCx::set_state`, `StateProvider`, `:state(ns.name)` | part | available | `provider/state.rs`, SG4, CS5e |
| `scene-access.md` | `SceneHandle` read/write/defer, `NodeRef`/`NodeMut`, `Query`, anchors and paths, `Cursor`/`visit`, the journal (`ChangeFilter`, `changed_since`, `Damage`), `ExtKey` columns, `hit_test`; phases and what fails when | hand | available | `scene/*.rs`, SG1–SG3, ARCH-SCENE §6 |
| `scene-passes-and-overlays.md` | `ScenePass`, `register_pass`, `show_viewport_passes`, overlay roots (`create_overlay`/`show_overlay`), `position: absolute`; SV7 overlay trait (EV1, in progress) | hand | available / part in progress | SG4, `provider/pass.rs`, `embed/mod.rs:1082-1128` |
| `virtual-lists-and-row-models.md` | `virtual-list` from Rust's side, `VirtualChildren`, `RowPool`, `RowSource`, `RowHeights`; row-model trait (LS2, planned) | hand | available | LS1-SG5, `scene/virt.rs` |
| `theme.md` | `Theme::vscode()` / dark/light/HC, densities, `with` overrides, `EguiBackend::set_theme` at runtime, `Theme::apply_to` for egui-native Rust UI, typed getters, `--cad-*` passthrough, `set_tokens` | part (token tables generated) | available | `theme/*`, DS1, DESIGN.md §4 |
| `fonts-and-icons.md` | `register_weight`, `register_icons`, `check_icons`, `FontWarning`; semantic icon ids (DS5/IC1/AS1, in progress) | hand | available / in progress | `fonts.rs`, ICONS.md §5 |
| `focus-and-keys.md` | `focus_info`, `shortcut_override`, `text_command`/`TextCommand`, `composing`; integrating a host keymap (cadatomic's layers); focus across viewports | hand | available | `focus.rs`, EV3 |
| `clipboard-dnd-dialogs.md` | `platform::system()`/headless, the clipboard backend, `DragPayload` between Svelte and Rust panels, `DialogBackend`/`set_dialog_backend` | hand | available | `platform.rs`, `kinds/dnd.rs`, `dialogs.rs`, DX2, WG4b |
| `settings-and-ui-state.md` | `Dx`, `SettingsBackend`, `UiStateBackend`, `Paths`, console→`tracing` (`js` target, category), crash context | hand | available | DX1 |
| `accessibility.md` | names, roles, author ids, `invalid`, slot groups; what kittest requires | hand | available | `a11y.rs` |
| `automation.md` | the automation protocol, the out-of-process e2e runner, the inspector | stub | in progress (AU1) | E2E-INSPECTOR.md |
| `assets.md` | `Asset` handles, the registry, `asset://`, SVG, reload | stub | in progress (AS1) | ASSETS.md |
| `zoom.md` | host zoom API, relayout, SVG eviction | stub | planned (ZM1) | STATUS |
| `testing.md` | `EmbedHarness` / `TestApp`, `run_until_settled`, querying by label / anchor / author id, kittest input, svelters-testkit cases on the app's own directory, snapshots, SCHED1 (in progress) | hand | available | `runner/kittest.rs`, TK1, `svelters-testkit` |
| `performance.md` | the rules: no JS per frame or per row; 0-frame idle; handler budgets; relayout boundaries (definite sizes); virtual lists; culling and `box-shadow` bounds; measuring (`EmbedStats`, `SceneStats`, frame histogram ZU3) | hand | available | ARCH-SCENE §0.8/§8, CS5g, LS1 |

### 5.3 Svelte side (`svelte/`)

| Page | Covers | Gen | Feature | Sources |
|---|---|---|---|---|
| `project-setup.md` | `package.json` `"svelters": {toolkit: "egui", specFragments, embed}`, `svelteConfig`, embed bundles, tsconfig `types/egui` + `.svelters/app/*.d.ts`, svelte-check, the source loader without Vite | hand | available | `examples/egui-kinds/app`, `fixtures/embed-egui` |
| `language-support.md` | Svelte 5 features that work, what is rejected (with codes), the conformance score | part (rejection list from `conformance/expected.json`) | available | CF1, `docs/site/.../svelte-support.md` |
| `layout.md` | elements are flex boxes (taffy); `box`, `grid`, `stack`, `sides`, `scroll-area`, `split-view`; layout as attributes or as CSS; points; `%`/`calc()`; `position: absolute` | hand | available | `kinds/builtin.rs`, CS5e |
| `props.md` | kebab-case, `null`/`undefined` resets, "an explicit prop beats CSS", spreads, text content rules | hand | available | CSS.md §2.8, SP1, codes `text_not_allowed` |
| `events.md` | native names (`onclicked`), handler arguments, sync handlers (`close-request`), queued delivery, `on:` forwarding, component callback props; capture and bubble (EV1, in progress) | hand | available / EV1 in progress | site `events.md`, FW1, EV1 |
| `bindings.md` | which props are bindable (generated list), observe classes, `bind:this` + `call()`, `\|nonreactive`, function bindings, `\|commit` and drafts (CX3, in progress) | part | available / CX3 in progress | R2, site `bindings.md` |
| `components.md` | props, snippets, **reserved names** (`slot`, …), **controlled props on self-toggling elements**, forwarding | hand | available | §2 items 5–6, FW1 |
| `behaviours.md` | `use:` with `svelters:behaviors/<ns>`, params, events, typing | hand | available | EV2b |
| `menus.md` | `menu-bar`, `menu-button`, `menu`, `action`, `menu-separator`, `context-menu` (attaches to its parent), focus return | hand | available | WG2a, `menus.rs` |
| `overlays.md` | `popup`, `modal`, `inner-window`, `toast-host`; dialog focus scopes | hand | available | WG2a, WG4a |
| `focus-and-keys.md` | `focus-scope`, `autofocus`, `default`/`cancel` buttons, `request-focus`, keys a field claims | hand | available | EV3, `focus.rs` |
| `lists-and-tables.md` | keyed `{#each}`, `virtual-list`, `table`/`table-column`, selection, keyboard nav, type-ahead, cell editors; tree-view (WG5b, in progress) | hand | available / WG5b in progress | LS1, WG4a, WG5a |
| `drag-and-drop.md` | `drag-type`/`drag-data`/`drop-types`, the events, interplay with Rust panels | hand | available | WG4b |
| `dialogs.md` | `openFile`/`saveFile`/`pickFolder`/`messageBox` from `svelters:app/egui`, abort | hand | available | `dialogs.rs` |
| `embed-root.md` | `root()` from `svelters:embed`: `title`, `dirty`, `visible`, `scale`, `emit`, `onCloseRequest` | part (from `embed.d.ts`) | available | EM1 |
| `modules.md` | catalogue of `svelters:*`: `core`, `embed`, `app/<module>`, `behaviors/<ns>`, `clipboard`, `shell`, `settings`, `paths`, `log`, `app/egui` | part (from `.d.ts`) | available (DX1 typings missing, §1) | `packages/native-runtime/types`, host |
| `typescript.md` | `lang="ts"`, `.svelte.ts`/`.ts` entries, what Q2 rejects (enums, namespaces, decorators), svelte-check | hand | available | Q2 |
| `hot-reload.md` | what is kept (`$state` per instance, signature reset, `// @hmr:reset`, module state), what resets, when it falls back to a full reload, limits | hand | available | H1, H1b, H3, HMR.md |
| `transitions.md` | `transition:`/`in:`/`out:`, the native animator, `tick` | hand | available | TR1–TR3 |
| `accessibility.md` | `accessible-name`, `anchor`, `invalid`, labels; what kittest rejects | hand | available | `a11y.rs`, WG1 |

### 5.4 Widget reference (`reference/widgets/`)

One page per tag (45), plus `widget.md` (the `Widget` base: 26 props and the drag-and-drop events) and
`menu-entry.md`, plus a generated `index.md` grouped by `category`. Template: the sample
`docs-samples/reference-text-edit.md`.

| Section | Gen | From |
|---|---|---|
| Title, summary, native type, base, children/slot rules, text content | full | `egui.json` `doc`, `native`, `chain`, `children`, `text_content` |
| When to use / alternatives | hand | — |
| Example (a testkit case, run in CI) | hand (checked) | `docs/egui/cases/<tag>-*.svelte` |
| Props: name, type, default, nullable, `bind:` (observe class), doc | full | `props`, `bindings` |
| Events: attribute, handler signature, doc; sync flag | full | `signals` |
| Methods | full | `methods` |
| Behaviour you need to know | hand | milestone limits, tests, native refs |
| CSS: selectors, which properties apply, theme defaults | full | `style` section + **new** `paint` field + `theme/base.css` rules |
| States: pseudo-classes the kind sets and their source | full | **new** `states` field |
| Accessibility: role, name source, value, actions | full | **new** `a11y` field |
| Testing: how to find and drive it in kittest | hand | — |
| See also | hand | — |

The same generator runs on any **spec fragment** (`kinds.json` from `svelters_egui::dump`). cadatomic generates
pages for its own kinds (`quantity-field`, `code-editor`) into its own docs, in the same format, for free.

### 5.5 CSS (`css/`)

| Page | Covers | Gen | Sources |
|---|---|---|---|
| `overview.md` | the engine runs in process on the scene; origins and layers (`sv.tokens`, `sv.base`, `sv.affordance`); precedence (explicit prop > inline > rules > default); scoping; `px` = point; nothing runs until the first style input | hand | `style.rs` module doc, CSS.md §2 |
| `properties.md` | **matrix:** property × value type × setter/engine × tags it applies to; egui first, then GTK, Qt, AppKit, WinUI columns | full | `svelters_style::tables`, `ENGINE_PROPS`, `metadata/style/*.yaml` |
| `selectors.md` | supported selectors, combinators, pseudo-classes, attribute selectors (they read props), `:global`, `:state()`; what the parser drops (`style_syntax`) | part | `svelters-style` `selector.rs`, `STATE_PSEUDO_CLASSES` |
| `states.md` | pseudo-class ↔ state bit ↔ who sets it; provider states; stock widgets (egui's three visuals) vs box-painted kinds | part | `style.rs:599-680`, `provider/state.rs` |
| `tokens.md` | the L1/L2/L3 token tables (`--vscode-*`, `--sv-<role>`, `--sv-<kind>-<part>`), overriding from CSS and Rust, dark/light/HC, `@media (prefers-color-scheme)` | part (tables generated) | `theme/mod.rs` `ROLES`/`COMPONENTS` |
| `compile-time-checks.md` | what the compiler reports for egui `<style>` (`style_bad_value`, `style_unknown_declaration`, `style_syntax`, `style_unsupported_at_rule`) and what is runtime only | hand | CS5g |
| `unsupported.md` | deferrals with workarounds (fonts in measure/paint until CS5f, outline, cursor, per-side borders, transitions of CSS properties, …) | part (from a deferrals table the CSS lanes keep) | CS5e/CS5f/CS5g "Deferred" |
| `scoping.md` | Svelte scoping on egui, `class:`, `style:`, `style=`, `:global` | hand | Q3/Q3w |
| `cookbook.md` | invalid outline through `:state(cad.invalid)`, a form grid, a toolbar row, density, dark-only tweaks, a selected row | hand (cases checked) | §2 item 4 |

### 5.6 Icons (`icons.md`, one page)

What works today: `<icon name="gear" family?>` over a font family the embedder registers (`fonts::register_icons`),
`frame`, `tooltip`, and the placeholder box on errors. What is being designed: semantic ids (`mcad.extrude`), the
per-theme id table with a fallback chain, SVG through AS1, `icons.d.ts` id union (DS5, IC1, AS1; cadatomic owns the
set). The page states plainly which half is which, and it is rewritten by DS5's lane when that lands.

### 5.7 Recipes (`recipes/`)

Each recipe is complete: Rust snippets from `examples/egui-docs`, Svelte from a case, a "check it" section naming
the test, and "mistakes we have seen".

| Recipe | Shows | Feature |
|---|---|---|
| `panel-in-dock-pane.md` (**sample written**) | berth pane ↔ slot/root, lazy mount, title/dirty, close guard, viewports, faults | available |
| `task-panel.md` | a form on a Rust store of records and tagged enums, validation, commit, no JSON text | available (`\|commit` later) |
| `list-with-rust-model.md` | 10k rows from Rust: store + `virtual-list`, selection, keyboard, no per-row JS | available |
| `property-grid.md` | 2k-row grid with cell editors (ARCH-SCENE §10.2) | available (WG5a) |
| `custom-kind.md` | a kind end to end: struct, `Widget`, dump, typings, link, test | available |
| `native-behaviour.md` | `use:pick` end to end | available |
| `invalid-field.md` | state bits + `:state()` + tokens instead of painting | available |
| `modal-dialog.md` | `modal` with focus scope, default/cancel, result back to Rust | available |
| `context-menu-on-row.md` | `context-menu` on a list row, actions to Rust | available |
| `theme-switch.md` | runtime dark/light/HC and `--cad-*` tokens shared with Rust UI | available |
| `test-a-panel.md` | `EmbedHarness`, settle, anchors, a testkit case for the same panel | available |
| `extension-panel.md` | a second bundle loaded at runtime into the same embed, its own roots, fault isolation; "security policies are deferred" note | available (isolates SV15 not built) |

### 5.8 Diagnostics (`diagnostics/`)

| Page | Covers | Gen |
|---|---|---|
| `index.md` | every code: id, severity, summary, fix-it, backends; grouped by area; egui-relevant first | full (extends the `cx1_codes` generator) |
| explanations | rustc-style long text per `svelters::*` code (62): why, a failing example, the fix; printed by `--explain` too | part (§6.2) |
| `runtime.md` | runtime errors and warnings: `EmbedError`, `RootError`/`FaultCause`, `SlotWarning`, `FontWarning`, `NativeError` codes (`type`, `range`, `missing-field`, `unknown-variant`, `cancelled`, `disposed`, `superseded`, `panic`, `unsupported`), `settings/*` | hand now, generated once a runtime registry exists |

### 5.9 Troubleshooting (`troubleshooting.md`)

Symptom → cause → fix, seeded from §2 and the milestone limits:

- nothing draws (`logic` never called, slot hidden, wrong viewport);
- the panel updates one frame late;
- `unknown_event` for `onclick`;
- `unknown_element` for your own kind (spec fragment not passed, dump stale, kind not linked);
- duplicate kind ids;
- two eguis in the tree;
- the `id_salt` debug assertion;
- a root faulted (budget interrupt, render error);
- a glyph placeholder box;
- a style rule with no effect (stock widget, unsupported property, an explicit prop wins);
- `bind:` not updating (prop not exact);
- types missing in svelte-check;
- HMR dropped state;
- kittest "input without a name";
- a test that never settles (`NotSettled.chain`).

### 5.10 Contributor pages (`contributing/`)

- `CONVENTIONS.md`: page templates (widget, guide, recipe, stub), front matter keys, status words, snippet and
  generated-region markers, voice rules (§4).
- `OWNERS.toml`: source globs → pages (§6.4).
- `eval.md`: the agent evaluation tasks (§7.3).

## 6. Generation and checking

### 6.1 Metadata additions (lane DOC1)

`spec.rs` emits four new fields per kind in `egui.json`, from fields in the kind tables (`kinds/*.rs`), and
`#[svelters::widget(...)]` attributes for provider kinds:

| Field | Values | Used by |
|---|---|---|
| `category` | `inputs`, `buttons`, `display`, `containers`, `layout`, `views`, `menus`, `overlays`, `pickers`, `media` | the reference index, the sidebar |
| `states` | built-in state names the kind sets, each with `source: response \| prop \| kind` | the States section, `css/states.md` |
| `a11y` | `{ role, name_from: [...], value, actions }`; `role: "egui"` when egui sets it (with the egui role names) | the Accessibility section |
| `paint` | `stock` (egui's `WidgetVisuals`), `box` (svelters paints), `provider`, `none` | the CSS section: which properties apply |

The `tests/spec.rs` freshness test already guards `egui.json`, so a kind change without regeneration fails CI.
These are additive fields; the compiler ignores them.

### 6.2 Generators

| Output | Generator | Inputs | Check |
|---|---|---|---|
| `reference/widgets/*.md` regions, `reference/widgets/index.md` | `tools/docs/egui/gen_reference.py` (Python, beside `tools/docs/*`) | `egui.json`, `theme/base.css`, optional `--fragment kinds.json` | `--check` in `pnpm docs:test` (js.yml). A new tag gets a stub page with hand sections marked TODO |
| `css/properties.md`, `css/selectors.md` tables | same generator | `egui.json` `style` (+ new `style.pseudo_classes`, exported from `svelters_style::STATE_PSEUDO_CLASSES` through `spec.rs`), `packages/style/src/props.generated.js` for the other backends | same |
| `css/tokens.md`, `rust/theme.md` tables | a Rust test in svelters-egui with `UPDATE=1` (as `cx1_codes`), writing a `theme` section into `egui.json` | `theme::ROLES`, `COMPONENTS`, the four built-in sets | the test fails when stale |
| `diagnostics/index.md` | extend `codes::docs_markdown` with a second target | `codes.rs` | `cx1_codes` |
| explanations | `crates/svelters-compiler/src/codes/explain/<code>.md`, `include_str!`ed by `explain()` | hand-written | **trigger test**: each file's ` ```svelte fail` block must report that code and its ` ```svelte ok` block must not |
| `svelte/modules.md` region | `tools/docs/egui/gen_modules.mjs` | `packages/native-runtime/types/*.d.ts`, the dumped `app.d.ts` of `svelters:app/egui` | `--check` in `docs:test` |
| `llms.txt` | `tools/docs/egui/gen_index.py` | front matter | `--check` |
| CLI pack `egui.svpack` | `tools/docs/egui/frontend.py` → docs IR → `svelters docs-pack` | `egui.json` + page hand sections as `description` | the CL0 pack/site consistency test |
| `examples/egui-docs/tests/doc_api.rs` | `tools/docs/egui/gen_api_check.py` | `api:` front matter | `--check`, then compiled in linux-gtk's nextest |

Managed regions: a generator rewrites only text between `<!-- BEGIN GENERATED: … -->` and
`<!-- END GENERATED -->`, and never touches the rest. This is why one file can hold both.

### 6.3 Doc tests and staleness

1. **Rust snippets.** `examples/egui-docs` is a workspace crate whose `src/*.rs` and `tests/*.rs` hold every Rust
   block that pages show, between `// region:name` and `// endregion` markers. A page carries a literal copy
   after `<!-- snippet: examples/egui-docs/src/dock_pane.rs#adapter -->`. `tools/docs/check_snippets.py --check`
   (in `docs:test`) compares them after normalising indentation, and `--fix` rewrites the page. Agents read real
   code in the page, and CI compiles the source. The crate's tests run in linux-gtk's nextest (headless kittest).
2. **Svelte snippets.** `docs/egui/cases/*.svelte` are testkit cases (`@case`, `@toolkits egui`, `@test` steps).
   linux-gtk's egui runner gets `docs/egui/cases` as a second case directory with its own ratchet entry. A page
   embeds them with the same snippet marker, and the `@test` comment is stripped in the copy.
3. **rustdoc.** A new step in linux-gtk:
   `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" cargo doc --no-deps -p svelters-egui -p svelters -p svelters-host -p svelters-modules -p svelters-style`
   and `cargo test --doc` for the same crates. nextest doesn't run doctests, so they need this step. The 12
   `ignore` blocks in svelters-egui (and the ones in `svelters`) become `no_run`, compiled against the crate, or
   move to `examples/egui-docs` and are linked.
4. **Symbol check.** `doc_api.rs` is generated from every page's `api:` list as `use` lines and
   `let _ = svelters_egui::Embed::show_slot;` per item. A rename or removal of a documented item fails the build,
   and the error names the item.
5. **Generated freshness.** Every generator in §6.2 has `--check`, run by `pnpm docs:test` (js.yml), plus the
   existing `tests/spec.rs` and `cx1_codes`. The chain is: a kind change → `egui.json` stale (spec test) →
   reference pages stale (`gen_reference --check`).
6. **Status markers.** `tools/docs/check_status.py` reads each page's `status: in progress (LANE)` and STATUS.md's
   table. It fails when the lane is `DONE` but the page still says in progress, and warns when an `available`
   page's `verified_at` is more than 200 commits old on a file its `sources:` lists as changed since then.
7. **`verify` markers.** `<!-- verify: … -->` is allowed only in drafts. `check_status.py` fails on it in any page
   whose status is `available`.

### 6.4 Keeping it current: the owner map

`docs/egui/contributing/OWNERS.toml` maps source globs to pages:

```toml
"crates/svelters-egui/src/embed/**"   = ["rust/embed.md", "rust/slots-and-roots.md", "rust/frame-order.md"]
"crates/svelters-egui/src/focus.rs"   = ["rust/focus-and-keys.md", "svelte/focus-and-keys.md"]
"crates/svelters-egui/src/kinds/**"   = ["reference/widgets/*"]      # regenerated, plus hand notes
"crates/svelters/src/lib.rs"          = ["rust/data-across-the-bridge.md"]
"packages/native-runtime/types/**"    = ["svelte/modules.md"]
```

`tools/docs/check_doc_touch.py origin/master...HEAD`, run in js.yml, does three things:

- it lists changed files whose diff adds or removes a `pub` item, a kind table row or a `.d.ts` export;
- it looks up the pages the owner map gives for them;
- if none of those pages changed and no commit carries a `Docs: none (<reason>)` trailer, it fails.

It warns for the first two weeks, then fails (Q9).

## 7. Lanes

### 7.1 The lanes

"Docs" lanes build nothing (CLAUDE.md). They push, and CI compiles their snippets and cases. "Tool" lanes are
code lanes under the usual build rules. Sizes are lane-days.

| Lane | Type | Content | Size | Needs | Machine |
|---|---|---|---|---|---|
| **DOC0** skeleton | docs (+ one small Python script) | `docs/egui/` tree with **every §5 page as a stub**: front matter, status, 3–10 lines pointing at the current source of truth, so the map exists on day 1. START-HERE (from the sample), `status.md` (all SV/SVP rows with shas), `CONVENTIONS.md`, `gen_index.py` + `llms.txt`. Design-record banners on the research docs. The stale fixes of §1 (MATRIX line 15, `menus.rs:10-11`, `egui-kinds/src/lib.rs:5`, `embed.d.ts:5`; comment-only). The CLAUDE.md rule (§9). The cadatomic snippet sent to main-cadatomic | 1 | — | macbook |
| **DOC1** reference generator | tool | §6.1 fields; `gen_reference.py` with `--check`; the 47 generated pages (hand sections TODO); `--fragment` mode; egui IR frontend and `svelters docs egui`; tests in `tools/docs/test_ir.py` | 2 | — | thinkpad |
| **DOC4** doc-test machinery | tool | `examples/egui-docs` crate, `check_snippets.py`, `docs/egui/cases` in the egui runner, the rustdoc CI step, `cargo test --doc`, `ignore`→`no_run` in svelters-egui, `gen_api_check.py` + `doc_api.rs` | 1.5 | — | WSL |
| **DOC2** Rust integration core | docs | `rust/{setup, versions, embed, frame-order, slots-and-roots, viewports-and-windows, data-across-the-bridge, focus-and-keys, testing, settings-and-ui-state, clipboard-dnd-dialogs, accessibility, performance}`, `concepts/*`, recipes `panel-in-dock-pane` (from the sample) and `task-panel`, troubleshooting v1. **Order inside the lane:** data-across-the-bridge, frame-order, setup, testing first | 2.5 | DOC0, DOC4 (snippet markers) | WSL |
| **DOC3** Svelte side | docs | all of §5.3. **Order:** project-setup, events, bindings, components, layout first | 2 | DOC0 | macbook |
| **DOC5** CSS | tool + docs | the pseudo-class and theme exports (§6.2), generated `css/{properties, selectors, tokens}` tables, hand pages `overview, states, compile-time-checks, unsupported, scoping, cookbook`, `recipes/invalid-field` | 1.5 | DOC1 | thinkpad |
| **DOC6** Rust extension surface | docs | `rust/{custom-kinds, behaviours, states-and-providers, scene-access, scene-passes-and-overlays, virtual-lists-and-row-models, theme, fonts-and-icons, loading-code, hot-reload}`; rustdoc module docs of `provider/*` and `scene/*` moved to consumer voice (lane ids into an `# Internals` footer). After RA3 merges, or with RA3's parts as in-progress stubs | 2 | DOC0, DOC4 | WSL |
| **DOC7a** widget notes, half 1 | docs (+ kittest checks for `verify` rows) | hand sections and example cases for inputs, buttons, display, pickers, media (about 22 tags) | 1.5 | DOC1, DOC4 | macbook |
| **DOC7b** widget notes, half 2 | same | containers, layout, views, menus, overlays (about 23 tags + 2 bases) | 1.5 | DOC1, DOC4 | thinkpad |
| **DOC8** recipes | docs | the other 9 recipes of §5.7 | 2 | DOC2, DOC6 | macbook |
| **DOC9** diagnostics | tool + docs | explanations for the 62 `svelters::*` codes with trigger tests, `--explain` output, `diagnostics/index.md`, `diagnostics/runtime.md` | 1.5 | DOC0 | WSL |
| **DOC10** upkeep | tool | `OWNERS.toml`, `check_doc_touch.py`, `check_status.py`, milestone template field "Docs touched" | 0.5 | DOC0 | thinkpad |
| **DOC11** site | tool | Starlight renders `docs/egui` (content loader pointed at it), sidebar from categories, egui out of `PLANNED_TOOLKITS`, link check over `docs/egui` | 0.5 | DOC0 | thinkpad |
| **DOC-EVAL** ×2 | docs | §7.3, after waves 2 and 4 | 0.5 + 0.5 | — | any |

Total: about 21 lane-days.

### 7.2 Waves

| Wave | WSL | thinkpad | macbook | Done when |
|---|---|---|---|---|
| 1 (day 1–2) | DOC4 | DOC1 | **DOC0** (merged first: day 1) | the map, START-HERE and status exist; the generators and checks are in CI |
| 2 (day 2–4) | DOC2 | DOC5 | DOC3 | an agent can host a panel, pass data, style it and test it from the docs alone; **DOC-EVAL 1** |
| 3 (day 4–6) | DOC6 | DOC7b | DOC7a | every tag page is complete; the extension surface is documented |
| 4 (day 6–7) | DOC9 | DOC10 + DOC11 | DOC8 | the recipes, diagnostics and upkeep checks are enforced; **DOC-EVAL 2** |

Doc lanes are light: the docs lanes build nothing, and the tool lanes build tests for one crate. They share
slots with the feature lanes already queued on each machine (FM1 and SG6 on the thinkpad, RA3 on the macbook)
without starving them. Feature lanes that are in flight (EV1, CX3, RA3, AS1, AU1, DS5/IC1, SCHED1, WG5b, FX3,
ZU3) and queued ones (ZM1, FM1) don't get doc lanes of their own. Each writes or flips its pages under the §9
rule, and DOC0's stubs tell them which pages are theirs.

### 7.3 Acceptance criteria

Each page, before its status leaves `stub`:

1. Its front matter is complete (`title`, `description`, `status`, `verified_at`, `sources`, and `api`/`cases`/`snippets` where it has code).
2. Every code block is a checked snippet, or the page's single `illustrative` block (recipes have none).
3. There is no lane voice in the body (§4.6), the page is ≤ 300 lines (reference pages are exempt), it opens with when-to-use, and it has a "Mistakes" section where §2 or the milestones record one.
4. There are no `verify` markers, and the links resolve.
5. The pages its lane touched are listed in the lane's milestone record.

Per lane, its CI is green with the new checks on. The checks are proven by a deliberate failure in the lane's
own test: a stale page, a drifted snippet, or a removed API item.

**DOC-EVAL.** A fresh agent with read access to `docs/egui/` only (not `plans/`, not the crates) answers
twelve tasks and writes the code sketch for each. A second fresh agent grades it against
`contributing/eval.md`'s keys. The tasks:

1. Mount a panel in a pane and show its dirty mark.
2. Pass a list of records with a tagged enum field to Svelte.
3. Show a custom kind's invalid state with CSS.
4. Keep Ctrl+S from firing while a field has focus.
5. Write a test that waits for settle and finds a field by anchor.
6. What does `combo-box` take?
7. Why is `onclick` an error?
8. Add a `use:` behaviour.
9. Show 10k rows from Rust.
10. Does capture/bubble exist at this sha, and what should I do meanwhile?
11. Override the panel background token for one panel.
12. My kind needs a `revealLine` method: what now?

The pass bar is 11/12, with zero answers that build their own version of an existing feature. Each failure is
fixed in the page and recorded in `eval.md`.

## 8. Samples

Written in full under `plans/research/docs-samples/`, checked against `424ebcf4` (each file's header says what
is final and what isn't):

| Sample | Final path | Shows |
|---|---|---|
| `START-HERE.md` | `docs/egui/START-HERE.md` | the entry page: model, frame contract, do/don't from §2, the "already exists" table, the task map, status words |
| `reference-text-edit.md` | `docs/egui/reference/widgets/text-edit.md` | the widget template: generated regions (props, events, methods, CSS, states, a11y) around hand sections (when to use, behaviour, testing). It also shows a `verify` marker on the one row that wasn't checked: `background-color` on a stock text field. The States and Accessibility regions need DOC1's new fields; their content was taken from `style.rs:614-680` and egui 0.37's `text_edit/builder.rs:1117-1125` |
| `recipe-panel-in-dock-pane.md` | `docs/egui/recipes/panel-in-dock-pane.md` | the recipe template: the event table, the Rust adapter (from `runner/dock.rs`), the frame, the Svelte panel (from `fixtures/embed-egui`), faults, "check it", real `SlotWarning`s as mistakes |

## 9. Who keeps the docs current: the rule for CLAUDE.md

Proposed text for svelters' `CLAUDE.md`, a new section after "Before reporting a lane done" (DOC0 adds it once
the coordinator agrees):

```markdown
## Docs (egui)

`docs/egui/` is what cadatomic's agents read; keep it true in the same branch as the code.
- A lane that adds or changes a public Rust item, a kind (props, events, methods, states), a CSS property, a
  token, a diagnostic code or a `svelters:*` module updates the pages `docs/egui/contributing/OWNERS.toml` maps
  its files to: rerun the generators (`pnpm docs:egui`), edit the hand-written parts, flip `status:` from
  `in progress (LANE)` to `available` with `verified_at:` = the merge sha.
- New code in a page goes in `examples/egui-docs` or `docs/egui/cases` with a snippet marker, never inline only.
- The milestone record lists "Docs touched: <pages>" (or "Docs: none, <reason>"; the same reason goes in a
  `Docs: none (...)` commit trailer).
- Docs lanes build nothing; CI checks snippets, cases, generated pages and API names (`pnpm docs:test`).
- The coordinator updates `docs/egui/status.md` in each merge commit that delivers an SV/SVP request.
```

The coordinator's merge checklist gets one line: "docs touched or `Docs: none` trailer present".

## 10. Open questions (with recommendations)

**Decided (user, 2026-10-23, from cadatomic's tours research):** docs sites should use **SvelteKit**, because the
stack is Svelte-based. cadatomic's tutorial and docs site will be SvelteKit, prerendered with `adapter-static` and
Markdown through mdsvex. svelters' `docs/site/` is Astro Starlight today (D1, DOC11), so it is to be revisited under
the same direction. Whether to migrate it, and when, is the svelters coordinator's call. The source is
cadatomic `docs/research/2026-10-23-tours-tests-tutorials-videos.md` §7.4 and §11 Q8.


All of these are procedural, so svelters-agent decides them (CLAUDE.md "Who decides"). The exception is Q8.

| # | Question | Recommendation | Why |
|---|---|---|---|
| Q1 | Hand-written docs in `docs/egui/`, in the site's content dir, or rustdoc only? | **`docs/egui/`**, rendered by the site | a short path at the pin, greppable, no site noise. Rustdoc alone can't hold Svelte or recipes |
| Q2 | Commit the generated reference pages? | **Yes**, with `--check` in CI | agents read a pinned checkout without building; diffs show API changes in review |
| Q3 | Document in-progress features? | **Status stubs only**: "in progress (LANE), don't build your own, interim: …" | §2 items 2, 3 and 12: silence leads to reinvention |
| Q4 | Rust snippets: doctests, or an examples crate with snippet markers? | **Both:** rustdoc examples compiled (`no_run`), and page snippets from `examples/egui-docs` | page code must be complete programs; rustdoc stays item-local |
| Q5 | One egui doc set, or toolkit tabs on the existing GTK/Qt guides? | **A separate egui set** that links to shared concept pages | the readers are egui-only; GTK tags in examples mislead (`entry` vs `text-edit`). Merge later if other backends need it |
| Q6 | Move lane voice out of rustdoc module docs? | **Yes, gradually**: consumer text first, lane and milestone references in an `# Internals` footer, done by the lanes that own the files (DOC6 starts) | rustdoc is the reference cadatomic should use, and the first paragraph decides whether it's read |
| Q7 | `SvelteDock` is behind the `kittest` feature, and cadatomic builds with `default-features = false`. Promote it (a `dock` feature or always on)? | **Yes**: move it to `embed::dock` with no feature gate; it has no kittest dependency | cadatomic writes the same adapter by hand today; the recipe could then use it directly |
| Q8 | Publish the site (GitHub Pages)? | **Not now**; repo-only. This one is the user's (public) | CLAUDE.md: anything public goes to the user |
| Q9 | Doc-touch check: warn or fail? | **Warn for two weeks, then fail**; generated freshness fails from day 1 | the owner map needs tuning before it may block merges |
| Q10 | Should the DX1 module typings (`svelters:settings`, `paths`, `log`) and kind method typings (`call('request-focus')` untyped) be fixed by doc lanes? | **No**: doc lanes file them as follow-ups (a small DX1b, and RA3 for methods) | doc lanes stay docs-only; these are code |

## Sources

svelters (master `424ebcf4`):

- `crates/svelters-egui/src/{lib.rs, embed/*.rs, provider/*.rs, kinds/mod.rs, style.rs, theme/{mod.rs, base.css}, focus.rs, menus.rs, overlays.rs, windows.rs, dialogs.rs, a11y.rs, platform.rs, fonts.rs, kinds/dnd.rs, scene/mod.rs, runner/{mod,kittest,dock}.rs}`;
- `crates/svelters/src/lib.rs`, `crates/svelters-modules/src/lib.rs`, `crates/svelters-style/src/{lib.rs, state.rs, tables.generated.rs}`;
- `crates/svelters-compiler/src/codes.rs`, `tests/cx1_codes.rs`, `src/bin/svelters-compile.rs`;
- `crates/svelters-testkit/src/egui.rs`;
- `metadata/semantics/egui.json`;
- `packages/native-runtime/types/{egui,embed,embed-module,platform}.d.ts`;
- `docs/notes/docs-generator.md`, `docs/site/{site.config.mjs, src/content/docs/guides/*}`;
- `plans/milestones/{CL0, CS5g, DS1, DX1, H1, H3, TK1}`;
- `plans/research/{ARCH-EMBED (headings, §4), ARCH-SCENE (§16), ICONS (§0, §5), HMR (summary), CSS, DESIGN, E2E-INSPECTOR, ASSETS (headings)}`;
- `STATUS.md`, `MATRIX.md`;
- `.github/workflows/{js,linux-gtk}.yml`;
- `fixtures/{README.md, embed-egui, gallery-egui}`, `examples/egui-kinds`, `tests/cases`.

egui 0.37.0 (`~/.cargo/registry/.../egui-0.37.0/src/widgets/text_edit/builder.rs`), egui_kittest 0.37.0 (`node.rs:191`).

cadatomic, read only (master as of 2026-10-22, pin `a52d437c`):

- `CLAUDE.md`, `pins.toml`, `Cargo.toml`;
- `crates/cadatomic-app/{Cargo.toml, src/svelte/*, src/views/svelte_view.rs, src/shortcuts/mod.rs, src/app.rs, tests/{svelte_ui,task_forms_svelte}.rs}`;
- `ui/{package.json, vite.config.js, src/*}`;
- `scripts/{build-ui.sh, svelters-snapshot.sh}`;
- `docs/plan/{00-decisions, 04-kernel-requests, README}.md`;
- `docs/research/2026-10-22-{component-framework, scene-node-and-svelte-semantics}.md`;
- worktree `k1-code-editor` (`code_editor/core.rs`).
