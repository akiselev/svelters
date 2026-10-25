# PRESTO-egui-island: the egui island (presto batch 2)

Lane EGUI-ISLAND (`plans/cloud/presto-b2/LANE-egui-island.md`), branch `ci/presto-egui-island`, from
`ci/presto-base` at `e34079e6` (merged up to `3bde04cd`). Owns `crates/presto-egui-island/**`. Design: architecture §13 (egui islands),
§15 (the island), §11.7 (the DnD bridge).

## What landed

`presto-egui-island` (egui 0.37.0, egui-wgpu 0.37.0, wgpu 30.0.1, accesskit 0.25.1, as `Cargo.lock` already
had them; egui pinned exactly like svelters-egui):

- **`island::EguiIsland`**: owns an `egui::Context` and an egui-wgpu `Renderer` (created with the device, or
  lazily at the first render, so passes, input and accessibility work without a GPU).
  - `run(now, ui)`: one egui pass (`Context::run_ui`, so cadatomic's `PaneHost::ui(&PaneId, &mut Ui)` plugs in
    directly) over the input queued since the last pass, tessellated, platform output turned into presto's:
    cursor (egui → presto CSS cursor names), IME area and caret in window points, copied text (also written
    to the clipboard), `OpenUrl`s, the grafted AccessKit update, DnD output, and `repaint_at` from egui's
    `repaint_delay` (`Duration::MAX` → `None`).
  - `render(device, queue, encoder, registry)`: texture deltas → `update_buffers` → `render` into the island's
    own `Rgba8Unorm` texture (cleared transparent; egui writes gamma-space premultiplied colour, presto's
    blending space, §8.3). The texture is (re)registered through the registry whenever it is (re)allocated
    (first render, size or scale change). Returns paint callbacks' command buffers, to submit before the
    encoder's (eframe's order). A pass replaced before it was rendered keeps its texture uploads
    (`TexturesDelta::append`).
  - Repaint only when needed: `needs_pass(now)` / `next_wakeup()` are set by input that reached egui, a size or
    scale change, accessibility actions, drags over the island, and egui's own repaint requests; an idle
    island has no deadline (the idle barrier holds).
  - `display_list()`: one `Prim::Surface` over the node's box; `layer_kind()`: `LayerKind::Texture(id)`.
  - `egui_wgpu::CallbackTrait` paint callbacks run on the island's renderer (`renderer_mut()` exposes
    `callback_resources` and `register_native_texture`).
- **`input`**: presto `InputEvent` → egui `Event`s, no egui-winit. Pointer routing with capture (events reach
  egui inside the rect, or outside while a button pressed inside is held; leaving is `PointerGone`; `Cancel`
  releases held buttons), touch as egui `Touch` plus pointer events, wheel with phase/momentum → egui
  `MouseWheelSource`, keys and text only with keyboard focus (`set_focused`, presto owns focus), egui-winit's
  clipboard shortcuts (Copy/Cut events; paste reads the clipboard), egui-winit's printable-text filter, IME
  preedit byte → char ranges, pinch → `Zoom(exp(delta))` (egui-winit's mapping), rotate, OS file hover/drop
  → `hovered_files`/`dropped_files` (dropped files read on demand). Keys map by name (presto's `Key` table is
  egui's).
- **`a11y::A11yGraft`**: AccessKit nested trees. `island_tree_id(seed)` (never `ROOT`); `graft_node` marks the
  host's node; `graft` rewrites egui's `TreeId::ROOT` to the island's tree, replaces egui's root transform
  (`scale(ppp)`) with the island's placement (default: its offset in window points, since subtree transforms
  compose through the graft node and the host root, per accesskit_consumer `NodeRef::transform`), repairs a
  focus id not in the update; `map_action` points actions for the island's tree back at `ROOT` (egui ignores
  any other tree). Node ids stay egui's (per tree in nested trees).
- **`dnd`**: the `egui::DragAndDrop` ↔ presto bridge. presto → island: `drag_enter/move/leave/drop`; the payload
  goes into `DragAndDrop` through a `PayloadCodec` (default: `IslandDrag`); a drop is a release with no press
  (egui's `any_released` → drop zones take it; no widget sees a click); acceptance is `IslandDrag::accept()`.
  island → presto: an exportable egui drag leaving the rect with its button held comes out once as
  `DndOutput::drag_out`; `drag_out_finished(accepted)` reports back to the egui source. A codec for
  svelters-egui's `DragPayload` can be plugged in by an embedder that links svelters-egui.

## Tests (written, not run locally)

- `tests/island.rs` (no GPU): button and label clickable through presto input (scale 1 and 2); input outside
  not taken; pointer capture until release; focused island takes keys and text, command chords type nothing;
  idle island sets no deadline and input wakes it; the display list; **the AccessKit graft is valid**: applied
  to an `accesskit_consumer::Tree` with a host root and graft node, the button is reachable from the host root
  inside the subtree, its bounds compose (offset then host scale), a repeat full update applies, and an
  AccessKit `Click` on the island's tree clicks the button; DnD: a presto drag drops on an egui drop zone, a
  leaving drag clears the payload, an egui drag leaving the island becomes a presto drag exactly once.
- `tests/input.rs`: key names, modifiers per platform, clipboard shortcuts, focus gating and text filtering,
  IME char ranges, wheel routing and sign, pinch.
- `tests/render.rs` (GPU, `PRESTO_GPU_TESTS` + an adapter): renders into a registered 64×48 `Rgba8Unorm`
  texture, reads back egui's red rect and the transparent clear, a paint callback's `prepare`,
  `finish_prepare` and `paint` each run once, no render without a pass, a resize to scale 2 re-registers a
  128×128 texture, `release` unregisters.

Run locally: `cargo check -p presto-egui-island --all-targets`. After CI run 40223175641 (on `15ec9267`) failed
two test assertions on Windows and macOS (wrong test expectations: egui's display key names, and Super is no
command key off macOS), one filtered local run of `--test input --test island` to debug them: 7 and 11 passed
after the fix. `--test render` passed on CI's Windows (WARP). **Not run locally:** `render`, clippy.

CI's Linux presto job fails earlier, in `cargo check`: `yeslogic-fontconfig-sys` needs `libfontconfig1-dev`,
which the presto workflow's apt step doesn't install (a presto-base/workflow issue, not this crate's).

## Wave-1 APIs used, and the shim left (`src/shim.rs`)

Merged `ci/presto-base` at `3bde04cd`. The island uses the real APIs now:
- `TextureRegistry` (island.rs) is implemented for `presto_compositor::CompositorHandle`
  (`register_texture`/`unregister_texture`); tests implement it with a recorder.
- The clipboard is `presto_platform::clipboard::Clipboard` (`ClipboardKind::Clipboard`).

Still a shim: `PrestoDrag { entries: Vec<(type, bytes)>, source: Option<NodeId> }`, standing in for
presto-core's DnD session (§11.7), which doesn't exist yet.

## Not done / left out

- **No `presto_core::Kind` impl.** The trait now exists (`presto-core/src/kind.rs`), but a kind is built by a
  `KindFactory = fn() -> Box<dyn Kind>` with no arguments, while an island needs its descriptor (texture id, tree
  id) and the host's `ui` callback (`PaneHost::ui`), and `EventCx`/`RecordCx` are still being filled in. The
  island exposes what a kind needs (`display_list`, `layer_kind`, `handle_input`, `set_rect`,
  `a11y().graft_node`, `next_wakeup`). See question 4.
- egui `OutputCommand::CopyImage` and `TextSelectionSettled`: no presto counterpart; ignored.
- presto `ImeEvent::DeleteSurrounding` (bytes of text the island can't see) is dropped: egui wants chars.
- Physical keys: presto carries USB HID usages, egui wants `Key` names; `physical_key` is `None` (egui-winit's
  layout fallback for non-Latin layouts' clipboard shortcuts is therefore missing).
- egui deferred/immediate viewports inside an island are not supported (an island is one egui root viewport).

## Cross-lane edits

- `THIRD_PARTY_NOTICES.md`: one paragraph for the egui-winit rules adapted in `input.rs` (MIT notice).
- `Cargo.lock`: the crate's new deps (presto-compositor, presto-platform, egui, egui-wgpu, wgpu, accesskit;
  dev: pollster 0.4, accesskit_consumer 0.39.1), all already in the lock.

## Questions for the coordinator

1. **Gesture rotate sign.** `presto_base::event::GestureEvent::Rotate { delta_rad }` doesn't state its sign.
   egui-winit negates winit's (counter-clockwise positive) for egui. The island passes presto's value through
   unchanged. Recommendation: document in presto-base that `delta_rad` uses egui's convention (clockwise
   positive), so platform backends convert once and the island stays a pass-through.
2. ~~`CompositorHandle::unregister_texture`~~: resolved, the compositor skeleton has it.
3. **Threading of island renders.** The island renders on the UI thread on presto's device/queue (the commit
   showing the texture is sent after the submit, so queue order makes it visible). If the compositor prefers
   to encode islands itself, `render` already takes `(device, queue, encoder)` and can move to the compositor
   thread with the pending paint jobs. Recommendation: keep UI-thread rendering for now.
4. **How an island becomes a node.** Recommendation: the app runner keeps islands in a side table keyed by node
   id and registers one `EguiIslandKind` whose `record` emits the island's `display_list`, `event` forwards to
   `handle_input`, and `a11y` calls `graft_node`. The kind looks its island up by node id, so the no-argument
   `KindFactory` still works. The consolidation writes it once presto-core's contexts settle, or the
   coordinator gives it to this lane in a later batch.
