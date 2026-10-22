# CADATOMIC: the svelters lane's response to cadatomic's requests

Status: research and roadmap proposal, 2026-10-21/22. Nothing here is implemented. This document is the svelters
lane's answer to cadatomic's integration research and its requests SV1–SV47. It is not a design of the integration:
cadatomic's GUI lane did that research and the user decided most of it. cadatomic stays read-only from this lane.

Read at:

- **cadatomic `9e73bf6`**:
  - `docs/plan/00-decisions.md`: "Product direction and UI framework (user, 2026-10-22)", "Shell answers", "Docking and
    windows answers" (D1–D11) and "Native extensions";
  - `docs/plan/04-kernel-requests.md`, "Requests to the svelters lane";
  - `docs/research/2026-10-22-*.md`. The svelters sections were read closely: component-framework §2, §5, §11–12;
    scene-node all; docking §4.3, §5.3, §6–§9; js-api-publishing §0, §4.2, §8; models §0, §6.3, §7–8; slint §0, §10;
    widget-survey §4.1, §5. The rest was skimmed.
- **svelters**:
  - `7aee41e5` (origin/master, this branch's base);
  - local `master` `07620d2b`, which merges CF1 and is not pushed. Where this document cites it, it says so;
  - in-flight branches `origin/ci/cf2` (`b323bae0`), `origin/ci/tr1` (`1156212a`) and `origin/ci/fu3` (`9f01e40f`).

## 0. Summary

- **svelters accepts the role cadatomic gives it.** svelters becomes the retained scene, the Svelte runtime and the
  compiler under cadatomic's component framework. Rust owns the dock and chrome; svelters renders panel, dialog,
  window and overlay contents as many roots. The cadatomic-specific parts (tokens, `Op` waiting states, typed
  picking, `command=`, draft and store bindings, `pick`/`shortcut`/`contextMenu`) live in cadatomic's own crate and
  plug in through extension points. All of SV1–SV47 are generic enough to belong in svelters. None is refused. Five
  get an alternative or an amendment (the Pushback column of §2).
- **One position matters before any code: C2, scene ownership.** The svelters lane's recommendation is that the scene stays in
  `svelters-egui`. It is generalised into a node registry that external crates extend (SV1, SV2, SV4, SV6, SV7), with a Rust
  node API so the dock and other Rust components create nodes without JS. The other direction is cadatomic's framework
  owning the scene, with svelters' kind table as an adapter onto its `Node`. That would split one retained tree, its layout,
  a11y, observation and op-log replay across two repositories. It would also leave svelters' egui backend unusable outside
  cadatomic.
- **Most of the list maps onto plans svelters already has.**
  - SV1/SV2 are E4 (`#[svelters::widget]`).
  - SV16/SV20/SV30/SV36 are EGUI D4 plus HMR.md H1–H4.
  - SV13/SV14/SV37 are TR1's next step.
  - SV11/SV4/SV5 are G2's styling item, which ST2's decl dialect can carry (§6 D4).
  - SV40–SV44 are G2's `virtuallist` plus M5/N3 open items.
  - SV22–SV27 extend E0–E5.
  - SV15/SV21 are QUICKJS-RUNTIME item 9 made real.
- **Some are already done or partly done.**
  - `mount(Component, { target })` into a host handle exists in `@svelters/native-runtime`
    (`packages/native-runtime/src/index.js:1308`), the JS half of SV17.
  - `use:` actions lower to attachments on local master (CF1, `07620d2b`; SV12).
  - Host-written props on egui can never change natively, so SV10 holds today by construction.
  - The compiler is already a library: `compile()`, with diagnostics carrying codes and spans (half of SV27).
  - `PlatformSemantics` with `Overlay` is the seam for SV8 and SV26.
- **The first milestone is SC1, a thin vertical slice.** It renders the task panel for `body.box` in Svelte, inside the
  running cadatomic app. It is driven by cadatomic's command session through E0/E3 exports, tested with kittest and
  visible to cadatomic's pixel tier. svelters builds SV17 (Rust-side mount into slots), the minimum of SV1 (a kind
  provider), the minimum of SV16 (stable ids), the G2 elements the form needs, and the embedding API changes
  cadatomic did not file (§2.8). cadatomic builds the host view, the exports, a QuantityField kind and the test.
- **Decided (user, 2026-10-22): no rquickjs determinism fork.** svelters stays on stock QuickJS-NG, and no
  determinism work is planned. cadatomic's decision 9 (and PB5, the `rquickjs` row of its fork sets) conflicts
  with this; the cadatomic lane updates its own register (§1.2).
- **Four conflicts need the user (§6).**
  1. `on:event` forwarding against runes-only.
  2. ST2's JS decl engine against the deferred Rust CSS engine.
  3. HMR's state rule and module delivery: the live IDE in release builds rules out Node at run time.
  4. The priority order.

## 1. cadatomic's architecture as it affects svelters

### 1.1 What svelters must be (decided by the user, 2026-10-22)

| Decision (cadatomic) | Consequence for svelters |
|---|---|
| Rust owns plumbing (dock tree, regions, tabs, toolbars, menus, windows). svelters renders contents of panels, dialogs, windows, overlays, with many roots (Shell answers, docking §0) | A runtime mounts N components into slots that Rust created. There need not be a toplevel `window` element (today the first `window` is the root viewport, `crates/svelters-egui/src/backend.rs:206`). The scene must take Rust-created nodes. |
| First-party Svelte runs in process on the UI thread (D4) | The G0/G1 embed mode, driven from cadatomic's `App::logic`/`App::ui` split. QuickJS time must fit a per-frame budget (docking §6.4: 8 ms warning, 50 ms interrupt). |
| Third-party roots run in QuickJS isolates off the UI thread and send op batches (direction, framework §5.1) | A proxy backend, a message-driven isolate host and an isolate compiler target (SV15, SV21). It is later in cadatomic's own order (framework §12 step 6). |
| `$state` kept across hot reload (D9); hidden roots snapshot and restore with the same machinery (D5) | HMR H1 and H3 are no longer optional. The identity prop (EGUI D4) becomes the key for node state and `$state`. |
| Secondary OS windows are full shells; panes move between windows with no remount (D1, U1) | Reparenting a root across viewports (SV18). It needs deferred viewports that reach the non-`Send` scene through a thread-local (cadatomic experiment W2). |
| The framework is its own crate in cadatomic, on svelters extension points (S6, C1) | Extension points, not cadatomic code: SV1–SV8. |
| Native extensions have no hot reload; the GUI layer does | Hot reload is a svelters-only concern. Rust kinds registered through SV1 restart the process, and that is accepted. |
| Composition with capabilities (S1); bubbling, capture and `on:event` forwarding (S2); a compiler extensible for native store bindings and draft bindings (S4) | SV9, SV3 and SV8. `on:` forwarding conflicts with runes-only (§6 D1). |
| A live IDE: an in-app editor, in-process compile, HMR for user, project and dev layers, possibly in release builds (js-api §6, J6) | The compiler as a library (SV27), editor diagnostics (SV33) and dynamic APIs (SV22–SV26). A release build has no Node, which constrains HMR delivery and CSS scoping (§6 D3). |

### 1.2 Their open questions that touch svelters, and svelters' view

| Open (cadatomic) | svelters' view |
|---|---|
| **C2** Does svelters-egui's arena become the framework's scene, or does the framework import svelters' scene? | The scene stays in svelters, extended by providers (§6 D2). cadatomic's `Node` proposal (scene-node §1) is close to `scene.rs` already: generational keys, taffy layout item, dirty flags, typed props per kind. The additions are generic: `StateFlags`, an anchor, provider-defined state bits, an owner tag and `Widget` behaviours. Only `op`, `pick` and `presents` are cadatomic semantics. They fit a provider-owned extension block on the node (SV1, SV4). |
| **C3** Retained everywhere, or a hybrid? | svelters can support both. A "native island" kind runs an egui closure inside the retained tree (`docs/notes/egui-scene-graph.md` §4, `Native(NativeIsland)`). It makes an incremental port possible: the viewport and existing egui panels become island nodes first. |
| **C4** Isolate details (one per extension or grouped; remoc or irpc) | Transport-agnostic on svelters' side. The proxy backend speaks `svelters_abi::Value` batches over a channel trait. cadatomic plugs remoc or irpc in at the edge, so svelters takes no tokio dependency. |
| **C5** egui's AccessKit output versus the retained tree | svelters' walk projects node role and name onto egui's own AccessKit nodes and does not build `TreeUpdate`s (EGUI.md §1.5). Node ids are egui ids, so a stable node id (SV16) is a stable a11y id. The G1 kittest tests rely on this. |
| **C6** Token file format | svelters is neutral. SV5 takes a resolved token map from Rust, whatever file it came from. |
| **C7** Plugin tiers; own shell | Not svelters'. |
| **C8** Owner of the type registry | Not svelters'. SV45 needs it only as a provider input. |
| **S3** Per-frame `tick` callbacks against descriptors | svelters measures it (SV14), after the native animator (SV13) and the proxy backend (SV15) exist. TR1's current design runs JS per frame on every backend (TR1 §5). That stays the fallback, and descriptors become the egui default (§2.3). |
| **S5** How much CSS | The basics, `var()` and taffy flex and grid fit ST2's decl dialect ported to egui. A Rust selector engine (CSS.md) is not needed for S5's minimum (§6 D4). |
| **D10** subsecond | No effect on svelters. subsecond patches Rust function bodies through a jump table under `dx serve --hotpatch`. It cannot change struct layouts or re-run `inventory` registration, so new kinds (SV1) still restart. JS, styles and tokens reload through svelters' own path. The only svelters duty: the host must survive a patched behaviour function between frames. It does, because behaviours are called fresh each frame. |
| **rquickjs determinism fork** (cadatomic decision 9) against svelters' stock QuickJS-NG | **Decided by the user (2026-10-22): the fork is dropped; svelters stays on stock QuickJS-NG (rquickjs 0.14.0).** No determinism work is planned in svelters. This conflicts with cadatomic's decision 9, PB5 ("on the user's fork") and the `rquickjs` fork set in its `01-overview.md` §6. The cadatomic lane updates its register; svelters does not edit cadatomic. A technical note for that lane: there must be one rquickjs in the process. `rquickjs-sys` has no `links` key, so a second copy would link duplicate QuickJS C symbols. |

## 2. The requests, one row each

Columns:

- **State**: done, partial or none, with evidence.
- **Maps to**: the existing svelters plan.
- **Overlap**: in-flight lanes (CF2, SP1, TR1, FU3, WP-W).
- **Size**: lane-days, in the style of HMR.md §4.
- **Pushback**: anything that conflicts with svelters' design, or a better primitive.

Sizes assume one agent lane with the repo's gates, and are planning estimates.

### 2.1 Extension points (SV1–SV8)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV1 | Kind provider: an external crate registers kinds (props, events, slots, methods, capabilities, a Rust behaviour); spec and typings generated | **none**. The kind table is closed: `crates/svelters-egui/src/kinds.rs` has 8 kinds and a generated enum `W` (`scene.rs`). `#[svelters::widget]`/`wrap_widget!` are designed (`docs/notes/egui-rust-exports.md` §7) and not built | E4; EGUI.md §3.3; resolved disagreement "Rust kind table as the single source" (EGUI.md §6) | SP1 (spreads must check provider props) | 8–10 (SC1 builds a 3–4 day minimum) | — | One macro family (`#[svelters::widget]`, struct and fn forms) and one schema shared with exports (EGUI.md §6). Capabilities are a bitset on the kind, plus typed provider blocks on the node. A behaviour is a trait object; built-in kinds stay a generated enum for speed. |
| SV2 | Behaviour provider: `use:name` and `{@attach}` resolve to registered native behaviours with typed params, on the UI thread | **none** for native behaviours. JS `{@attach}` works; JS `use:` lowers to `Op::Action` on local master (CF1, `07620d2b`, `crates/svelters-compiler/src/lower/mod.rs` `UseDirective`) | new; the compiler resolves `use:name` against the spec first, then falls back to a JS identifier | CF1 (merged locally) | 3–4 | SV1 | **Amend the spelling.** `use:name` in Svelte names a JS binding in scope. A spec-resolved behaviour needs its own namespace or an import: `use:native.pick={…}`, or `import { pick } from 'svelters:behaviors'`. The import keeps svelte-check and stock tooling happy. A behaviour is a node-attached Rust object; the update is a prop set. |
| SV3 | Compiler bind-target hooks: declared bind sources and lowering (Rust store field to a scene-side link; `slot(path)` to draft events), plus bind modifiers (`\|commit`) | **partial**. `bind:` on a writable E3 store field already lowers to `bind_prop` (E3, golden `crates/svelters-compiler/tests/r2/store-field-gtk.svelte`), but the value crosses JS. `\|nonreactive` exists as a modifier (R2) | R2 `plan_bind` policy; E3 stores; E5 types | — | 5–7 | SV1, E5 (exports in the compiler) | Make it a **lowering table, not a code hook**. Each provider declares bind sources as data (pattern, observe class, lowering op), so the compiler stays one binary that loads provider specs. A native link is a host op `link(store, field, node, prop)` executed in Rust, so no JS runs on keystrokes. Draft bindings are a cadatomic bind source whose lowering is "events to a store method". `\|commit` is generic: commit on Enter, blur or release. |
| SV4 | Pseudo-class and node-state hooks: provider states (`:pending`, `:stale`, `:failed`, `:pick-target`, `:pick-rejected`) | **partial**. ST2's decl engine sends state blocks to the backend and the backend picks the active ones (`plans/milestones/ST2-appkit-styling.md`; AppKit only; fixed set hover, focus, active, disabled, checked) | ST2 decl dialect on egui (ST3 below) | — | 1–2 on top of ST3 | ST3, SV1 | Provider states are named bits on the node, set from Rust. The decl translator accepts any state name the spec declares, so no engine change is needed. |
| SV5 | Theme token provider: `--sv-*` replaced or extended by role tokens | **partial**. `packages/style/src/tokens.js`: `DEFAULT_TOKENS` light and dark (16 tokens), `setTokens` overrides from JS. Unstyled egui widgets already take the embedder's `Visuals` | X6 tokens; G2 "tokens on `Visuals`" | — | 1–2 | — | Add a Rust-side `Host::set_tokens(scheme, map)`, and let the token namespace come from the provider (`--cad-*` or `--sv-*`). cadatomic's role tokens map almost 1:1 onto `--sv-*` (`panel`→`--sv-bg`, `raised`→`--sv-surface`, `text`→`--sv-fg`, ...). The rest are added as extra names. |
| SV6 | Event hooks: observe or intercept events before the target and after bubbling | **none** | follows SV9 | — | 2 | SV9 | A Rust filter registered per subtree or root, run in the propagation walk (Qt event-filter shape). Only Rust may intercept synchronously; JS and isolates observe. |
| SV7 | Frame and overlay hooks: draw above the scene, query anchors' rects, schedule frames | **partial**. `EguiBackend::with_scene` reads rects; `request_frame` exists in the ABI (v2.1) | G3 F12 overlay v0 uses the same hooks | — | 2–3 | SV16 (anchors) | One `Overlay` trait called after the walk, with rects by node, anchor or identity. Frames come from the existing `schedule_wake`/`request_repaint` path. |
| SV8 | Diagnostics hooks: provider compile-time checks with their own codes | **partial**. `PlatformSemantics` plus `Overlay` (`crates/svelters-compiler/src/semantics/overlay.rs`) already lets Rust override answers. `Diagnostic.code` is `&'static str` (`diag.rs:13`) | R2 diagnostics; E5 | — | 2–3 | SV1 | Checks are declared in the provider spec where possible (slot path grammar, id sets from a file), and as a `Check` trait in the compiler library otherwise. Codes become namespaced strings (`cad.slot_unknown`). |

### 2.2 Scene, events, observation, styling (SV9–SV12, SV16)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV9 | Capture and bubbling along the egui node tree; Svelte-style `on:event` forwarding | **partial**. ABI v2.2 `listen_ex` with `SVL_LISTEN_CAPTURE` exists and is implemented on GTK/Qt (`plans/architecture/backend-abi.md` §"ABI v2.2", `fixtures/listen-phases`); egui has no propagation. **Forwarding is rejected**: a bare `on:click` is `unsupported_feature` (`lower/mod.rs`, "on: directives with modifiers or without a handler"), and the conformance tracks it under R-legacy (`plans/milestones/CF1-conformance.md`: `event-on-2`, `event-listener-spread`) | G2 sync handlers on raw input; X3 phases | SP1 (rest props are the runes way to forward) | bubbling 3–4; forwarding sugar 1–2 | SV1 (event declarations per kind) | Bubbling and capture: yes, on egui natively. **Forwarding: a conflict with runes-only** (§6 D1). The proposal: a bare `on:click` on a native element inside a component lowers to `onclick={$$props.onclick}`, a runes-compatible callback prop, plus a diagnostic note. SP1's `{...rest}` is the idiomatic path. Legacy component events (`createEventDispatcher`, `$on`) stay out. |
| SV10 | Exact observation for every behaviour-owned value | **done for G1 kinds** (`plans/milestones/G1-egui-skeleton.md` "Observe: exact"). The three widget props are `exact` + `delivery`. Every other prop is `set_only`, which on egui means nothing but the host changes it, so it has no change to miss | — | FU3 item 7 (stale baselines for E3 store fields) | 0.5–1 | SV1 | `set_only` stays the honest class for props nothing native changes; it is not a gap. The work is a rule for provider kinds: a behaviour that writes a prop must do it through `cx.set`, which makes it `exact`, and a test per kind. |
| SV11 | Pseudo-classes from node state flags; `class:`/`style:` on egui; CSS basics, `var()`, flex/grid | **partial**. Layout props in `<style>` on egui (D3, G1). `class:` works through compile-time setter rules. `style:` is rejected on egui ("style: directives on this backend", `lower/mod.rs`). `style` answers `SVL_STYLE_TARGET_NONE` (G1). taffy is built without `grid` (`crates/svelters-egui/Cargo.toml`) | G2 styling; ST2 decl dialect; CSS.md CS5 (deferred) | — | 6–8 (ST3) | SV1 for flag declarations | **ST2's decl dialect on egui (ST3)**, not the Rust engine (§6 D4). The flags hover, active, focus, focus-within, disabled, checked, selected, open, expanded, invalid and exiting are scene bits set from egui responses. The backend applies the matching state block, so no JS runs on hover. Turn on taffy `grid`. `style:` becomes inline decls. One known limit (ST2): a state applies to the element it is written on, so `.row:hover .label` is an error. |
| SV12 | `use:` actions lowered to attachments | **done on local master** (CF1 `07620d2b`: `UseDirective` → `Op::Action`); not on origin `7aee41e5` | CF1 | CF1, CF2 (attachment and template-effect order) | 0.5 (egui test) | — | None. |
| SV16 | Stable compiler-emitted identity (D4), for anchors and a11y | **none**. Ids are `Id::unique(("svl", handle))` (`crates/svelters-egui/src/scene.rs:240`), stable across keyed reorders but not across reloads or runs | EGUI D4 (user: yes); G5; HMR H4 | CF2 (template structure feeds the identity's template position) | 4–6 | — | Two layers. An **explicit anchor** (`anchor="panel.task/slot[extents]"` or `use:anchor`) comes first. cadatomic builds anchors from domain ids (SCOPE §6.2 rule 2) and needs it first. The **compiler identity** (file + template position + key path + instance path) comes second. The egui `Id` is salted by root, so two roots or two hosts never collide. Handles count from 1 per backend today. |

### 2.3 Animation (SV13, SV14, SV37)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV13 | Animator in the scene: descriptors (duration, delay, easing, keyframes), intro and outro with exiting nodes, `animate:flip`, implicit transitions, reduced motion, virtual clock | **in flight, JS-driven**. TR1 (`origin/ci/tr1` `1156212a`, 29/33 transition samples) reuses Svelte's own `transition()`/`pause_effect`. A JS WAAPI-style player sends one style op per channel per frame (TR1 §0, §5). On egui: opacity through `multiply_opacity`; transform and clip "later"; the `bounds` member stays NULL, so no FLIP | TR1, then **TR2 (native player)** | **TR1 heavily** | 8–12 (TR2) | TR1 merged; ST3 for implicit `transition:` decls | TR1's design already samples `css(t)` into keyframes, which is a descriptor. TR2 sends the keyframes and timing once (`SVL_STYLE_ANIMATE`, additive on the style op). The egui scene interpolates and reports `outro_done`, and the JS player stays the fallback for backends without it. Exiting nodes: TR1 keeps outroing branches in place; the scene marks them non-focusable, non-pickable and out of the a11y tree. FLIP needs `bounds` on egui, which is cheap because the scene knows its rects. |
| SV14 | Profile per-frame `tick` in-process and from an isolate | **none** | TR2 + SV15 | TR1 | 2–3 | SV13, SV15 for the isolate half | The in-process half can run right after TR2. The isolate half waits for SV15. |
| SV37 | Animation demand per root and viewport; slow-motion factor | **none**. Frame requests are per backend (`request_frame` → `host.frame()`) | TR2 | TR1 | 1–2 (in TR2) | SV17, SV18 | Fold it into TR2. |

### 2.4 Isolates and dynamic APIs (SV15, SV21–SV26)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV15 | Isolate target: proxy backend, message-driven isolate host, compiler target with Promise calls and sync handlers as errors, atomic batches with coalescing | **none**. The op stream is already serializable and replayable (op-log format, `svelters-backend-log`), and the compiler has a portability mode (R2 `--backend gtk,qt,…`) to model a target on | QUICKJS-RUNTIME item 9 ("one `Runtime` per thread, messages as `svelters_abi::Value`") | — | 10–15 | SV17 (mount into slots), SV16 (handle mapping by identity is optional) | The transport is a trait, so remoc and irpc stay in cadatomic's edge crates and svelters adds no tokio. Isolate handles are minted in the isolate (namespaced). `get_prop` reads from a mirror kept by the `changed` batches (ABI v2.6). |
| SV21 | Batch metadata: root ids, sequence numbers, detached build of new roots, queue bounds, coalescing | **none** | part of SV15 | — | 3–4 | SV15 | One lane with SV15. |
| SV22 | Host-supplied module resolver and loader for extra schemes, per host | **none**. The resolver accepts `svelters:core`, `svelters:app/*` and relative paths (`crates/svelters-host/src/quickjs.rs:1045`) | QUICKJS-RUNTIME Q1 (module loader) | — | 2 | — | None. The hook returns source text or a typed error, synchronously (QuickJS's loader is synchronous). |
| SV23 | Dynamic export modules registered at run time from a descriptor, dispatched to a host `dyn` handler | **none**. The schema is built once from `inventory` (`exports/schema.rs`, a `OnceLock`) | E0 registry (`crates/svelters-host/src/exports/`) | — | 5–7 | SV24 | Keep `svelters-exports/1` as the descriptor. A dynamic module is a schema fragment plus a handler. Handles carry a publisher generation, the same rule E3 uses for runtime generations. |
| SV24 | Codegen as a library over a descriptor value | **partial**. `exports::{json, typescript, javascript}` exist, but over the static `schema()` (`exports/codegen.rs`) | E0 codegen | — | 2–3 | — | None: change the signature to take a `&Schema`. |
| SV25 | Per-host visibility filter for `svelters:app/*` | **none** | E0 resolver | — | 1 | SV22 | None. |
| SV26 | Type facts from a supplied `exports.json` at run time | **none**. `PlatformSemantics::native_type` answers `None` for `svelters:app/*` (E0 "Modules, typings, compiler") | **E5** | — | 3–4 | SV27 | One lane with E5. |

### 2.5 Roots, reload, settle (SV17–SV20, SV30, SV35, SV36)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV17 | Mount into a host-provided slot (`mount(entry, slot, props)` / `unmount`), many per runtime, no toplevel | **partial (JS half)**. `mount(Component, { props, target })` mounts into an existing host handle and never destroys it (`packages/native-runtime/src/index.js:1308`). Missing: a Rust-side API to create a slot node and mount an entry export into it with props; the egui backend drawing a slot into a given `Ui`; Rust-side unmount and props updates | EGUI D1 embedding; `docs/notes/egui-all-rust.md` §2.4 islands | — | 4–6 (in SC1) | G0 start (done) | None. See §2.8 SVP1/SVP2 for the egui half. |
| SV18 | Reparent a root's top node between slots and viewports with no remount; `layout` event | **none**. Further windows are immediate viewports (G1 "Windows") | G2 X7 toplevels | — | 4–6 | SV17; deferred viewports through a thread-local scene (cadatomic W2) | In the arena a reparent is a pointer move. The work is in the deferred-viewport walk and in restoring focus. |
| SV19 | Per-root visibility; frames paused while hidden | **none** | — | TR1 (rAF) | 1–2 | SV17 | None. The `visible` context comes from the host, as a per-root reactive value. |
| SV20 | `$state` snapshot and restore by identity across HMR and full reloads | **none**. A reload is a full in-process restart: all JS state and native UI state are lost; E3 services, stores and `persist` survive (HMR.md §1.2) | HMR H1 + H3 (+ H4 node state) | CF2 (wrapper and template-effect shape) | H1 6–8; H3 4–6; H4 4–6 | SV16 | **The state rule differs.** cadatomic §7.2 keeps per declaration (same name, same initializer hash, value type-checks). HMR.md D3 recommends resetting the whole component when its signature changes. svelters proposes cadatomic's rule plus HMR.md's per-instance registry (§6 D3). Module delivery (HMR D1) also has to be decided. |
| SV30 | Dev-mode live value overrides keyed by identity | **none** | new; after H4 | — | 4–6 | SV16, SV20 | None. Emit in dev builds only, as asked. |
| SV35 | Settle API with an iteration cap | **partial**. `Host::checkpoint`, `is_idle` (`crates/svelters-host/src/lib.rs:492`). kittest's `harness.run()` steps until no repaint is requested. `is_idle` ignores in-flight E1 ops, dirty E3 stores and the backend's pending frames | R1 batch; G1 frame driver | — | 2–3 | — | Two things: `flush_until_settled(max)` with a reported chain, and a `settled()` predicate that cadatomic's `IdleBarrier` can register as a busy source (SVP4). |
| SV36 | Keep only values that changed across reload | **none** | H3 | — | 0.5–1 (in H3) | SV20 | None. It composes with the initializer-hash rule. |

### 2.6 Compiler and tooling (SV27, SV31–SV34)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV27 | Compiler library: incremental compile of one file with spans, codes, fix-its, without the CLI | **partial**. `svelters_compiler::compile(src, &CompileOptions) -> CompileResult` and `compile_module`. Diagnostics carry `code`, `start`/`end` and `help` (`crates/svelters-compiler/src/compile.rs:88`, `diag.rs`). There are no fix-its or incremental state, and CSS scoping still runs in Node (Vite plugin) | EGUI §4 `svelters-build`; Q3 (CSS scoping in Rust) | — | 3–5 (+Q3 ~1 lane for CSS without Node) | — | The in-app IDE in a release build cannot run the Vite step. CSS scoping in Rust (Q3) is a dependency of the live IDE (§6 D3). |
| SV31 | Kind contracts and typed slots | **none** | E4/E5 | SP1 | 4–6 | SV1 | None. |
| SV32 | Binding-loop, purity and unit checks over the lowered IR | **partial**. R2's seven bind diagnostics only | R2 | CF2 (IR shape) | 6–10 | SV1 (units from registry types) | Purity as an error only for the isolate target and a warning elsewhere matches cadatomic's SQ5 proposal. Start with `bind:clientWidth`-style layout loops, which egui makes possible. |
| SV33 | Editor diagnostics and completion from the native compiler | **none** | new | — | 10–15 | SV27, SV1 | Write it as an LSP over SV27 that plugs into the Svelte language server rather than replacing it. Order it after SV27, since the in-app editor needs SV27 first. |
| SV34 | One test case against every backend | **partial**. Op-log fixtures replay on GTK, Qt and egui (`oplog_replay.rs` per backend), `tools/oplog-compare.mjs` compares by state, the conformance harness has an `expected.json` ratchet (`experiments/native-compiler/conformance`), and the metadata ledger exists | — | — | 3–5 | SV15 for the isolate target | The missing piece is a single `.svelte` case with inline expectations dispatched to all runners. The building blocks exist. |

### 2.7 Lists and models (SV40–SV47)

| Id | Request | State (evidence) | Maps to | Overlap | Size | Depends on | Pushback / alternative |
|---|---|---|---|---|---|---|---|
| SV40 | `virtuallist` on svelters-egui: recycled row slots, variable heights, scroll anchoring by key, set-size a11y | **none on egui**. The M5 protocol is built on GTK/Qt (`plans/milestones/M5-lists.md`) | G2 `virtuallist` (EGUI.md §1.5) | — | 5–7 | — | None. Setup and bind run before the pass for the predicted range (EGUI.md §1.5). |
| SV41 | Rust model mode: the `model` prop takes a Rust object implementing a row-model trait | **partial analog**. N3 passes a GTK `GListModel` or Qt `QAbstractItemModel` and the view reads it with no JS copy (`plans/milestones/N3-native-models.md`) | N3 + E0 shared objects | — | 4–6 | SV40 | None. The trait lives in svelters-host (engine-neutral), so GTK and Qt can adapt it too. |
| SV42 | Row records: one record per row bind with the columns the snippet reads | **none** | M5 bind args | CF2 | 3–4 | SV41 | None. Static `item.<field>` analysis is a lowering pass. |
| SV43 | Row templates: script-free row snippets lowered to declarative templates the scene rebinds without JS | **none**. EGUI.md §4 proposes a cross-backend "static-template op" | static-template op | **CF2** (template effects decide what a "script-free" fragment is) | 8–12 | SV42, CF2 merged | Make this the static-template op of EGUI.md §4 with column bindings, rather than a list-only feature. Every backend's first paint gains from it. |
| SV44 | Columns and tree rows in the list protocol | **none**: M5 open item 3, N3 open item 5 | M5/N3 follow-ups | — | 6–8 | SV40 | None. |
| SV45 | Typed model typings from the kind provider | **none** | E5 + SV1 | — | 2–3 | SV1, SV41 | None. |
| SV46 | Key-based multi-selection with deltas | **none** | M5 | — | 3–4 | SV40 | None. |
| SV47 | List-tier benchmarks on egui in svelters CI | **none** for egui; REACTIVITY and the M5 benches give the GTK/Qt numbers | bench harness | — | 2–3 | SV40–SV43 | None. Grow it alongside each tier. |

### 2.8 Gaps cadatomic did not file (svelters proposes; SVP = svelters-proposed)

These came out of reading both codebases. Most are prerequisites of SC1.

| Id | Gap | Evidence | Size | Owner |
|---|---|---|---|---|
| SVP1 | **Logic/ui split for embedding.** `EguiBackend::ui` runs the checkpoint (JS) inside the egui pass's caller. cadatomic's rule is "`App::logic` drains and executes, `App::ui` builds widgets only" (SCOPE §6.1, §6.2 rule 3). Split it into `backend.logic(&ctx)` (closes, checkpoint, rAF, remeasure, layout) and `backend.show_slot(ui, slot)` (walk plus outbox) | `backend.rs:206` (`before_pass` called from `ui` on pass 0) | 1–2 | svelters |
| SVP2 | **Rust node API and many roots per backend.** Rust creates slot nodes and provider-kind nodes, with no JS and no `window`. The walk takes a root rather than "all windows" | `backend.rs` `walk` iterates presented windows; window 0 goes in a `CentralPanel` | 2–3 | svelters |
| SVP3 | **An owned embed handle and id salting.** `Host<'a>` borrows the vtable (the test harness uses `Box::leak`, `harness.rs`). Give embedders one owning struct. Salt node ids per host and root | `harness.rs` | 1 | svelters |
| SVP4 | **A settled predicate for wait-idle**: no queued events, timers, in-flight E1 ops, dirty stores, pending checkpoint, frame, layout or outbox. cadatomic's `IdleBarrier` registers it as a busy source | `Core::is_idle` covers events and timers only | 1 | svelters (API); cadatomic (registration) |
| SVP5 | **A serde bridge for exports.** cadatomic's types (`Refusal`, `Availability`, `InputSchema`, `SlotStatus`, ...) are serde + schemars in `cadatomic-core`. That crate must stay headless and build for `wasm32`, so it cannot derive `svelters::Record`. The orphan rule then forbids `JsType` impls in a glue crate. Add `Json<T: Serialize + DeserializeOwned>` (and `JsonSchema` for typings), and data enums (`#[js(tag)]`), which E0 lists as missing | E0 "Deviations" (data enums, `Json<T>` not in E0) | 3–4 | svelters |
| SVP6 | **Fonts and icons.** Map `font-weight` to a named family the embedder registers (cadatomic has `Name("semibold")`). Add an `icon` kind: a glyph in a named family, with an optional name table (cadatomic ships Phosphor as family `icons`, `theme/mod.rs`) | ecosystem catalog `icon` (EGUI.md §5) | 1–2 | svelters |
| SVP7 | **Feature hygiene.** Embedders depend on `svelters-egui` with `default-features = false`. Otherwise its `eframe` feature turns on eframe `glow` next to cadatomic's `wgpu` (features unify) | `crates/svelters-egui/Cargo.toml` | 0 (doc) | cadatomic |
| SVP8 | **QuickJS opt-level.** svelters' workspace sets `[profile.dev.package.rquickjs-sys] opt-level = 2` (JS-heavy tests are 5–10x slower without it). Profiles do not propagate from dependencies, so cadatomic's workspace needs the same lines | `Cargo.toml` (svelters root) | 0 | cadatomic |

## 3. Roadmap: svelters lanes for cadatomic, in dependency order

The svelters lane names its own milestones and lists the SV ids inside each. cadatomic's order matters too:

- the native side first, then hot reload (Native extensions decision);
- tokens and `Op` before retained work (framework §12);
- the embedding trial at step 8 of framework §12. The user's pivot pulls it to the front, which is why SC1 comes first.

| Lane | Content | Needs | Size (svelters) | Acceptance | Who |
|---|---|---|---|---|---|
| **SC1** slice (§3.1) | SVP1–SVP6; SV17; SV1 minimum (struct form, props, events, Rust `Widget`, spec fragment + `.d.ts`); SV16 minimum (explicit `anchor` → stable salted egui `Id` and AccessKit author id); G2 subset: `grid` (taffy grid), `collapsing-header`, `toggle-button` (segmented), `combo-box`, `separator`, `scroll-area`, `icon`, `tooltip` on all kinds, an `invalid` flag | G0, G1, E0–E3 (done) | 12–16 | §3.1 | svelters + cadatomic (glue) |
| **ST3** styling on egui | SV11, SV4, SV5: decl dialect on egui, node state flags, provider states, Rust-side tokens, `class:`/`style:` on egui, `insets` and percent lengths (G1 deviations) | SC1 | 6–8 | gallery page under kittest snapshots (needs the `wgpu` kittest feature with lavapipe); a hover restyle with no JS entry; a `:pending` provider state styled by a cadatomic test kind | svelters |
| **G2c** rest of Tier 0 for panels | menus and context menus (X4 kinds), popover, modal and dialog roots, progress bar, spinner, hyperlink, image; key/pointer sync handlers before the pass; focus API (`focus-scope`, restore on root activation); SV19 | SC1 | 8–10 | gallery pages; focus-traversal kittest (widget survey pin 7) | svelters |
| **EV1** events and behaviours | SV9 (bubbling and capture on egui, the forwarding decision), SV6, SV2, SV7, SV10 rule, SV12 egui test | SC1 | 8–10 | `on…capture`/bubble fixture replays on egui; a native `use:` behaviour from a provider runs with no JS per frame | svelters |
| **H0/H1** hot reload | HMR H0 (geometry carry-over, egui reload test), H1 (per-component swap; delivery per §6 D3) | SC1 (roots) | 8–11 | HMR.md H1 tests, plus a swap of one root in cadatomic's kittest leaving other roots' handles untouched | svelters |
| **ID** identity + state | SV16 full (compiler identity), H3 with SV20/SV36 rules, H4 (node state), SV30 | H1 | 12–18 | HMR.md H3/H4 tests; cadatomic W7 on the SC1 panel (focus, caret and `$state` kept; edit-to-pixels ≤ 300 ms) | svelters |
| **LS1** lists | SV40, SV41, SV46, SV47 (T1/T2 benches) | G2c | 12–16 | 100k-row Rust model scrolls at frame budget; selection by key survives filtering | svelters |
| **CX** compiler extension points | SV3, SV8, SV31, SV45, E5 + SV26 | SC1 (provider spec) | 14–20 | a native store binding types with no JS entry (counted); a cadatomic provider check fails a bad slot path at build time | svelters |
| **TR2** native animator | SV13, SV37, `bounds` on egui; SV14 in-process half | TR1 merged, ST3 | 10–14 | TR1's samples pass on egui with no per-frame JS for keyframe transitions; reduced motion; virtual clock in kittest | svelters |
| **LS2** list tiers 2–3 | SV42, SV44, SV43 (static-template op) | LS1, CF2 merged | 17–24 | T3 rows rebind with zero JS while scrolling (counted) | svelters |
| **ISO** isolates | SV15, SV21, SV22, SV25, SV14 isolate half | SC1, EV1 (no sync handlers) | 16–22 | cadatomic W5/W6 on the svelters side: three isolates at 10k `set_prop`/s keep UI p99 ≤ 16.6 ms; a crash or loop interrupts only that isolate's roots | svelters (proxy, host, target); cadatomic (bus transport) |
| **API** dynamic APIs | SV23, SV24 | ISO (per-host resolver) | 7–10 | a module registered and unregistered at run time; typings regenerate from the descriptor | svelters |
| **IDE** tooling | SV27 (+ Q3 CSS scoping in Rust), SV32, SV33, SV34 | CX | 25–35 | per-keystroke diagnostics ≤ 20 ms in process (cadatomic AP6); `<style>` builds without Node | svelters |

Parallelism. After SC1:

- ST3, G2c and EV1 can run in parallel, since they touch different files of `svelters-egui`;
- H0/H1 can run alongside them;
- CX can start once SC1's provider spec format is fixed.

The machine allows at most two heavy lanes at once (memory notes: three lanes OOM'd the VM).

### 3.1 SC1, the first milestone: one cadatomic panel rendered by Svelte

**Panel.** The task panel for `body.box`. It is the highest-value surface and it is fully generated:

- `forms::show(ui, entry, view, cat, state, out)` turns `(CommandEntry, SessionView)` into `Vec<Event>`, with no per-command code (`crates/cadatomic-app/src/forms/mod.rs`);
- it has an exact behavioural spec in `crates/cadatomic-app/tests/task_forms.rs`;
- it exercises unit-aware text fields, X/Y/Z rows, direction presets as segmented buttons, the collapsed Placement group, inline errors, and Accept/Cancel with a disabled reason;
- it needs no selection box (`body.boolean`'s picks come in a later step).

**svelters lane delivers:**

1. `svelters-egui` embedding (SVP1–SVP3):
   - `logic(&ctx)` and `show_slot(ui, slot)`;
   - Rust-created slot nodes;
   - an owned embed handle;
   - salted ids.
2. SV17 from Rust:
   - `embed.mount(entry_module, export, slot, props) -> RootId`;
   - `set_root_props`;
   - `unmount`;
   - one runtime, several roots, no `window` element.
3. The SV1 minimum:
   - a provider crate registers a kind with typed props, events and a Rust `Widget` (`measure`, `ui`, a11y name);
   - the backend merges it into the kind table;
   - the compiler reads the extra spec fragment (`--spec <file>` or the build API);
   - typings come out as `.d.ts`.
4. The SV16 minimum: an `anchor` prop on every kind becomes the egui `Id` (salted by root) and the AccessKit author id.
5. The G2 subset listed in the SC1 row, with accessible names forwarded (egui_kittest 0.37 fails unnamed inputs).
6. SVP4 `settled()`, SVP5 `Json<T>` and data enums, SVP6 fonts and icons.
7. Tests in svelters: a kittest of two roots in one runtime inside a host `Ui`, one with a provider kind; idle 0 frames; ids unchanged across a keyed reorder and across a second root mounting.

**cadatomic GUI lane delivers** (a proposal for that lane; it decides):

1. A `SvelteView: ViewClass` hosting one slot. The right panel's task branch draws it instead of `surfaces.task_ui` behind a setting or feature, so both can be compared.
2. An exports module `svelters:app/task`, built on SVP5 DTOs:
   - an E3 store with the session's state, slots (status, value, problems), `valid`, refusal and preview fields;
   - methods `setField(slot, entered)`, `accept()`, `cancel()`, `rebase()`, each pushing the same `lifecycle::Event`s the Rust form pushes;
   - `units.parse(text, dim)`;
   - `catalog.t(id, args)`, with labels from the Fluent catalog so the i18n source scan still holds.
3. A `QuantityField` kind through SV1, to prove a cadatomic Rust component used from Svelte.
4. Token feed from `theme::Tokens` to `--sv-*` and extras, plus the scheme. Fonts come for free: one egui `Context`.
5. Registration of `settled()` with the `IdleBarrier`, and the `rquickjs-sys` opt-level (SVP8).
6. Where the Svelte source and its build live, and how `check.sh` builds the bundle (§6 D6).

**Acceptance (joint):**

- In the running app (`scripts/xrun.sh`), invoking `body.box` opens the Svelte task panel in the right region.
- Typing extents and pressing Accept commits through the registry and writes one command-log line, identical to the Rust panel's.
- A kittest port of `task_forms.rs`'s `body.box` cases passes against the Svelte panel:
  - `10` → 10 mm;
  - fractions;
  - pasted lists;
  - invalid input blocks Accept;
  - direction presets;
  - Placement collapsed while default.
- Idle draws 0 frames; `wait_idle` settles after Accept.
- The pixel tier's `ui.tree` lists the fields by role and label, and `ui.type` drives them.
- No new `[patch]` entries; one egui in the tree (`cargo tree -d -i egui` is empty).

**Not in SC1:** styling beyond `Visuals` (ST3), draft bindings (CX), hot reload (H1), isolates.

## 4. Versions and build alignment (checked in both `Cargo.lock`s)

| Crate | svelters | cadatomic | Note |
|---|---|---|---|
| egui, eframe, egui-wgpu, egui-winit, epaint, egui_kittest | 0.37.0 (`=0.37.0` in svelters-egui) | 0.37.0 | identical |
| wgpu | 30.0.1 | 30.0.1 | |
| accesskit / accesskit_winit / accesskit_consumer | 0.25.1 / 0.34.1 / 0.39.1 | same | |
| winit | 0.30.13 | 0.30.13 | cadatomic will fork it for Wayland tear-off (D7). svelters follows the fork set when that happens |
| kittest | 0.5.0 | 0.5.0 | |
| rquickjs / rquickjs-sys | 0.14.0 | not yet | stock QuickJS-NG, no fork (user, 2026-10-22); cadatomic's scripting host (PB5) shares the same crate |
| taffy | 0.15.0 | — | add the `grid` feature in ST3 |
| inventory | 0.3.25 | 0.3.25 | both registries use it |
| smallvec | 1.16.3 | 1.16.3 | cadatomic's kernel-lock exception covers it |
| serde_json | 1.0.154 | 1.0.151 | svelters asks for `"1"`, so cadatomic's lock stays at the kernel's 1.0.151 |

- cadatomic's lock check against the kernel lock (`scripts/check-boundaries.sh`) only compares shared crates. svelters adds rayon, notify, sourcemap and taffy, which the kernel lock does not have.
- `egui_tiles` 0.18 and `egui_mcp` 0.3 now target egui 0.37 (both in the local registry), as cadatomic's research noted.
- **D9 (egui version).** svelters' provisional pin matches cadatomic exactly. Upgrades should be one joint event: cadatomic's egui fork set and svelters-egui move together (§6 D5).
- **Release packaging.** The bundle (or QuickJS bytecode, once Q1 exists) is embedded with `include_bytes!`.
- **Dev builds.** They load from disk and watch. Vite and pnpm stay dev-time tools until Q1 (module loader) and Q3 (CSS scoping in Rust) remove them (§6 D3).

## 5. Coordination protocol

cadatomic keeps its requests in `docs/plan/04-kernel-requests.md` ("to file", "filed", "answered" with a sha). svelters
does not edit cadatomic. Proposal:

1. **A mirrored section in svelters `STATUS.md`, "Requests from cadatomic".** The svelters coordinator adds it when the
   user accepts this document; this research branch does not touch `STATUS.md`. One row per SV id:

   | Id | Short | svelters state | Lane | Sha | Note |
   |---|---|---|---|---|---|
   | SV17 | mount into a host slot | scheduled | SC1 | — | JS half exists (`index.js:1308`) |

   The svelters states are:
   - **accepted**: in the roadmap, not started;
   - **scheduled \<lane\>**: in a planned milestone file;
   - **in progress \<lane\>**: a branch exists;
   - **landed \<sha\>**: merged to svelters `master`; the sha is the merge commit;
   - **alternative \<ref\>**: svelters proposes a different primitive and waits for the user;
   - **declined \<reason\>**: only with a user decision.
2. **Filing.** cadatomic's "filed" sha is the svelters commit that adds the row to `STATUS.md`. A peer's reply is
   information, not user approval, on both sides.
3. **Landing.** A milestone record names the SV ids it answers, under "Requests answered". The cadatomic lane copies
   the landed sha into its table as "answered" at its next pin advance of svelters.
4. **Read points.** Every svelters milestone that serves cadatomic names the cadatomic sha it read (here `9e73bf6`).
   cadatomic names the svelters sha it builds against, by its own pin rules (svelters as a sibling checkout, `../svelters`,
   pinned by sha in `pins.toml`, advanced in its own commit; the GUI lane decides).
5. **New requests from svelters to cadatomic** go in the svelters milestone file under "Requests to cadatomic", using
   the same ids style (`SVP` here). The cadatomic lane copies the ones it accepts into its own plan.
6. **Shared fixtures.** cadatomic's acceptance tests for svelters features (W4, W5, W7, AP7) run in cadatomic. Each
   svelters lane adds a svelters-only test of the same property, so svelters CI catches regressions without cadatomic.

## 6. Decisions for the user

1. **D1 `on:event` forwarding (SV9) against runes-only.** svelters forces runes mode. In Svelte 5, a bare `on:click` is
   legacy component-event forwarding, rejected today and tracked as R-legacy.
   - (a) Support legacy forwarding: component events (`createEventDispatcher`, `$on`) on the native runtime. This
     reverses runes-only for one feature.
   - (b) **Sugar:** a bare `on:click` on a native element inside a component lowers to `onclick={$$props.onclick}` (a
     callback prop) and declares the prop in the component's typings. The syntax is what S2 asked for, and the
     semantics stay runes.
   - (c) Refuse; use callback props and SP1's `{...rest}`, with a fix-it.

   **Recommendation (b)**, documented as a svelters extension, with (c)'s fix-it offered as the idiomatic form.
2. **D2 Scene ownership (cadatomic C2).**
   - (a) **svelters-egui's scene is the scene**, extended by providers (SV1/SV2/SV4/SV6/SV7) and a Rust node API (SVP2).
   - (b) cadatomic's framework owns the scene and svelters adapts its kind table onto it.

   **Recommendation (a).** One tree, one layout, one a11y projection, one op log; svelters' egui backend stays usable by
   other apps. The cadatomic lane still writes all cadatomic semantics.
3. **D3 Hot reload: the state rule and module delivery (SV20, SV36; HMR.md D1/D3).**
   - **State rule.** cadatomic keeps per declaration (name, initializer hash, type) and only changed values (SV36).
     HMR.md recommends resetting the whole component on a signature change. **Recommendation:** cadatomic's rule with
     HMR.md's per-instance registry, since D9 already says keep.
   - **Delivery.** HMR.md D1 proposes Vite's `ModuleRunner` in QuickJS first and the Rust module loader (Q1/Q5) later.
     cadatomic wants in-process compile and possibly the live IDE in release builds (J6), and Node is unavailable
     there. **Recommendation:** go straight to the Rust loader, and port CSS scoping (Q3). Keep Vite for `pnpm`-based dev
     only until then. This makes Q1+Q3 (≈ 2 lanes) a prerequisite of H1 for cadatomic.
4. **D4 Style engine for egui (SV4/SV5/SV11; CSS.md deferred by the user).**
   - (a) **ST2's JS decl dialect on egui (ST3).** The cascade, `var()` and matching run in JS when classes change.
     State blocks are applied by the scene, so hover runs no JS.
   - (b) Bring CSS.md's Rust engine forward (CS0, CS1, CS5: about 3 lanes).
   - (c) Setters only.

   **Recommendation (a).** It meets S5's minimum in one lane, and `SVL_STYLE_DECLS` stays the sink if the Rust engine
   comes later. Revisit when isolates need cascades without JS, or when `:has()`/sibling selectors are wanted.
5. **D5 egui version (EGUI D9).**
   - (a) **Decide 0.37 now; upgrade jointly with cadatomic** (one minor per svelters-egui line).
   - (b) svelters upgrades on its own schedule.

   **Recommendation (a).**
6. **D6 Where cadatomic's Svelte sources and their build live** (cadatomic's call, raised here because it shapes SC1).
   - (a) **In cadatomic** (`ui/`, with svelters packages by relative path at the pinned sha), built by `check.sh`.
   - (b) In svelters as an example app.
   - (c) A third repository.

   **Recommendation (a).** The sources depend on cadatomic's exports typings and catalog.
7. **D7 Priority order.**

   **Recommendation:** SC1 → (ST3 ∥ G2c ∥ H0/H1) → EV1 → ID → LS1 → CX → TR2 → LS2 → ISO → API → IDE.

   This puts the user's pivot (one real panel) first, then what every first-party panel needs (styling, elements,
   events, reload with state). Isolates come later, as cadatomic's own order has them (native side first, extensions
   later). If third-party extensions are wanted sooner, ISO can move ahead of CX and TR2. It needs only SC1 and EV1.
8. **D8 Native-acceptance goldens** (cadatomic widget survey W3).
   - (a) The svelters lane owns NA1 capture on its native backends and publishes goldens as files.
   - (b) cadatomic depends on svelters' native backends as a dev dependency.

   **Recommendation (a).** It reuses the ledger and the headless runner, and keeps GTK, Qt, AppKit and WinUI out of
   cadatomic's build.

## 7. Risks

- **Two scenes.** If C2 goes the other way or stays open while both lanes build, the retained tree is written twice.
  Settle D2 before SC1 starts.
- **UI-thread budget.** First-party JS shares the frame with Rust. Checkpoints that run long show up as dropped frames
  in the whole app, not just the panel. Mitigations:
  - the interrupt handler and budgets (docking §6.4);
  - E1 offload for anything heavy;
  - T3 rows (SV43) for big lists.
- **egui churn ×2.** Both repositories carry the egui upgrade cost. Joint upgrades (§6 D5) halve the risk of a split.
- **Id hygiene across roots.** Duplicate ids break interaction silently (EGUI.md §9). Salting per root (SVP3) and kittest
  id-clash checks are needed in SC1.
- **Scope creep in "generic".** Several requests (SV3, SV8, SV31, SV45) let cadatomic shape svelters' compiler. Keep
  them data-driven (provider specs), so svelters stays one compiler with no cadatomic code.
- **Build cost.** svelters-egui's first release build was 8 min 16 s with 4 jobs (G1). cadatomic's one-builder rule and
  the 8 GB guard apply to it too.

## Sources

- cadatomic `9e73bf6`:
  - `docs/plan/00-decisions.md`, `docs/plan/04-kernel-requests.md`;
  - `docs/research/2026-10-22-{component-framework,scene-node-and-svelte-semantics,docking-windows-and-svelters-roots,js-api-publishing-and-live-ide,models-filtering-and-dynamic-data,slint-lessons,widget-survey-and-native-acceptance,native-extensions-abi-and-engines,ide-shell-and-ux,code-editor-and-lsp}.md`;
  - `crates/cadatomic-app/src/{app.rs,forms/mod.rs,theme/mod.rs,dock/seam.rs,views/mod.rs,pixel.rs}`;
  - `crates/cadatomic-core/src/{schema,units.rs,lifecycle,jobs}`;
  - `Cargo.toml`, `Cargo.lock`, `pins.toml`, `forks.toml`, `scripts/check-boundaries.sh`.
- svelters:
  - `plans/research/EGUI.md` (D1–D10);
  - `plans/milestones/{G1-egui-skeleton,E0-rust-exports,E1-exports-offload,E2-exports-buffers,E3-exports-reactive,ST2-appkit-styling}.md`;
  - `crates/svelters-egui/src/{backend,runner,harness,scene,render}.rs`;
  - `crates/svelters-host/src/{lib.rs,quickjs.rs,core.rs}`;
  - `crates/svelters-compiler/src/{lower/mod.rs,compile.rs,diag.rs,semantics/overlay.rs}`;
  - `packages/native-runtime/src/index.js`, `packages/style/src/tokens.js`;
  - `docs/notes/egui-{all-rust,scene-graph,rust-exports,ecosystem}.md`;
  - `plans/research/QUICKJS-RUNTIME.md` item 9.
- svelters branches:
  - `origin/research/css:plans/research/CSS.md`;
  - `origin/research/hmr:plans/research/HMR.md`;
  - `origin/ci/tr1:plans/milestones/TR1-transitions.md`;
  - `origin/ci/cf2` log;
  - local `master` `07620d2b`: CF1, including `plans/milestones/CF1-conformance.md` and the `use:` lowering.
