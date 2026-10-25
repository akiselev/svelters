# Local pipeline map: svelters (1d6656c2) + cadatomic (0a1c345), egui 0.37

Read-only investigation, nothing built or run. Paths: `S/` = /home/user/svelters, `C/` = /home/user/cadatomic.
egui/eframe/winit sources are NOT in the local cargo registry, so statements about egui/eframe internals come
from the repos' own research notes (cited) and are marked [notes]. My own inferences are marked [inference].

---------------------------------------------------------------------------------------------------
## 0. Headline: the latency chain (click or key -> pixel showing the JS result)

| # | Stage | Where | Cost |
|---|---|---|---|
| 0 | OS input -> winit -> egui-winit accumulates `RawInput`; eframe schedules a redraw; eframe never polls (`ControlFlow::Wait`/`WaitUntil`) | eframe internals [notes: C/docs/best-practices/performance.md:100-106, 240-251] | waits for the next `RedrawRequested`; up to ~1 vsync [inference] |
| 1 | Frame N, begin of pass: svelters' `EventsPlugin::input_hook` hit-tests last pass's rects and runs DOM-style listeners (`onclick`, `onpointerdown`, `onkeydown` …) **synchronously** via `call_sync` | S/crates/svelters-egui/src/events.rs:15-31 (doc), 595-599 (hook), 778-858 (`dispatch_input`), 1054-1088 (call_sync) | 0 extra frames. call_sync does not drain microtasks (S/plans/architecture/backend-abi.md:320-321); leftover jobs -> `wake` -> flag + `request_repaint` (S/crates/svelters-egui/src/ops.rs:377-388) consumed by the same frame's logic (step 2) [inference from ordering in S/plans/research/ARCH-EMBED.md:402-427] |
| 2 | Frame N, `App::logic`: cadatomic `ShellApp::logic` -> `drain_and_pump` -> `svelte.panel.logic` -> `AppEmbed::logic` -> `Embed::logic` -> `logic_pass` -> `before_pass`: checkpoint only if first/wake/timer due/checkpoint_pending, then rAF, list binds, state providers, restyle, transitions, remeasure | C/crates/cadatomic-app/src/app.rs:1324-1336, 4104; C/crates/cadatomic-ui/src/app_embed.rs:391-395; S/crates/svelters-egui/src/embed/mod.rs:832-893; S/crates/svelters-egui/src/frame.rs:158-165, 184-261 (checkpoint gate at 214-221) | JS time (budget warn 8 ms / interrupt 50 ms: embed/mod.rs:78-81) |
| 3 | Frame N, `App::ui`: dock -> pane -> `Embed::show_slot` -> layout if dirty -> walk -> widgets/units read `egui::Response` (click on release) -> **widget signals** (`onclicked`, `changed`, `bind:` writes) go to the outbox `Out::Event/Changed` | S/crates/svelters-egui/src/embed/mod.rs:895-987; walk.rs:17-22, 90-296 | outbox only; no JS |
| 4 | End of slot/pass: `after_pass` flushes outbox as `post_event`/`prop_changed` (queue only), sets `checkpoint_pending`, `request_repaint_of(logic_vp)` | S/crates/svelters-egui/src/frame.rs:402-431; host side S/crates/svelters-host/src/lib.rs:125-132 (post_event -> push_event + wake) | schedules frame N+1 |
| 5 | Frame N tessellated, painted, presented (shows egui's own press/hover/typed-text feedback, but not the JS result) | eframe [notes: performance.md:100-106] | 1 frame |
| 6 | Frame N+1 (requested "right after" N): logic -> checkpoint -> JS handler -> Svelte microtask flush -> ops mutate the scene directly (no batching) -> restyle/remeasure -> walk -> tessellate -> present | S/crates/svelters-host/src/core.rs:1059-1150 ("Ops reach the backend directly while JS runs (no batching in P)", 1071-1072); S/docs/notes/egui-scene-graph.md:216-226 | **+1 frame (~1 vsync) vs GTK** (S/plans/research/EGUI.md:75; S/docs/notes/egui-scene-graph.md:218-221) |
| 7 | Presentation queue: eframe default `SurfaceConfig::LOW_LATENCY` (AutoVsync, `desired_maximum_frame_latency=Some(1)`) | [notes: C/docs/best-practices/performance.md:28-32, 288-299] | measured eframe input-to-photon ~2 frames (glow) / ~3 frames (wgpu) at 120 Hz = 33-50 ms at 60 Hz (performance.md:33-36, 301-308; egui issue #7761 per the note) |
| 7b | Extra frames that some interactions add | own button focus -> `root().focused` -> one more logic frame (S/plans/milestones/OW0-widget-foundation.md:337-340; test allows 1..=4 root frames, S/crates/svelters-egui/tests/viewports.rs:214); slot resize -> repaint (embed/mod.rs:977-981, 1040-1043); walk-time restyle that shows next frame (frame.rs:437-440); `use:` bits one frame late (S/STATUS.md:94 OW1a4 gaps) | +0..2 frames |

**Two event paths with different latency** (important for the decision):
- DOM-style `onclick`/`onpointerdown`/`onkeydown`/`onwheel` on any element: dispatched in the input hook *before*
  egui's pass, JS runs in frame N (events.rs:15-31). Same-frame JS.
- Widget signals (`onclicked`, `onchanged`, `bind:value`, text-edit `text` changes, slider `input`): produced by
  the walk, delivered at the next frame's checkpoint (frame.rs:402-431; S/docs/notes/egui-js-widgets.md:199,522).
  cadatomic's Svelte UI uses the slow path almost exclusively: `onclicked` 66 times vs `onclick` 2 times,
  `onchanged/oninput/bind:text/bind:value` 24, `onkeydown` 4 (grep over C/ui/**/*.svelte, 26 files).

**Rough total for a button `onclicked` on 60 Hz with eframe+wgpu** [inference from the rows above]:
input wait (0-1) + frame N (1) + frame N+1 (1) + swapchain/compositor (~1-2) ≈ 3-5 vsyncs ≈ 50-85 ms, before any
CPU overrun. If one frame's CPU+GPU exceeds 16.7 ms, each of N and N+1 slips a vsync. The cloud harness measured
the egui pass alone at ~10.8 ms p50 / 14.8 ms p95 on llvmpipe (§2.5), and the main dev box (WSL PC) has **no GPU
driver: Vulkan is lavapipe** (C/CLAUDE.md "WSL: the machine is WSL2 (WSLg display, no GPU Vulkan driver; Vulkan is
lavapipe)"). On that machine frames N and N+1 are both near or over budget, so the JS-visible result plausibly
lands 4-7 vsyncs after the click [inference]. This is likely a major part of the "sluggish" feel and is
independent of egui vs a custom renderer (a CPU rasteriser pays per pixel either way).

`request_discard` / same-frame JS: designed but **not built**. `request_discard` is used nowhere in production
code (only C/crates/cadatomic-app/tests/surfaces.rs:99,137). Option described in S/docs/notes/egui-scene-graph.md:
222-226, 465 and S/plans/research/EGUI.md:75 ("Measure in a windowed demo before choosing"). No such measurement
exists in either repo.

---------------------------------------------------------------------------------------------------
## 1. Frame loop end to end

### 1.1 Who owns the event loop
- **cadatomic: eframe owns it.** `eframe::run_native("cadatomic", NativeOptions{viewport, wgpu_options, ..Default::default()}, …)`
  (C/crates/cadatomic-app/src/main.rs:35-89). `ShellApp` implements `eframe::App { logic, ui, on_exit }`
  (C/crates/cadatomic-app/src/app.rs:5866-5878). Renderer is wgpu (eframe features `wgpu`, C/Cargo.toml:42).
- **svelters' own desktop runner is also eframe**, not a custom winit runner: `runner/eframe.rs` builds
  `NativeOptions { renderer: Glow, ..Default }` and calls `eframe::run_native` (S/crates/svelters-egui/src/runner/eframe.rs:61-74).
  `DesktopApp::logic` -> `embed.logic(ctx)`, `ui` -> `embed.show_windows(ui)` (runner/eframe.rs:84-102). The
  legacy `EframeApp::ui` -> `EguiBackend::ui` (runner/eframe.rs:33-43; frame.rs:133-153).
- `runner/` has three runners: eframe, embed (host owns loop; cadatomic's mode), kittest (headless)
  (S/crates/svelters-egui/src/runner/mod.rs:1-12). There is no custom winit runner; it was discussed as a fallback
  only (S/docs/notes/egui-scene-graph.md:19, 178, 466; ARCH-EMBED.md:440-443 "A custom runner could avoid it, but
  that stays deferred (SCOPE §6.1)").
- cadatomic links svelters-egui with `default-features = false` (no eframe/glow runner, no kittest) so features
  unify with cadatomic's wgpu (C/crates/cadatomic-app/Cargo.toml:41-43; C/crates/cadatomic-ui/Cargo.toml:23-27).

### 1.2 winit -> RawInput
- Done entirely by eframe/egui-winit; neither repo touches winit events. svelters only depends on egui-winit
  for its `clipboard` feature (S/crates/svelters-egui/Cargo.toml: egui-winit `=0.37.0` features clipboard/x11/wayland).
- svelters intercepts `RawInput` through an `egui::Plugin::input_hook` (events.rs:589-606), installed once per
  context (events.rs:564-580). It can remove consumed / default-prevented events before egui sees them
  (events.rs:832-835, 845-848).
- `pointermove` is coalesced to one per pass (events.rs:28, 791-799).

### 1.3 Where JS runs relative to the egui pass
- Phases (ARCH-SCENE R17): `Idle`/`Logic`/`Ui` (S/crates/svelters-egui/src/scene/mod.rs:73-84).
- `Embed::logic` runs only on pass 0 and once per (viewport, cumulative_pass_nr) (embed/mod.rs:832-850);
  steps: apply queued commands -> sync info -> zoom keys -> `logic_pass` (checkpoint, rAF, remeasure) -> collect
  faults -> apply commands -> `after_pass` -> route repaints (embed/mod.rs:851-893).
- `EguiBackend::before_pass` order (frame.rs:184-261): follow zoom; `run_deferred` scene writes; asset changes;
  announcements; native closes (sync `close-request` via call_sync, frame.rs:361-376); **checkpoint if
  first||wake||due||pending** (frame.rs:210-221); `host.frame()` if rAF pending (222-226); list realize (call_sync
  binds, 228); table cell editors (230); state providers (234-242); announcements; theme; restyle; CSS transitions;
  native animations; remeasure (253-260).
- `show_slot` never runs JS; a `pass_active` flag turns any checkpoint/call_sync during the walk into a panic
  (walk.rs:25-27; frame.rs:15-16; embed/mod.rs:942-944).
- Deferred viewports: an `on_begin_pass` plugin can run `logic` before a deferred viewport's pass
  (embed/current.rs:9-14, 151-184; embed/mod.rs:1076-1098). **cadatomic turns it off**: `prepass_plugin: false`
  ("cadatomic owns the per-frame order (seam item 4)") (C/crates/cadatomic-ui/src/app_embed.rs:459-463). So input in
  a deferred window costs a root-viewport frame (embed/current.rs:12-14; ARCH-EMBED.md:438-443).
- Off-thread wakes always request a ROOT repaint, then logic fans out (ops.rs:377-388; ARCH-EMBED.md:438-443).
- `raw_input_hook` (eframe App method) is NOT used; svelters uses the egui plugin `input_hook` instead.
  `request_discard`: not used (see §0).
- eframe always runs `ui()` after `logic()`: every JS timer tick costs a full frame (walk + tessellate + present)
  even if nothing changed (S/docs/notes/egui-scene-graph.md:177-178; EGUI.md:79).

### 1.4 Repaint scheduling details that add frames
- `after_pass` asks a repaint when checkpoint/frame/wake pending, closes, animations, transitions, restyle, deferred
  writes; timers via `request_repaint_after_for` (frame.rs:425-448).
- cadatomic's `wait_idle` frame waker uses `request_repaint_after(1 ms)` because "a zero-delay request_repaint from
  outside a frame runs two frames" (C/crates/cadatomic-app/src/app.rs:4015-4026); svelters' waker keeps the zero-delay
  `request_repaint` "because it needs the latency" (ARCH-EMBED.md:491-494, D12 at :1082).
- cadatomic also asks `request_repaint_after(50 ms)` in places (app.rs:1696) and per source-tab deadlines (app.rs:4082-4087).

### 1.5 Present mode / frame latency actually used
- **cadatomic-app**: `wgpu_options: viewport::adapter::wgpu_options()` which returns
  `egui_wgpu::WgpuConfiguration { wgpu_setup, ..Default::default() }` (C/crates/cadatomic-app/src/viewport/adapter.rs:84-96)
  — only the adapter selector and device limits are customised. No `present_mode`, no
  `desired_maximum_frame_latency`, no `set_wgpu_surface_config` anywhere in C/crates (grep). Per the repo's own
  research, egui-wgpu's `WgpuConfiguration::default()` is `HIGH_THROUGHPUT` (AutoVsync, Some(2)) but eframe's
  `NativeOptions::default()` overrides to `LOW_LATENCY` (AutoVsync, Some(1)) (performance.md:288-291).
  CAVEAT [inference]: cadatomic sets `wgpu_options` explicitly from `WgpuConfiguration{..Default::default()}`; if
  the LOW_LATENCY override lives in `NativeOptions::default()`'s `wgpu_options` field, this explicit value replaces
  it and cadatomic would actually run `HIGH_THROUGHPUT` (Some(2)). Verify against eframe 0.37 source; the
  research's wording ("NativeOptions::default() overrides it") suggests the override is in that field.
- MSAA: default (none) for egui; viewport renders its own 4x MSAA offscreen (C/crates/cadatomic-app/src/viewport/gpu.rs:24-25).
- **svelters-egui runner**: Glow renderer, everything else default (runner/eframe.rs:68-72): vsync per eframe
  default; `desired_maximum_frame_latency` is ignored by GL (performance.md:283-284).
- Plans: "Keep AutoVsync; start with eframe's low-latency default and measure the high-throughput setting while
  orbiting; never ship Immediate" (C/SCOPE.md:617-619); PF-M3/M4/M11 "input-to-present p95 at or below 50 ms at
  60 Hz; Fifo against Mailbox" — status "A5b active", not measured (C/docs/plan/05-experiments-and-research.md:88);
  G0-14 DX12 vs Vulkan input latency is Windows-only (05-experiments…:97).

---------------------------------------------------------------------------------------------------
## 2. Per-frame costs

### 2.1 What happens every frame
- Every frame walks the **visible** part of every shown slot and re-emits egui widgets/shapes; whole subtrees are
  culled by ink bounds against the clip rect (walk.rs:270-286; `culled` is the one predicate, 273-275).
  Off-screen leaves cost nothing; containers are tested (walk.rs:205-209, 229-231).
- Every leaf gets a child `Ui` (`ui.new_child(UiBuilder…scope_id(id.with("scope")))`, walk.rs:232-240) and per-leaf
  CSS state/visuals (`style::before_leaf`, walk.rs:242-244); every container records focus scope and hit box
  (walk.rs:210-214).
- egui re-tessellates every shape every frame by design (paint_cache.rs:8-9; ARCH-SCENE.md:1032-1037 citing
  egui context.rs:3358; performance.md:127-129, 362-369).
- egui repaints the whole window; no damage regions (performance.md:19-23, 330-341).
- AccessKit nodes are rebuilt by egui each frame when AccessKit is active (ARCH-SCENE.md:1018-1020).

### 2.2 Caching that exists
- **Layout (taffy 0.15, directly on the scene columns, no mirrored TaffyTree)** (S/crates/svelters-egui/src/layout.rs:1-10,
  trait impls 475-790). Runs only for queued work: whole root when new/resized, else dirty layout boundaries at
  their last size (layout.rs:8-10; walk.rs:31-40; scene/dirty.rs:1-15, 64-80). Clean check ≈ 1 µs.
  Leaves measured with egui fonts (`fonts_mut(...layout_no_wrap)`, layout.rs:290-322); intrinsic size cached in the
  node until text changes (scene/node.rs:395; dirty.rs:110-120). Interned taffy styles and common props (scene/mod.rs:96-99).
- **Text galleys**: egui's own per-viewport galley cache, keep-if-used-last-pass (ARCH-WIDGETS.md:78; performance.md:45-48, 119).
  No per-node galley cache (suggested as optional, egui-scene-graph.md:74).
- **Shapes**: opt-in `WidgetCx::cached_shapes` — `Arc<[Shape]>` per node keyed (version, size, ppp, dark); hit
  replays shapes translated; saves construction + text layout, not tessellation (S/crates/svelters-egui/src/scene/paint_cache.rs:1-71).
- **GPU canvases (cadatomic)**: render key per view; unchanged key renders nothing (C/crates/cadatomic-app/src/canvas2d/gpu.rs:14);
  motion frames, partial scissored re-renders, bucket patch uploads (C/docs/notes/batch6-reports/perf.md:58-157).

### 2.3 svelters measurements
- Probe (headless, no GPU, Ryzen 7 5700U WSL2, load 10-35, "noisy upper bounds") — S/docs/notes/egui-scene-graph.md §10:
  - full walk ≈ 5 µs/node: 1,101 nodes 4.14/4.26 ms; 11,001 nodes 55.8/49.9 ms; 55,001 nodes 286.8/289.4 ms run_ui (lines 236-250).
  - culled (≈375-386 visible nodes on 1280×800): 1.78-3.49 ms run_ui + 0.64-1.24 ms tessellate, independent of total size (238-256, 260-264).
  - AccessKit on: +1-2 ms per screen (+50-90%); per-leaf scopes +0.8-1.1 ms (265-266).
  - layout: cold 24.5-30 ms (1.1k), 163 ms (11k), 806-859 ms (55k); one label changed 1.5-2.3 / 8.3-9.2 / 43-53 ms;
    clean check 0.8-1.3 µs; width −200 px 5.0-5.7 / 64-74 / 365-418 ms (lines 220-224). Excludes GPU/eframe/winit (268).
- SG6 (merged; S/plans/milestones/SG6-scene-memory-culling.md:21-26; S/STATUS.md:91): 10k-node steady frame
  0.77-0.93 -> 0.29-0.45 ms; scrolling 1.01-1.37 -> 0.41-0.77 ms; relayout after a label change 8.7-9.9 -> 3.7-5.3 ms;
  first run 155-185 -> 84-111 ms; paint cache 1000 widgets × 25 shapes: 23.7 ms uncached -> 14.4 ms cached
  (18.5 with tessellation); culling visits 220/10,008 nodes; 500 B/node (640 B budget).
- Startup: counter-egui release first frame 93 ms from source, 23 ms precompiled bundle (S/STATUS.md:97).
- Text: TC-EVAL "100 typed characters cost 0.7-1.2 ms in total" (S/plans/research/TC-EVAL.md:26).
- `S/bench/keyed-bench`, `keyed-bench-qt`: Svelte keyed-list apps + scripted click/checkpoint op scripts
  (bench/keyed-bench/script.json); no recorded egui numbers found.
- Frame instrumentation exists: `FrameMonitor` (`SVELTERS_HANG_MS`, `SVELTERS_MEASUREMENTS=1`, `tools/dev/frame-histogram`)
  (S/plans/milestones/ZU3-ui-thread.md:100-110; runner/eframe.rs:28-42); `EmbedStats.last_logic/max_logic` (embed/mod.rs:886-891).
  No recorded histogram of a real app found.

### 2.4 cadatomic research doc (C/docs/best-practices/performance.md, research, not measured: line 3-5)
- egui 1-2 ms per frame README claim (118); tessellation counts (120); no reuse across frames (127-129);
  window = unit of repaint (330-351); present modes table (270-278); latency table/quotes (301-308):
  "glow about 2 frames (~17 ms), wgpu about 3 frames (~25 ms)" at 120 Hz; #1904 "1-2 frames of extra lag versus a
  custom canvas"; Kalbertodt 2023: egui 2 frames vs 3 for iced and Tauri.
- Targets: M13 UI pass < 4 ms (455), latency 50 ms input-to-present (33-36).

### 2.5 cadatomic measurements (cloud: 4-core container, llvmpipe Vulkan, 1920×1080, release; "not a gate")
- E4 frame budget (C/docs/notes/batch6-reports/perf.md:20-56; perf/e4-final.md):
  - pan whole board p50/p95 14.0/18.7 ms (baseline 342.8/424.3); zoom stress 42.5/194.9; zoom every 6 frames 13.7/19.8;
    hover 84.9/168.9; drag ghost 21.9/32.2; drop -> new scene 1.42-1.74 s.
  - Pan stages p50/p95: **egui pass (encode + GPU) 10.84/14.76 ms**; app step (logic+ui) 2.37/3.48; logic 0.86/1.36;
    ui 1.26/1.84; shell_regions 0.98/1.44; publish_bottom 0.76/1.22; tessellate 0.21/0.34; canvas GPU 0.17/0.32
    (e4-final.md "Stages of pan").
  - "The floor left is the egui pass (≈ 11 ms of window chrome under llvmpipe)", "3.7 ms of it is the dock area
    filled under the opaque canvas" (perf.md:18, 52-56, 211). DRC markers as egui circle strokes were ~61k triangles,
    ~9 ms of the egui pass, replaced by textures (perf.md:44-45, 153-156).
- Marker latency (seam test through publish, cloud): 5 frames / 99 ms; undo 3 frames / 74 ms
  (C/docs/notes/batch4-reports/consolidation.md:118-119); 6 frames / 77 ms, undo 4 frames / 65 ms
  (batch4-reports/eu6.md:108-111); E6 marker 33 ms (C/STATUS.md:34).
- Router step latencies (seam, no pixels): push p50 138 µs / p95 3.9 ms (batch6-reports/eu7.md:58-70).

---------------------------------------------------------------------------------------------------
## 3. The retained scene

### 3.1 Structure
- One arena, parallel columns per slot: `core: Vec<NodeCore>`, `children`, `layout: Vec<LayoutBox>` (Arc<taffy::Style>,
  cache, layouts, intrinsic), `payload` (`W` enum or provider), `cold`, interned `common`, ink rects, paint cache,
  ext columns, journal, root table, kind indexes, per-viewport hit lists, panes, CSS `SceneStyle`
  (S/crates/svelters-egui/src/scene/mod.rs:86-140). File map: scene/mod.rs:8-24.
- `NodeCore` ≈104 B: parent, kind, root, flags, dirty, owner, state bits, listen mask, explicit mask,
  **`egui_id: egui::Id`, `rect`/`drawn: egui::Rect`**, changed tick (scene/node.rs:340-380; ARCH-SCENE.md:102-105).
- `W` enum of built-in payloads (Window, Box, Label, Button, Checkbox, TextEdit, Ext(provider), VirtualList, …)
  (scene/node.rs:17-79).
- Ids: `NodeId(NonZeroU64)` = `gen << 24 | idx`, generation retired at 2^28 so ids < 2^52, equals ABI handle
  (scene/id.rs:1-60). egui `Id` derived from embed salt + root key, not from the `Ui` (embed/mod.rs:15-17; ARCH-EMBED.md:52-57).
  Four identities: NodeId, egui Id, anchor, later D4 identity; `VNodeId{host,key}` for painted rows (ARCH-SCENE.md:106-107).
- Dirty bits `STYLE|MEASURE|LAYOUT|PAINT|A11Y|CHILDREN|DESC_STYLE|DESC_LAYOUT` (scene/node.rs:305-325); propagation
  only to the nearest layout boundary, per-root queues `relayout`/`remeasure`/`full` (scene/dirty.rs:21-31, 40-120;
  ARCH-SCENE.md:785-809).
- Flags (scene/node.rs:252-302). **Side finding: bit collision** — `SCROLL_CENTER = 1 << 21` (node.rs:290) and
  `INK_OVERFLOW = 1 << 21` (node.rs:296). layout.rs:1004-1016 toggles INK_OVERFLOW; widgets/scroll_area.rs:131,171-172
  set/clear SCROLL_CENTER on the same bit.
- Providers: `Widget` (measure / `ui(&mut self, ui: &mut egui::Ui, cx) -> egui::Response` / a11y with
  `egui::accesskit::Node` / hit_test / virtual_children) (S/crates/svelters-egui/src/provider/mod.rs:330-356);
  `Behavior` (sense -> `egui::Sense`, before_ui, after_ui(resp: &egui::Response), paint_over(&egui::Painter))
  (provider/behavior.rs:56-81); `StateProvider` (update on SceneMut, egui-free except ctx accessor) (provider/pass.rs:41-64);
  `ScenePass` (overlay(&mut egui::Ui), after_layout, order -> egui::Order) (provider/pass.rs:147-169).
  Counts: 76 `#[svelters::widget]` kinds in svelters, 8 in cadatomic (grep); ~58 Behavior/Unit impls in svelters-egui.
- Own widgets decision (user 2026-10-22): "svelters draws every widget; egui = painter, input, text shaping, layers,
  AccessKit only" (S/STATUS.md:94; S/plans/research/ARCH-WIDGETS.md:5-15). Paint primitives emit epaint shapes
  (ARCH-WIDGETS.md:71-110); behaviour units read `egui::Response` + `egui::InputState` (ARCH-WIDGETS.md:117-140;
  W2 "egui's Ui::interact stays the pointer picking, capture and focus-memory substrate", :45). Remaining egui
  widgets: `egui::TextEdit` for `text-edit` (leaf.rs:232), table cell editor (views/table.rs:2133), color editor
  field (widgets/color_editor.rs:911); OW4 moves text to TC0 `quire-text` (ARCH-WIDGETS.md:12-15, 507ff).

### 3.2 The ABI the JS runtime drives
- C vtable `svl_backend`: resolve, create, insert, remove, destroy, set_prop, get_prop, call, listen, handle_info,
  run, wake, schedule_wake, quit, last_error, describe, describe_type, cancel, request_frame, child attrs, listen_ex,
  style, construct/call_static, observe, start, bounds (S/crates/svelters-abi/src/lib.rs:292-352). Host callbacks:
  post_event, checkpoint, call_sync, on_destroyed, invoke/release callback, frame, prop_changed (lib.rs:263-280).
- Semantics: checkpoint = settle offloads -> changed batch -> events -> timers -> jobs -> ops through vtable ->
  wake if jobs remain (S/plans/architecture/backend-abi.md:84-94); frame clock v2.1 (backend-abi.md:303-312);
  call_sync rules (backend-abi.md:314-324); wake delivery per backend (backend-abi.md:96-105).
- egui backend implementation: ops edit only the scene, never egui (scene/mod.rs:4-6; egui-scene-graph.md:27-30);
  wake = AtomicBool + request_repaint_of (ops.rs:377-388); schedule_wake = deadline + request_repaint_after_for
  (ops.rs:398-409); request_frame = flag + repaint (ops.rs:411-416).

### 3.3 egui-independent / reusable for a new renderer [assessment; line counts from `wc`]
- **Fully egui-free crates**: svelters-abi (2.8k lines), svelters-host (20.4k; rquickjs only), svelters-style
  (8.2k; cssparser/selectors only, "It links no toolkit", S/crates/svelters-style/src/lib.rs:1-12), svelters-sched
  (0.9k, zero deps, Cargo.toml comment), svelters-compiler (53.9k), svelters-modules. JS side `@svelters/native-runtime`.
- **Scene core, egui only as value types**: scene/ (8.8k lines) uses `egui::Id`, `egui::Rect`, `egui::Vec2`,
  `egui::ViewportId`, `egui::Order` as plain data (node.rs:355-395; tree.rs:35-60, 125). dirty.rs, ext.rs,
  compact.rs, keys.rs, source.rs have no egui mention at all. Journal, query, hit lists, virt are logic over
  rects. Replaceable by `emath`-like types or kept via `epaint/emath` alone [inference].
- **Layout**: taffy on the arena (layout.rs, 1.4k) — reusable; only text measurement goes through
  `egui::Context::fonts_mut` (layout.rs:290-322), i.e. epaint's text shaper.
- **Event propagation (EV1)**: DOM capture/bubble, filters, behaviours over the scene (events.rs 1.2k): logic is
  egui-free, input types are `egui::Event/RawInput/Pos2/Modifiers` (events.rs:51-57).
- **CSS cascade glue** style.rs (2.6k): computed style -> `egui::Color32`, per-state `WidgetVisuals` for stock widgets
  (style.rs:1-30, 709, 1112-1264) — partly obsolete as own widgets replace stock ones.
- **Own widget layers**: `paint/` (1.4k, emits epaint shapes via `egui::Painter`), `interact/` (3.7k, units over
  `egui::Response`), `widgets/` (12.5k, heaviest egui use: 628 lines mention egui). These are the parts a new
  renderer must re-host: they are portable in design (no `egui::Button` etc.) but bound to egui's
  Painter/Response/Sense/InputState/Memory/Galley types.
- Embed API (embed/, 2.6k): slots, roots, faults, settle — the API shape is renderer-agnostic, but signatures take
  `egui::Ui`/`Context`/`ViewportId` (embed/mod.rs:895, 1166-1212).

### 3.4 Tied to egui (what you lose or must rebuild)
- `egui::Response`/`Ui::interact`/`Sense` for picking, capture, click/drag detection (ARCH-WIDGETS.md W2 :45; units :117-140).
- `Memory` focus: `ui.memory(|m| m.focused())`, `request_focus` (embed/mod.rs:968-970, 1014-1044; focus.rs 1.1k lines;
  scene focus restore across viewports ARCH-SCENE.md:1074-1079).
- Text input/IME: `egui::TextEdit`, `TextEditState` cleanup on destroy (ops.rs:194; native/mod.rs:27), IME events
  (focus.rs:442-443), `IMEOutput` from value_box (widgets/value_box.rs:423). OW4/TC0 is replacing the editor core.
- Text shaping/galleys: epaint (harfrust/skrifa/vello_cpu per C/docs/research/2026-10-22-code-editor-and-lsp.md:158; no bidi).
- AccessKit via egui: `UiBuilder::accessibility_role/label` (embed/mod.rs:936-940), `ctx.accesskit_node_builder`
  (walk.rs:128-129), `Widget::a11y(&mut egui::accesskit::Node)`; AccessKit node ids are egui Ids (egui-scene-graph.md:18).
  56 files in svelters-egui mention accesskit.
- Layers/z-order: `egui::Area`/`Order`, overlays (tree.rs:35-37; scene passes on layers, pass.rs:150).
- Viewports: deferred/immediate viewports via eframe; `SlotPainter` thread-local hack for `Send+Sync` closures
  (embed/current.rs:1-14).
- Tests: 59 of 73 svelters-egui test files use egui_kittest/Harness; svelters-testkit snapshots on lavapipe
  (OW0 milestone :334-336). svelters-automation drives kittest harnesses and AccessKit actions / synthetic egui
  input (S/crates/svelters-automation/src/lib.rs:1-30; 126 `egui::` refs).
- egui_mcp / egui_inspection read the AccessKit tree and inject `egui::Event`s via a plugin input_hook
  (egui-scene-graph.md:310); cadatomic's pixel tier (C/crates/cadatomic-app/src/pixel.rs:1-30) and
  C/crates/cadatomic-cli/src/mcp/mod.rs.

---------------------------------------------------------------------------------------------------
## 4. svelters-style and svelters-sched
- **svelters-style** (CS0): the one selector/cascade authority for all backends: parses sheets, matches selectors on
  any tree implementing `Element`, cascade (origins, !important, @layer, specificity), custom properties, var(),
  calc(), @media, typed `ComputedStyle` from generated per-toolkit tables, invalidation sets, transitions, utility
  classes (S/crates/svelters-style/src/lib.rs:1-40). Deps: cssparser =0.38, selectors =0.41, cssparser-color,
  smallvec (Cargo.toml). **No egui dependency** (one doc-comment mention). The egui coupling is in svelters-egui's
  `style.rs` (computed style -> egui colours/visuals/padding) and `theme/egui_style.rs`.
- **svelters-sched** (SCHED1): `Spawner` and `Clock` traits plus a seeded deterministic `TestScheduler` and
  `run_seeds` for randomized-schedule tests; zero dependencies so quire, berth, cadatomic can implement it
  (S/crates/svelters-sched/src/lib.rs:1-50; Cargo.toml comment). Host uses it via `HostConfig.sched`
  (S/crates/svelters-host/src/core.rs:86-90). No egui coupling.

---------------------------------------------------------------------------------------------------
## 5. cadatomic's direct egui use

| Area | Mechanism | Citation |
|---|---|---|
| App shell / loop | eframe `App::logic` + `App::ui`, mimalloc | C/crates/cadatomic-app/src/main.rs:3-4, 35-89; app.rs:1324-1370, 4421-4466, 5866-5878 |
| Docking | berth: `berth_core::Dock` model (egui-free) + `egui_berth::DockUi` renderer, isolated in one seam file | C/crates/cadatomic-app/src/dock/seam.rs:1-40 (only file naming berth besides tests/docking.rs) |
| 3D viewport | `egui_wgpu::Callback::new_paint_callback` with `CallbackTrait`: render in `prepare` into own targets (Rgba8Unorm/Depth32/R32Uint id, 4x MSAA), `paint` blits | C/crates/cadatomic-app/src/viewport/callback.rs:1-2, 60-110; viewport/gpu.rs:1-40; viewport/widget.rs:448 |
| ECAD board/schematic/footprint/symbol canvases | same seam via canvas2d (own 2D GPU renderer of capsules/arcs/discs/boxes/polygons per layer and tile) | C/crates/cadatomic-app/src/canvas2d/mod.rs:1-30; board/pane.rs:266; board/sheet.rs:266; footprint/pane.rs:334; library/symbol.rs:210; library/part.rs:741,811 |
| Code editor | quire (`quire-core`, `quire-syntax`, `egui_quire`) both as an app pane and as a `<code-editor>` provider kind | C/crates/cadatomic-app/src/ide/editor_view.rs; C/crates/cadatomic-ui/src/svelte/code_editor/{mod,core}.rs; Cargo.toml pins C/Cargo.toml:61 |
| Svelte UI | svelters Embed (`AppEmbed`), 8 provider kinds (property grid, quantity field, pick, status, empty state, code editor, op/commands behaviours) | C/crates/cadatomic-ui/src/app_embed.rs; svelte/*.rs |
| Menus | egui menus in shell chrome and action bar (7 refs) | C/crates/cadatomic-app/src/shell/chrome.rs; action_bar/mod.rs |
| Other egui widgets | `TextEdit` 42 refs; ScrollArea/Grid/CollapsingHeader/Window/Area 50 refs in cadatomic-app+ui | grep |
| Tests | kittest: 45 of 57 cadatomic-app test files; E4 harness runs ShellApp in kittest eframe mode with egui-wgpu renderer | C/crates/cadatomic-app/tests; examples/e4_frame_budget.rs:440-460 |
| Scenarios | 10 ndjson scenarios (kittest + live) | C/scenarios |
| MCP / inspection | egui_inspection plugin + egui_mcp tree projection behind the RPC socket (debug only) | C/crates/cadatomic-app/src/pixel.rs:1-30; Cargo.toml:72-74 |

`egui::` occurrences (grep -o, src only; tests+examples separately). Note many files `use egui::{…}` so this
undercounts call sites:
- cadatomic-app: 1,320 in src (97 of 155 files mention egui; 64.3k src lines); 693 in tests/examples.
- cadatomic-ui: 240 (24 of 45 files; 13.7k lines).
- cadatomic-cli: 1; cadatomic-core/ecad/model: 0 (`cadatomic-core` must stay headless, C/CLAUDE.md).
- Per module in cadatomic-app (egui:: + `ui.x(` + `ctx.x(` hits / lines): board 695/14,049; library 269/5,642;
  shell 249/2,007; app.rs 186/5,878; footprint 135/3,648; tour 126/7,185; designer.rs 108/655; palette 103/985;
  inspector 102/843; viewport 95/5,992; shortcuts 79/471; views 61/1,187; project 51/391; pixel.rs 41/462; tree 36/879;
  dock 35/1,260; problems 33/799.
- svelters for comparison: svelters-egui 2,253 `egui::` in src (163 files; 65.9k src lines), svelters-automation 126,
  svelters-testkit 11, everything else ~0.

---------------------------------------------------------------------------------------------------
## 6. Other backends: frame clock and dispatch
- GTK: native signal -> `host.post_event` (svelters-gtk/src/backend.rs:1184-1192) -> host `push_event` + `wake`
  (svelters-host/src/lib.rs:125-132) -> wake is an idle at `G_PRIORITY_DEFAULT`, i.e. same priority as GDK input,
  above `GDK_PRIORITY_REDRAW`; after 4 wakes in a row it yields below redraw with a 4 ms backstop
  (svelters-gtk/src/backend/wake.rs:1-30, STREAK=4, AGE_MS=4). So JS runs before the same frame's layout/paint:
  "the handler runs before the next frame clock tick" (egui-scene-graph.md:220). rAF on `add_tick_callback`
  (GdkFrameClock) (backend-abi.md:312; STATUS.md:154). Sync signals call_sync inside the emission (backend.rs:1195-1200).
- Qt: wake = queued call / posted event, one pending (backend-abi.md:100-102); rAF on `QWindow::requestUpdate` (backend-abi.md:312).
- AppKit: wake = version-0 `CFRunLoopSource` (backend-abi.md:102-103); frames from `CADisplayLink` with a watchdog
  timer fallback, 16 ms timer if no link (svelters-appkit/src/backend.rs:1099-1120).
- WinUI: wake = `DispatcherQueue.TryEnqueue` at Normal (svelters-winui/src/backend.rs:206-208, 260; backend-abi.md:103-104);
  rAF via `CompositionTarget::Rendering` (backend.rs:1002-1012).
- egui: wake = flag read at the next pass + `request_repaint` (backend-abi.md:104-105); widget events only after the
  pass -> "about one vsync later than GTK" (EGUI.md:75).
- Design rule: "Each toolkit's event loop is authoritative … JS timers and animation frames use the toolkit's native
  timers and frame clock" (S/PLAN.md:16).

---------------------------------------------------------------------------------------------------
## 7. Existing notes on latency / sluggishness / custom renderer
- One-vsync JS lag on egui and the same-frame `request_discard` option: S/plans/research/EGUI.md:75;
  S/docs/notes/egui-scene-graph.md:12-16, 216-226, 465.
- Custom winit runner as a fallback only (timer ticks costing full frames): egui-scene-graph.md:19, 178, 466;
  EGUI.md:79; ARCH-EMBED.md:440-443 ("stays deferred").
- One-frame latency accepted for dock tab clicks: S/plans/research/ARCH-CRATES.md:1164-1166.
- JS-driven drag feedback one frame late; mitigate with Rust-side previews: S/docs/notes/egui-js-widgets.md:199-200, 522.
- Per-frame cost grows with what is visible, not what changed: egui-scene-graph.md:284.
- cadatomic: performance.md latency section (:301-319), frame-latency trade-off (:28-36, :280-299, :787-789, :840),
  "egui repaints a whole window or nothing" (:19-23), SCOPE.md:589 (callback command buffers submitted before egui's,
  no extra frame for compositing), SCOPE.md:617-619, 02-phase-a.md:1351, 05-experiments:88, :97.
- "vello": only as rejected/irrelevant options — Vello 0.11 "not for copper; no consumer now"
  (C/docs/research/2026-10-22-ecad-gui.md:183); epaint uses vello_cpu for glyph rasterisation
  (S/docs/notes/egui-ecosystem.md:32; C/docs/research/2026-10-22-code-editor-and-lsp.md:158); anyrender in the Slint
  lessons note (C/docs/research/2026-10-22-slint-lessons.md:275).
- No note anywhere says "sluggish"/"input lag" about cadatomic itself; `custom-renderer-trial.md` and
  `svelte5-custom-renderer.md` in S/docs/notes are about Svelte's compiler-side custom renderer PR (sveltejs/svelte#18042),
  not a GUI renderer (custom-renderer-trial.md:1-5).
- The user's 2026-10-22 OW decision already reduced egui to "painter, input, text shaping, layers, AccessKit"
  (STATUS.md:94; ARCH-WIDGETS.md:5-15): the closest existing step toward an own renderer.

---------------------------------------------------------------------------------------------------
## 8. The JS host (svelters-host over rquickjs 0.14 / QuickJS-NG)
- rquickjs 0.14.0, `default-features=false, features=[std, loader]` (S/Cargo.toml:57); QuickJS C at opt-level 2 even
  in dev ("5-10x slower" otherwise) (S/Cargo.toml:64-67).
- "a synchronous runtime on the calling thread" (S/crates/svelters-host/src/quickjs.rs:1-3). `Runtime::new`,
  `Context::full` (quickjs.rs:207, 247). **JS runs on the UI thread**: `Embed` is `!Send` (Rc); arena, QuickJS
  runtime, all JS, logic, show_slot on the UI thread; any thread may only wake, write E3 stores, complete E1 jobs,
  read BusyProbe, set the interrupt flag (ARCH-EMBED.md:498-505). Offloaded exports run on rayon/blocking pools and
  settle at the next checkpoint (STATUS.md:43 E1; core.rs:1086-1094).
- Limits: memory 256 MiB, stack 1 MiB, `turn_budget` 10 s default, `max_jobs_per_checkpoint` 10,000
  (core.rs:94-112); embed budgets warn 8 ms, interrupt 50 ms, mount 500 ms (embed/mod.rs:78-81); cadatomic may override
  the interrupt (app_embed.rs:469-471).
- Interrupt handler: external flag or wall-clock deadline armed per entry (quickjs.rs:211-235, arm/disarm 294-310);
  after an interrupt the host calls `__svelters_recover()` (STATUS.md:121 FX6).
- GC: no GC threshold set; `run_gc` only through `Host::collect_garbage` (quickjs.rs:665-667; lib.rs:544-546), which is
  called only from tests (grep). So QuickJS-NG's default automatic GC applies [inference].
- Jobs: `execute_pending_job` loop bounded by budget (quickjs.rs:631-663).
- Ops: one rquickjs `Function` per op on `__svelters_host` (`setProp`, `getProp`, `call`, `listen`, … quickjs.rs:1140-1240);
  each op crosses straight into the backend vtable; **no batching** ("Ops reach the backend directly while JS runs (no
  batching in P)", core.rs:1071-1072; backend-abi.md:94 "Batching comes later if measurements justify it").
- Measured per-op cost (S/docs/notes/js-native-serdes.md, i7-8850H, load 27-31, "probably 2-3x inflated", :441):
  JS->native call ≈ 0.8 µs dominates (:30, :504); `setProp` by name 1.43 µs vs `setPropId` 0.77 µs (:31, :505);
  batching into a Float64Array is slower in QuickJS (batched int 1.53 µs vs direct 0.77 µs) (:516); callback
  registration ≈ 2 µs (:519); `$state` proxy read of 4 fields +2.5 µs (:515). Export calls 2.8-7.2 µs, C ABI path
  2.6 µs, rquickjs class 0.7-1.0 µs (STATUS.md:42). E1 settle 4.1-4.7 µs batched (STATUS.md:43). P5: "JS dominates the
  1000-row split (host/ABI < 20 ms)" (STATUS.md:155).
- Timing counters (`HostConfig.timing`: js_ns, ops_ns, ops, checkpoints) exist (core.rs:69-70, 114-126).

---------------------------------------------------------------------------------------------------
## 9. Reuse inventory for a custom retained renderer (summary) [assessment]
Keep as-is: compiler, native-runtime JS, svelters-host + ABI (renderer-agnostic by construction, five other
backends prove it), svelters-style, svelters-sched, svelters-modules, scene arena/ids/dirty/journal/query/hit/virt,
taffy adapter, EV1 propagation logic, embed slot/root/fault/settle model, behaviour-unit state machines, CSS
transitions/animation sampler, berth_core, quire-core/quire-text, cadatomic's wgpu viewport + canvas2d (they only
use egui-wgpu's callback seam to get a render pass and a rect).
Re-host: `paint/` primitives (epaint shapes -> new display list), `interact/` units (egui Response/InputState -> own
hit-test + pointer capture), `widgets/` (12.5k), `views/`, focus.rs (egui Memory focus -> own focus model), text
measurement/shaping (epaint fonts -> e.g. parley/cosmic-text/harfrust), AccessKit wiring (egui -> direct
accesskit + accesskit_winit), IME (egui-winit -> winit IME), clipboard/primary selection, viewports/windows,
kittest + svelters-automation drivers + egui_mcp/egui_inspection (AccessKit-based parts carry over if node ids stay
stable), cadatomic's immediate-mode chrome (shell, palette, inspector, board panels, library editors, tours,
designer overlay: ~1.3k `egui::` refs in cadatomic-app src), egui_berth's DockUi renderer, egui_quire's view.
