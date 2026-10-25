# PRESTO-app-test: the `presto` facade (app runner, latency) and `presto-test`

Lane APP-TEST of batch presto-b2 (`plans/cloud/presto-b2/LANE-app-test.md`). Branch `ci/presto-app-test`,
session `session_012jLSJr3WKZ7XDr4KKJtnf1`, 2026-10-25. Owns `crates/presto/**` and `crates/presto-test/**`.
Based on `ci/presto-base` 622aef24 (CORE, TEXT, PLATFORM, FRAME-ANIM and COMPOSITOR skeletons merged in).

## What landed

### `presto` (the app runner)

- `app::App<D: DocumentApi, C: CompositorApi>` implements `presto_platform::AppHandler`:
  - **event-time dispatch**: discrete input flushes the window's coalesced input first (order kept),
    then `Document::dispatch` (stamp `dispatch_end`), then the script host if there are JS-facing events
    (stamp `js_end`); a change runs an urgent frame when `UiFrameState` allows it, otherwise asks the
    compositor for a `BeginFrame` (`request_ui_frame`);
  - **continuous input** (pointer moves, wheel) is coalesced per window (`coalesce`: pointer moves keep
    every earlier sample in `history`; wheel deltas of one unit and gesture state add; gesture
    boundaries are never merged) and dispatched at the next UI frame; reports `high_rate_input`;
  - **`ToUi`**: `BeginFrame` → frame or skip; `CommitApplied` → back-pressure release (re-request when
    dirty); `Presented` → latency termination; `Animation`/`ScrollMoved` → the document;
    `DeviceReset` → re-request every window; late `ResizeDone` ignored;
  - **commit side effects**: `set_cursor`, `set_ime` (area), `update_a11y` (the platform adapter gets each
    commit's AccessKit update), `ui_cost` for answered `BeginFrame`s, `ui_missed` from `CommitReport::late`;
  - **windows**: `open_window` creates the platform window, the wgpu surface on the main thread
    (`CompositorHandle` impl maps `SurfaceTarget::{Raw, CompositionVisual, CoreAnimationLayer}` to
    `wgpu::SurfaceTargetUnsafe`) and hands it to the compositor; close, occlusion (drops in-flight
    inputs, one frame when visible again), exit on last window, compositor shutdown in `exiting`;
  - **resize handshake**: new size → document → urgent frame → `compositor.resize(seq)` → waits up to
    `AppConfig::resize_wait` (one refresh) for the matching `ResizeDone`, deferring other messages;
    timeouts are counted;
  - **AccessKit**: `a11y_event` (`Action` → `accessibility_action` with an urgent frame; `Activated`).
- `seams`: `DocumentApi`, `CompositorApi`, `ScriptHost<D>`, `Dispatched` — the traits the runner drives.
- `backends`: `CoreDocument` (wraps `presto_core::Document`), `impl CompositorApi for CompositorHandle`,
  `UiFrame` (wraps `presto_frame::UiFrameState`, mapping document revisions onto its own and retrying a
  refused `BeginFrame`), `compositor(config, glyphs)` (spawns the real compositor in `init` with the
  platform's clock).
- `latency` (architecture §16.3, `scheduler-latency.md` §5): `LatencyRecorder` with one record per
  `InputSeq` and the 12 stages `os … presented`; terminations `Presented(frame)`, `MergedInto(seq)`,
  `NoVisualChange`, `Dropped(WindowGone | Timeout)`, each exactly once (a second one, a stamp after the
  end, an unknown seq and a duplicate begin are recorded `Violation`s); `presented` folds every committed
  input up to the newest one named (commits fold newer over older); fast commits carry inputs without a UI
  frame; `frame_stages` takes compositor-side stamps. Log-linear `Histogram` (exact < 128 µs, 64 sub-buckets
  per octave, < 1 % error, to ~16.7 s) per (segment, class, window), session and a rolling recent window;
  Chrome-EventLatency segment names; `FrameCounters`; `snapshot_json` (the rpc/MCP surface);
  `chrome_trace` (Perfetto-readable: thread tracks with phase slices, zero-length stage slices joined by
  `s`/`t`/`f` flows per input, async segment slices per input).

### `presto-test`

- `Harness<D>` with kittest's verbs: `new`, `builder()` (`with_size`, `with_scale`, `with_interval`,
  `with_max_steps`, `with_config`, `build`, `build_unsettled`), `run` (until settled; panics after
  `max_steps`), `try_run`, `run_until_settled`, `run_for`, `step`, `busy` (why not settled), `state`,
  `state_mut`, `event` (presto `InputEvent`), `event_egui`, `window_event`, `set_size`, `set_modifiers`,
  `key_press`, `key_press_modifiers`, `key_down`, `key_up`, `type_text`, `hover_at`, `click_at`,
  `click_button_at`, `drag`, `wheel_at`; queries `get_by_label`, `query_by_label`, `get_all_by_label`,
  `get_by_label_contains`, `get_by_role`, `query_by_role`, `get_by_role_and_label`,
  `query_by_role_and_label`, `get_by_value`, `query_by_value`, `get_by_author_id`, `query_by_author_id`,
  `query_all(By)`, `get(By)`, `focused`, `a11y_dump`; `Node` handles with `click`, `click_secondary`,
  `hover`, `click_accesskit`, `focus`, `action`, `type_text`, `key_press`, `rect`, `label`, `value`,
  `role`, `author_id`, `toggled`, `is_disabled`, `is_focused`. `get_*` panics on none or several matches.
- It runs the real `presto::App` on `presto_platform::HeadlessPlatform` (input and AccessKit actions are
  scripted into it, so they take the production path) and its virtual clock; `step` = deliver, advance one
  interval, vsync, deliver the wake.
- `HeadlessCompositor` (GPU-less `CompositorApi`): applies folded commits at vsync, `CommitApplied`,
  `Presented { at: vsync }` naming the commits' and fast commits' input seqs, `BeginFrame` when asked or
  while an animation runs, `Started`/`Finished` for keyframe and `{duration, bounce}` animations,
  immediate `ResizeDone`, hidden windows hold their commits.
- `Harness::render` → `Image` (premultiplied RGBA8, device pixels): the headless compositor folds every
  applied commit and fast commit into a real `presto_compositor::tree::WindowTree`; `render` flattens its
  main surface into a `FrameScene` (full damage, `with_clear`, default white) and draws it with
  `presto_render::Renderer` on `presto_render::headless::HeadlessTarget`, on the environment's adapter
  (`GpuRender`: default adapter, then the fallback; lavapipe in Linux CI). Glyphs default to presto-text's
  `FontRasterizer` over `FontConfig::testing()` (`with_glyphs` overrides). Not drawn: the compositor's own
  caret, compositor animations' sampled values, OS-layer surfaces.
- `a11y`: the AccessKit snapshot folded from the app's `update_a11y` calls (unreachable nodes dropped,
  window-space bounds through ancestor transforms), `By` queries with author ids (EM1).
- `egui_event::EguiAdapter`: `egui::Event` → presto input (pointer, buttons with held-button mask, keys by
  egui name, modifiers with `command` → Cmd/Ctrl, text and IME with char → byte ranges, touch, wheel units
  and trackpad phases, copy/cut/paste as chords plus a clipboard write, focus, AccessKit requests).

### Tests (written, not run)

- `crates/presto/tests/latency.rs`: full pipeline → presented with all 11 segments; fold of older inputs;
  double termination, stamp-after-end, unknown seq and duplicate begin are violations; timeout and window
  drops; frame without commit; fast commits; bounded ended set; histogram quantiles within 1 %; snapshot
  and trace shape (one `s`, one `f`, balanced async pairs); pointer-move merge with history; wheel sums with
  gesture boundaries kept; discrete events never merged.
- `crates/presto-test/tests/harness.rs` (a small document: button, label, checkbox, text field): first
  frame and settle; click through dispatch updates the snapshot (urgent frame counted); a click reaches the
  screen in one step; queries by role/label/value/author id and bounds; `get` panics on several and on none;
  Tab focus and Space toggle; `type_text` via AccessKit focus; AccessKit click; egui events; idle steps do
  nothing; resize handshake; HiDPI sizes in points; `run` waits for a 100 ms compositor animation;
  **every injected input terminates exactly once** (hover merges, click, keys, drag: 17 seqs, no
  violations, presented ones carry commit and presented stamps); Chrome trace has flows; closing the last
  window exits and shuts the compositor down; hidden windows.
- `crates/presto-test/tests/render.rs` (`PRESTO_GPU_TESTS=1` only): a committed quad's pixels and the
  clear colour, a re-recorded list after a key press, and HiDPI output in device pixels.

## Not done (left out, not faked)

- Hover "at once" for continuous input (architecture §11.1) is not separate from the coalesced dispatch:
  `Document` has no hover-only entry (CORE request 2 below).
- Physical and svelte/motion springs never end in the headless compositor (no settle time without
  `presto_anim::spring`); `busy()` says so. Wiring `presto_compositor::seams::AnimSampler` in would fix it.
- `App` does not own a `TextSystem`: the document is built by the caller (`Document::new(registry,
  Box::new(text_system))`); a `presto::run(…)` convenience that builds platform + text + compositor +
  document is for the consolidation, once presto-render's renderer factory is wired into
  `CompositorConfig`.
- PresentMon ETW events and Tracy export (architecture §16.3) are not written; the Chrome JSON trace is.

## Builds and tests run

- `cargo check -p presto -p presto-test --all-targets` and `cargo clippy … -- -D warnings`: clean (only
  presto-core's own dead-code warnings appear).
- No tests run locally (per the batch rule). CI: see the report to the coordinator.
- Note: the `presto` workflow's crate list is `ls -d crates/presto-*`, which **excludes `crates/presto`**;
  its tests (`tests/latency.rs`) run only in linux-gtk's workspace `cargo nextest run`. Recommend adding
  `crates/presto` to the list (coordinator's file).

## Cross-lane edits

None. `Cargo.lock` gained presto's and presto-test's dependency edges.

## Requests and questions (each with a recommendation)

1. **CORE: `Document::needs_frame(window) -> bool`.** `CoreDocument` approximates it with "revision moved
   since this window's last frame", so every transaction (even a hover that changed nothing) costs a UI
   frame that commits nothing. Recommend exposing the window's dirty state (`RootDirty`/journal non-empty).
2. **CORE: a hover-only update for continuous input** (`Document::hover(window, pos)`), so `:hover` changes at
   event time while JS still gets the coalesced move at the frame (§11.1). Recommend adding it.
3. **CORE: AccessKit actions.** `Document` has no action handler; `CoreDocument` only maps `Focus` by
   assuming AccessKit ids equal presto `NodeId`s. Recommend `Document::accessibility_action(window,
   &ActionRequest) -> bool` and documenting the id mapping of the a11y projection.
4. **CORE: animation events, window focus/theme, device reset, AT activation** have no `Document` entry;
   `CoreDocument` drops them (documented). Recommend `Document::{animation_event, window_event,
   device_reset, a11y_full_tree}`.
5. **CORE: the IME purpose with the area.** `Commit::ime_area` has no purpose; `App` sends `Text`.
   Recommend an `ime: Option<(Rect, InputPurpose)>` (presto-base additive: a new field) or a focus-driven
   `ImeState` from the document.
6. **COMPOSITOR: publish `FrameArgs`** (next target, `D_ui`, `est_ui_input`, idle, visible) for the UI
   thread. Without it `UiFrameState::on_urgent_input` always runs urgent frames for `now` when idle.
   Recommend a `CompositorHandle::frame_args(window) -> Option<FrameArgs>` (seqlock/atomic snapshot).
7. **COMPOSITOR: compositor-side latency stamps** (`draw`, `submit`, `gpu_done`, `present`) never reach the
   UI thread; `LatencyRecorder::frame_stages` is ready for them. Recommend adding them to
   `ToUi::Presented` (presto-base: new fields are breaking for struct-literal builders, so a new
   `ToUi::FrameTimings` variant, which breaks exhaustive matches; the coordinator's call). Until then the
   `CommitToDraw … PresentToPresented` segments stay empty and `InputToPresented` is still exact.
8. **COMPOSITOR: back-pressure on `request_ui_frame`.** `App` relies on the compositor holding a requested
   `BeginFrame` until the in-flight commit is applied (scheduler-latency §2.11); `UiFrame` also re-requests
   after a refused `BeginFrame`. Please confirm the engine does that.
9. **CI: add `crates/presto` to the presto workflow's crate list** (see above).
10. **COMPOSITOR (a bug, not a design question): no "UI frame skipped" signal.** `IntervalPacer` sets
    `ui_sent` when it sends `BeginUiFrame` and clears it only on `CommitArrived`, so a `BeginFrame` the UI
    answers with nothing (no damage) stops all further `BeginFrame`s for that window. Stopgap on this
    branch: `CompositorApi::ui_frame_skipped`, which the `CompositorHandle` impl answers with an empty
    commit (no ops, revision 0, no input; the tree only moves its revision forward). Recommend
    `CompositorHandle::ui_frame_skipped(window, frame)` → `Signal::UiFrameSkipped { frame }` clearing
    `ui_sent` (presto-frame's `FrameClock` should take the same signal), then the impl calls it.
11. **TEXT-INPUT's `TextServices` (asked by the coordinator 2026-10-25 19:28):** not wired yet; the API
    is on `ci/presto-text-input` 0030df69, not in base. A TODO naming the call sits in `App::init`
    (`crates/presto/src/app.rs`). One design note for the consolidation: the platform clipboard is only
    reachable through `&mut dyn PlatformCx` during a handler call, so a `TextServices` installed once in
    a thread-local cannot hold it. Recommend that the services own their own
    `presto_platform::SystemClipboard` (or `MemoryClipboard` headless), and that the fast-commit channel
    be a clone of the compositor handle (`CompositorApi::fast`). `presto` then needs a dependency on
    `presto-widgets`.
    **Decided (coordinator, 2026-10-25):** `TextServices` owns its own `presto_platform::SystemClipboard`
    (`MemoryClipboard` headless) and a clone of the compositor handle for caret fast commits; the
    consolidation wires it at the TODO in `App::init`.

## Update (2026-10-25, after base 2a12e530+)

- `Harness::render` landed (above). Not run locally (no Vulkan ICD in this container); CI's lavapipe run
  is its first run.
- Adopted COMPOSITOR's new handle API: `needs_ui_frame(window, true)` for frame requests,
  `ui_frame_skipped` (request 10 is done, the empty-commit stopgap is gone), `ui_input_cost` for frames
  that handled discrete input; `ToUi::Occluded` from the frame source hides/shows the window like
  `WindowEvent::Occluded`.
