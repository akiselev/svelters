# skelter implementation plan

Part of the GUI toolkit proposal (`README.md` in this folder). Date: 2026-10-25. Status: proposal
for the user; nothing is built. Read `architecture.md`, `compiler-and-js.md` and `platform.md` first.

This plan turns the architecture into **work packages (W-…)**. Each package has an owner repository,
a size, the place it can run, its dependencies and a gate. Packages are grouped into **milestones
M1–M6**, and each milestone ends with a measured gate. The **go/no-go is at M1**: nothing after it
starts until M1's numbers are in and the user has looked at them.

The egui-side optimisations (round 1's M0: the low-latency surface config, DX12 on Windows,
same-frame dispatch of widget signals, latency instrumentation on egui) are being done by another
agent and are **not** in this plan. Several packages here also help egui (W-C1, W-C3, W-C5, W-H1),
and they are marked; the coordinator must give those to one owner, so the two efforts don't touch
the same files twice (§6).

## 1. How the work runs

- **Cloud batches as CLOUD.md describes:**
  - a coordinator plans each batch and the user approves its size;
  - lanes work on `claude/b<N>-<lane>` branches in their own containers;
  - a consolidation merges them;
  - only the coordinator merges `master`.
- **Most packages run in cloud containers.**
  - The headless crates (`skelter-core`, `-anim`, `-frame`, `-text`) need no GPU.
  - The renderer and compositor run on lavapipe, as cadatomic's kittests do now.
- **Some work needs the user's machines.** It waits in `docs/plan/REVISIT.md` (CLAUDE.md, "Who
  decides"):
  - real Windows and macOS present paths;
  - IME acceptance with real input methods;
  - screen readers;
  - timing measurements on a quiet machine;
  - the photodiode latency rig.

  The plan marks these **[local]**. A cloud package never claims a [local] result.
- **Each package lands with tests in its own crate,** plus the screenshot rule for anything that
  draws: a screenshot to the user at each visible milestone (CLAUDE.md).
- **Not implemented means not registered,** in skelter as in cadatomic: no placeholder kinds, ops or
  spec entries.

**Repositories** (README Q2 decides the name and home):

| Repository | Holds |
|---|---|
| `skelter` (new, own repo, MIT OR Apache-2.0 like berth/quire, git tags only) | `skelter-core`, `-anim`, `-frame`, `-text`, `-render`, `-compositor`, `-platform`, `-widgets`, `-egui-island`, `-test` |
| svelters | `svelters-skelter`, compiler and runtime changes, host changes |
| berth, quire | `skelter_berth`, `skelter_quire` beside the egui renderers (README Q3: the "egui-only" wording in DECISIONS.md changes) |
| `wgpu` fork (own repo, registered in `forks.toml`) | patches P1–P4 (architecture §8.6) |
| `winit` fork (own repo, registered), **only if needed** | one bridge patch, the macOS `NSTextInputClient` completion, if macOS users need it before W-P5 (`platform.md` §5) |
| cadatomic | adoption: the window loop, islands, shell chrome, panels, tests |

**GPUI code** is copied under Apache-2.0 with a per-file header, a `THIRD-PARTY.md`, and the
Microsoft MIT notice kept on the gamma functions (`round2/gpui-port.md` §1).

## 2. Work packages

Sizes are new or rewritten lines of Rust/JS, excluding tests. Where they come from:
- the porting inventories: `round2/svelters-inventory.md`, `round2/cadatomic-inventory.md`;
- GPUI's crate sizes;
- comparable code in winit, Chromium and Firefox.

They are planning estimates (±50%), not commitments.

### 2.1 Foundations (headless, cloud, no dependencies on each other except as noted)

| Id | Package | Repo | Size | Depends on | Gate |
|---|---|---|---|---|---|
| **W-F1** | `skelter-frame`: the UI and compositor state machines, the estimator, the injected clock, the deterministic simulator with display/present/GPU/input models and the fault cases (`round2/scheduler-latency.md` §2, §3, §6) | skelter | 3–4k | — | the simulator's invariants hold over 10k seeded runs of every fault case; frame-latency assertions (p99 ≤ 1 frame for in-budget native key presses) |
| **W-F2** | `skelter-anim`: cubic-bezier/steps/`linear()` curves, keyframe tracks with CSS value interpolation (transform lists, colours, lengths), Web Animations timing, closed-form springs with the Svelte mapping, scroll physics (wheel curves, flings, rubber band) per OS | skelter | 3k | — | Svelte's built-in transitions sampled at 60 Hz match Svelte's own JS within 1e-6; spring mapping matches Svelte's integrator at 60 Hz; fling curves match the cited sources' constants |
| **W-F3** | `skelter-core`: the tree moved from svelters-egui's scene (≈23k LOC that move nearly as is), value types, the flag-bit fix, stable kind/member ids with fingerprints, the `Kind` trait and registry, change classes, transactions and revisions, property trees, display-list types, hit lists, `Commit`, the event model (capture/bubble), the focus model, the AccessKit projection | skelter (+ svelters, the move) | 8–10k new + the move | — | svelters-egui's scene tests ported and green on skelter-core; property-tree and commit-delta unit tests; no egui in `cargo tree` |
| **W-F4** | `skelter-text`: parley/fontique/harfrust/skrifa; `TextMeasure` for taffy; ellipsis and tab stops; the shaped-paragraph, line-break, fallback and glyph caches; our area-coverage rasteriser with x-only AA and vertical hinting; the colour-glyph path; deterministic mode | skelter | 5–6k | — | exact CPU goldens (glyph runs, atlas tile bytes) for a bundled-font corpus; µs per label and per paragraph, bytes per layout (cloud numbers, marked loaded-machine) |
| **W-C1** | Stable integer ids: `spec.rs` emits stable ids with fingerprints and a spec hash; the compiler emits id constants; the runtime and host take ids; op logs keep names. **Also helps egui** | svelters | 1–1.5k | — | conformance unchanged (628/774); every `expected.oplog` byte-identical; the setProp microbench shows the id path |
| **W-C3** | Lazy `Native` objects and inline compares in `@svelters/native-runtime`; move the last-value dedupe into Rust columns. **Also helps egui** | svelters | 0.8–1.2k | W-C1 | `core-contract.test.mjs`, conformance and op logs unchanged; allocation count per counter mount drops (measured in the bench) |
| **W-C5** | Passive-listener inference and `LISTEN_PASSIVE`; `nonpassive_scroll_listener`; spec `passive`/`cancelable`. **Also helps egui** (the hit-test path can skip waiting) | svelters | 0.6–1k | — | compiler unit tests on handler shapes; `codes.rs` rows; conformance unchanged |
| **W-H1** | Host: a "dispatch and drain microtasks" entry; widget signals dispatched at event time instead of through the outbox. **Overlaps the egui agent's same-frame dispatch work:** one owner (§6) | svelters | 0.4–0.8k | — | an event's JS result is in the same frame (kittest frame counter); budgets enforced |

Batch suggestion: **one batch of 6–8 lanes:** W-F1, W-F2, W-F3, W-F4, plus W-C1, W-C3 and W-C5 if
the egui agent isn't already on them. W-H1 goes to whichever effort owns dispatch.

### 2.2 Skeleton and the go/no-go (M1)

| Id | Package | Repo | Size | Depends on | Gate |
|---|---|---|---|---|---|
| **W-R1** | `skelter-render`: Quad (with rounded clip), Shadow, Line, Mask, Color, Surface pipelines from GPUI's WGSL; instance-buffer ring; etagere atlases with LRU; BoundsTree batching; non-sRGB targets; headless render-to-image | skelter | 5k (of which ~3.5k adapted from GPUI) | W-F3, W-F4 | pixel tests on lavapipe within tolerance; draws per frame and µs per frame for a 10k-node scene |
| **W-K1** | `skelter-compositor`: the thread, commits and folding, fast commits, compositor animations and scroll physics at `T`, damage with scissored redraw (full present until P1), presentation feedback plumbing, the resize handshake | skelter | 4k | W-F1, W-F2, W-R1 | the simulator's invariants on the real compositor with a fake clock; a fling keeps display rate with the UI thread blocked 100 ms (cloud, lavapipe, frame counts) |
| **W-P1** | `skelter-platform` on winit 0.30.13 with the side-channel shims (`platform.md` §3.2): windows, input with timestamps and `InputSeq`, basic IME, clipboard (arboard, smithay-clipboard), surfaces created on main and moved, DX12 forced with the waitable on Windows, CAMetalLayer sublayers and CAMetalDisplayLink on macOS, the Wayland side queue (presentation feedback, subsurfaces, occlusion) | skelter | 4–5k | — | runs the counter demo on Linux (cloud, Xvfb/lavapipe); Windows and macOS **[local]** |
| **W-S1** | `svelters-skelter`: the backend ABI on skelter; `skelter.json` generated from the kind tables; the counter demo and a 10k-node tree; event-time dispatch | svelters | 3k | W-F3, W-C1, W-H1 | the counter fixture's op log equals egui's; the svelters examples that use only Tier 0 kinds run |
| **W-T1** | `skelter-test` v1: `Harness` with `run`/`step`/`get_by_label`/`click`/`type_text`/`key_press`/`render` over the AccessKit snapshot; injection at the dispatch point | skelter | 1.5k | W-F3, W-K1 | used by W-S1's tests |
| **W-L1** | Latency instrumentation in skelter: `InputSeq` stages `os → presented`, in-app histograms, Chrome-trace export with flows | skelter | 1.5k | W-F1, W-K1, W-P1 | every input in the demo terminates once; the trace opens in Perfetto |
| **W-J0** | The engine microbench: rquickjs closure vs raw `rquickjs-sys` C function vs exotic class, with strings and the `Map` dedupe; a full Svelte update with and without W-C1/W-C3; QuickJS GC pause against heap size | svelters | 0.5k | W-C1, W-C3 | **[local]** a quiet machine; results in the notes, then the fork question closes or reopens (`compiler-and-js.md` §5.6) |

**M1 gate (go/no-go)**, measured on the user's Windows machine and the MacBook **[local]**, and on
the cloud lavapipe image for frame counts. M1 passes when:
1. **Latency:** p50 click-to-present for the counter is at least 1 frame better than egui with the
   other agent's M0 fixes, on the same machine. Measured by W-L1 timestamps and PresentMon on
   Windows, checked once with the photodiode rig.
2. **Idle:** 0 frames and 0 wakeups over 10 s.
3. **One label change:** ≤ 0.5 ms UI-thread CPU, and the compositor draws only the label's damage.
4. **Under load:** a fling or a Svelte `fade` keeps display rate while JS is blocked for 100 ms.
5. **10k-node tree:** one change costs ≤ 1 ms of UI CPU; a full rebuild is reported.
6. **Text:** a screenshot review at 96 DPI on Windows and on Retina, against egui and against
   native apps.

If M1 fails on latency or text, the plan stops and the findings go to the user. **No later package
starts before this gate.**

### 2.3 Widgets, interaction and the compiler (M2)

| Id | Package | Repo | Size | Depends on | Gate |
|---|---|---|---|---|---|
| **W-W1** | Interaction substrate: the 13 interaction units re-hosted on skelter `Event`s, keeping egui's click semantics; pointer capture; hover; focus with `contain`/`dialog` scopes and `:focus-visible` | skelter | 4k | M1 | svelters' `events`, `focus_keys`, `ow1a*` tests ported and green |
| **W-W2** | Tier 0 kinds re-hosted (box, label, button, checkbox, radio, slider, progress, icon, image, separator, …) | skelter | 6k | W-W1 | `tier0`, `widgets` tests ported and green; screenshots |
| **W-W3** | Scroll and layers: scroll-area (with latching), panel, split-view, virtual-list, tree-view, popups, menus, select, inner-window, toasts, tooltips | skelter | 8k | W-W1 | the scroll, virtual-list and menu tests ported; the 1 s fling with blocked JS |
| **W-W4** | Text input: text-edit on quire-text through `TextInputClient`; IME-first key routing; value boxes, the table cell editor, the colour hex field; quire-text gains UTF-16 offsets and bidi motion | skelter + quire | 5k | W-W1, W-F4, W-P2 | the `egui_measure` pins 1–7 re-measured; IME acceptance per OS **[local]** (Japanese, Chinese, Korean, dead keys, the macOS accent popup, dictation) |
| **W-W5** | The rest of Tier 1: table, pickers, a svelters-drawn tab-view, DnD with the island bridge, zoom, dialogs, multi-window | skelter | 8k | W-W3 | the remaining svelters-egui test files ported |
| **W-A1** | Accessibility: the validating builder, platform adapters directly, the text patterns | skelter | 2.5k | W-W2 | the AccessKit snapshot tests; NVDA, Narrator, VoiceOver and Orca smoke tests **[local]** |
| **W-C2** | Native templates: the `Item::Template` IR, `$n.instantiate`/`destroy_range`, host and ABI ops, op-log expansion, HMR registration and refcounts, `source-loc` mapping | svelters + skelter | 2.5–3.5k | W-S1 | op logs byte-identical with the capability on; counter build crossings 49 → ≤ 14; HMR tests; DZ0 designer tests |
| **W-C4** | Change classes in the compiler: write sites, `class:` toggles from scoped-rule analysis, `style:` directives; the layout-from-per-frame-source warning | svelters | 1k | W-C1 | compiler tests; a toggle that changes only colour records without relayout (skelter counter) |
| **W-C6** | Native transitions, `animate:flip` and `svelters:motion` | svelters + skelter | 2.5k | W-F2, W-K1 | transition sampling equals Svelte's JS within 1e-6 at 60 Hz; FLIP scale bug fixed; transitions keep display rate with JS blocked |
| **W-C7** | CSS precompilation: binary sheets, interned utility sets, rule-candidate hints, static matches; dev keeps the runtime path | svelters | 2.5k | W-C1 | style tests and the visual-conformance ratchet unchanged; restyle µs on the 10k tree drops |
| **W-C9** | Accessibility diagnostics from the spec's a11y fields | svelters | 0.5k | W-C1 | `codes.rs` rows; `cx1_*` tests |
| **W-J1** | Hot ops as raw `rquickjs-sys` C functions with `magic` ids; typed setters emitted by the compiler; lazy wrappers holding (id, generation) with GC marks for handlers | svelters | 1.5k | W-J0, W-C1, W-C3 | W-J0's microbench shows the gain; leak tests (handler capturing its node) |
| **W-P2** | Platform IME and text in phase 1: the Windows shim's `WM_IME_REQUEST` and `ImmNotifyIME` cancel/commit; the Wayland tear-off experiment (`platform.md` §3.3); the upstream batch (#4150, #4708, #3442, #2334); the macOS bridge fork patch only if needed | skelter (+ winit upstream) | 1.5–2.5k | W-P1 | IME acceptance per OS **[local]** |

**M2 gate:**
- svelters' egui conformance, widget and visual-conformance tests ported and green on skelter;
- the screen-reader smoke test per OS **[local]**;
- IME acceptance **[local]**;
- M1's latency numbers hold with the full widget set.

### 2.4 cadatomic's shell on skelter (M3)

| Id | Package | Repo | Size | Depends on | Gate |
|---|---|---|---|---|---|
| **W-I1** | `skelter-egui-island`: egui on skelter's device, input translation, AccessKit graft by `TreeId`, the `egui::DragAndDrop` bridge, repaint-on-request | skelter | 2k | M2 | the whole cadatomic app runs inside one island (smoke test) |
| **W-B1** | `skelter_berth`: the dock renderer on skelter; global pointer capture; tear-off on Windows and macOS, and on Wayland through `xdg-toplevel-drag-v1` (W-P2's experiment) | berth + skelter | 5k | W-I1, W-W3, W-P2 | berth's own tests on the new renderer; tear-off on Windows and Wayland **[local]** |
| **W-D1** | cadatomic adoption: skelter owns the window, device and frame loop; `adapter::select` and the limits ported; the shell chrome native (menus, title bar, ribbon, status, edge panels, the palette modal); keyboard pre-dispatch through the core keymap (egui `Key` names round-trip); `CancelClose`; every pane an island | cadatomic | 6–8k | W-I1, W-B1 | cadatomic scenarios and tours green; idle barrier tests |
| **W-T2** | `skelter-test` v2: every kittest verb cadatomic uses, the `egui::Event`-shaped injection adapter, the tour live driver and the pixel tier re-targeted, egui_mcp/egui_inspection parity through svelters-automation | skelter + cadatomic + svelters | 3k | W-T1, W-D1 | cadatomic's 45 kittest files run on the harness (the panes still in islands) |

**M3 gate:**
- scenarios and tours green;
- the MCP pixel tier works;
- tear-off on Windows and Wayland **[local]**;
- M1's numbers hold for shell interactions (tab switch, palette open, menu open).

### 2.5 Layers, canvases and the present path (M4)

| Id | Package | Repo | Size | Depends on | Gate |
|---|---|---|---|---|---|
| **W-G1** | The wgpu fork: P1 `present_with_damage` (tracking upstream PR #10152), P2 acquire timeout, P3 Metal drawable from CAMetalDisplayLink, P4 no off-main NSView access; the Vulkan chain-hook backport | wgpu fork | 0.6–0.8k | M1 | each patch with a test; upstream PRs opened |
| **W-V1** | Viewports and canvases on OS layers: DComp child visuals, CAMetalLayer sublayers, Wayland subsurfaces; the per-OS resize handshake; fallback to Surface primitives | skelter + cadatomic | 3k | W-G1, W-D1 | orbit never redraws the UI (counter); resize without stretched frames **[local]** |
| **W-V2** | Native canvas layers: typed instance chunks, culling and LOD, camera-relative coordinates, static cache, overlay layer; Slug canvas text | skelter + cadatomic | 6k | W-V1, W-F4 | PCB pan at display rate with 10⁶ primitives **[local]** |
| **W-R2** | Renderer measurements and options: the depth pre-pass, tile caches for big scroll layers, paths in batch-sized MSAA targets | skelter | 1.5k | W-R1 | adopted only where a measurement on lavapipe and an iGPU shows a gain **[local for iGPU]** |

**M4 gate:**
- present with damage on Windows and Wayland, checked with PresentMon on Windows **[local]**;
- canvases at display rate **[local]**;
- idle GPU at 0.

### 2.6 Own platform code (M5)

Detailed in `platform.md` §5. In order:
- **W-P3:** the Windows hooks, then the own Windows backend.
- **W-P4:** Wayland.
- **W-P5:** macOS.
- **W-X1:** the LCD text option, behind a setting, after a screenshot review.

Each lands behind the platform trait with winit kept for everything else; X11 stays on winit. Sizes: W-P3 Windows 9–12k
(own window procedure and modal-loop policy, `WM_POINTER` with history and QPC timestamps, TSF, DirectManipulation,
DComp, an OLE drag source), W-P4 Wayland 6–8k (text-input-v3, toplevel drag, locked surface state), W-P5 macOS 7–9k
(our own NSView with the full `NSTextInputClient`, pen, the AccessKit host view).

**M5 gate:**
- **Windows:** PresentMon shows "Independent Flip" or "Composed: Flip" for the main window, and modal
  resize keeps presenting **[local]**.
- **macOS:** IME, dictation and the Character Viewer work **[local]**.

### 2.7 Port the panels, remove egui (M6)

| Id | Package | Repo | Size | Depends on | Gate |
|---|---|---|---|---|---|
| **W-Q1** | `skelter_quire`: the editor renderer on skelter-text (`parley_engine` per line, quire's ASCII fast path), IME, scroll layer with tiles if measured | quire + skelter | 6–8k | W-W4, W-R2 | quire's E1 numbers rerun (p99 ≤ 8.3 ms) **[local]** |
| **W-D2** | Port cadatomic's Rust kinds (`property-grid` and `code-editor` first), then the Rust egui panels by traffic: inspector, palette, library, ECAD panes (board/sheet first), then the tour and designer overlays | cadatomic | 15–25k touched | M3, W-Q1 | islands remaining → 0; egui and eframe gone from `Cargo.lock` |
| **W-S2** | Freeze svelters-egui as a maintenance backend like GTK, Qt, AppKit and WinUI | svelters | — | W-D2 | the egui backend still builds and passes its conformance in the daily check |

## 3. Critical path and parallelism

```text
W-F1 ─┐
W-F2 ─┼─▶ W-K1 ─┐
W-F3 ─┼─▶ W-R1 ─┤
W-F4 ─┘         ├─▶ W-S1 ─▶ M1 gate ─▶ W-W1 ─▶ W-W2/W-W3/W-W4 ─▶ M2 ─▶ W-I1 ─▶ W-B1/W-D1 ─▶ M3 ─▶ W-V1 ─▶ M4 ─▶ W-D2 ─▶ M6
W-C1 ─▶ W-C3 ───┤                                │                                              ▲
W-H1 ───────────┘                                └▶ W-C2, W-C4, W-C6, W-C7, W-J1 (parallel)     └─ W-P3..5 (M5) in parallel from M2
W-P1 ─────────────▶ (M1)                         W-A1, W-P2 (parallel)                          W-G1 from M1
```

**Width.** After the foundations batch (6–8 lanes), each milestone has 4–7 independent lanes. The
widget packages split by family, the compiler packages by feature, and the platform packages by OS.

**Local work.** [local] packages queue in REVISIT.md as each milestone's cloud part lands, so the
user's machines are never on the critical path for longer than a gate's measurement session.

**Overall size**, as a planning scale rather than a schedule:
- about 70–90k lines new or rewritten across skelter, svelters, berth and quire;
- about 23k lines moved;
- 15–25k lines of cadatomic touched in the panel port.

The largest single risks are not the renderer but the long tail (§5).

## 4. What stays the same

- **svelters:** the backend ABI (extended, not replaced), the module loader and HMR model,
  svelters-style as the CSS engine, `settle` and wait-idle semantics.
- **The cores:** berth-core and quire-core/-text.
- **cadatomic's architecture rules:** one command path (keymaps through the core), headless core,
  ids not labels, the recording rule, jobs.
- **The other svelters backends,** kept at maintenance.

## 5. Risks

| Risk | Where it bites | Mitigation |
|---|---|---|
| IME and text input on each OS | W-W4, W-P2, M5 | IME acceptance is a gate, not a report; the Windows `WM_IME_REQUEST` shim comes in phase 1, and a one-patch macOS bridge fork is ready if needed before W-P5; quire-text is already a full editing model |
| Text quality at 96 DPI | M1 | screenshot review against egui and native at M1; skrifa's DirectWrite raster policy; the LCD option planned (W-X1) |
| The interaction semantics drift and ~1,000 kittest assertions fail | W-W1 | units keep egui's click semantics on purpose; tests port first, behaviour second |
| Accessibility panics and focus bugs (Zed's history) | W-A1 | the validating builder; our own NSView on macOS |
| wgpu and winit churn | W-G1, W-P* | wgpu: small, upstreamed patches on a named tag (`forks.toml`, the 8-week base-age check); winit: no general fork, stay on the stable 0.30 line behind the trait, own backends replace it per OS (0.31→0.32 churn: 385 files, +47k/−32k) |
| Two efforts touching the same svelters files (this plan and the egui agent) | W-C1, W-C3, W-C5, W-H1 | one owner per package (§6) |
| Migration stalls with half the app in islands | M3–M6 | the islands-remaining ratchet; panels ported by traffic; every milestone ships a working app |
| The go/no-go is decided by feel | M1 | measured gates with the rig; the user looks at the numbers and the screenshots |
| A JS engine fork turns out to be needed | W-J0 | measured first; reopened only on the stated threshold (`compiler-and-js.md` §5.6) |

## 6. Coordination with the egui latency work

**State of that work.**
- **Where it lives:** cadatomic branch `claude/input-latency` and svelters `ci/input-latency`, with
  the note `docs/notes/2026-10-25-input-latency.md` on that branch.
- **What it found:** seven sources of frames on egui, with deterministic frame counts:
  - `onclicked`/`bind:` take 2 frames against 1 for DOM `onclick`;
  - a wheel notch takes 117 ms to reach 90 % (egui's easing);
  - menus and popups add 1 frame (egui's `Area` sizing pass);
  - the palette takes 3 frames.
- **Its conclusion:** with same-frame reruns, "pass cost is now what limits latency". One shell pass
  costs 6–10 ms in release in a container, so a 3-pass frame costs 20–45 ms. That is the structural
  cost skelter removes (architecture §1–§2), measured independently.
- **Status:** the user deferred it on 2026-10-25 ("Defer", REVISIT.md: input latency). Fix #1
  (`LOW_LATENCY`) may land meanwhile.

**Consequences for this plan.**
- **Ownership:** W-C1 (ids), W-C3 (lazy nodes), W-C5 (passive) and W-H1 (event-time dispatch) belong
  to this plan, and they help egui too. If the egui work resumes, the coordinator gives each package
  to one owner and records it in the batch notes.
- **Files that overlap:** `SVELTERS_EXP_DISCARD` touches svelters `frame.rs` and
  `widgets/scroll_area.rs`, the same files W-H1 and W-F3's scene move touch. W-F3's move happens in a
  consolidation after any open svelters latency branch has merged or been dropped.
- **Shared measurements:** the frame-count probes (`crates/svelters-egui/tests/input_latency.rs`,
  `crates/cadatomic-app/tests/input_latency.rs`, the `latency-egui` fixture) are the right baseline.
  M1 runs the same interactions on skelter and compares frames and pass cost.
