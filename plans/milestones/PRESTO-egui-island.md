# PRESTO-egui-island: the egui island (presto batch 2)

Lane EGUI-ISLAND (`plans/cloud/presto-b2/LANE-egui-island.md`), branch `ci/presto-egui-island`, from
`ci/presto-base` at `e34079e6` (merged up to CORE-3 `fa1d7758`). Owns `crates/presto-egui-island/**`. Design: architecture §13 (egui islands),
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

- **`host`: islands as presto-core nodes** (coordinator's Q4 decision, 2026-10-25). The `egui-island` kind
  (`EguiIslandKind`, a focusable leaf, role `Pane`) and `IslandHost`, a document service keyed by node id:
  - `IslandHost::register(&mut KindRegistry)`; `mount(doc, node, island)` (the island's AccessKit tree becomes
    `island_tree_id(node)`, accessibility turns on, the node's `mounted` prop is set; refusals hand the island
    back) and `detach(doc, node)`, which returns the island with its egui state and texture for a remount on
    another node (a pane moving between dock slots or windows: presto_berth's `PaneHost` slots call these);
  - the kind records the island's `Surface` over its border box, forwards its native input (pointer with
    capture and focus on press, hover leave, cancel, wheel, keys, IME, focus in/out, AccessKit actions) to
    `EguiIsland::handle_event`, sets egui's cursor, and grafts the subtree in `a11y` while mounted;
  - the runner drives it: `run(doc, now, |node, ui| …)` before `Document::frame` (due passes with the host's
    own UI callback, e.g. `PaneHost::ui`; each pass bumps the node's Paint-class `pass` prop so the commit
    re-composites it; returns each pass's `IslandOutput`, whose AccessKit update the runner forwards after
    the commit carrying the graft node), `render(doc, device, queue, encoder, registry)` before the commit is
    sent (also releases islands of destroyed nodes), `next_wakeup(doc)`, `bridge_events(doc, &events)` and
    `drag_moved(doc, pos)` for core's in-app drags.
- **DnD on core's in-app drag** (CORE-3; the `PrestoDrag` shim is gone, `shim.rs` deleted): `DragData { ty,
  data }` is core's payload (`drag-type`/`drag-data`). A mounted island listens (passive) to `drag-enter`,
  `drag-leave` and `drop` on its node; `bridge_events` turns them into `drag_enter` (type only, at the island's
  centre), `drag_leave` and, on `drop`, the payload with its data at the drop point plus `drag_drop`. The node
  is a core drop target when the app sets its `drop-types`.

- **Physical keys** (CADATOMIC's bug, coordinator 2026-10-25): `input::physical_key` maps presto's
  `KeyEvent::physical` (HID `page << 16 | usage`) to egui's key at the US-layout position, as egui-winit's
  `key_from_key_code` does (numpad folded the same way); every egui `Key` event carries it, and a key with no
  Latin logical key falls back to it, so Ctrl+(the key at C) copies on a Cyrillic layout (cadatomic's KL1).
- **Device reset** (same report): `EguiIsland::device_reset` (and `IslandHost::device_reset(doc)` for all
  mounted islands, next to `Document::device_reset` on `ToUi::DeviceReset`) drops the renderer and texture and
  asks for a pass; the next `render` creates the renderer on the device it is given, re-uploads egui's textures
  from the island's CPU mirror of them (egui sends each texture once, as deltas, and has no public way to
  resend its font atlas), and registers a new texture. Paint callbacks' `callback_resources` and native
  textures belonged to the old renderer: the host sets them up again (`init_renderer_on`).

- **Island → presto drags** (Q5, CORE-3's `Document::start_drag`): when an egui drag leaves its island with the
  button held, `IslandHost::run` starts presto's in-app drag for the island's held pointer with the island node
  as source (`drag-type` = the egui payload's type, its data), and releases egui's side (`drag_out_started`:
  a release and pointer-gone, so egui ends its drag). The target under the pointer is entered at once; the
  drop is presto's. The island node listens to `drag-end` too, and `bridge_events` reports `dropped` back to the
  egui source (`IslandDrag::was_accepted`). `IslandOutput::dnd.drag_out` stays set only when presto refused
  (another drag running) and for islands driven without a document.

## Tests (written, not run locally)

- `tests/island.rs` (no GPU): button and label clickable through presto input (scale 1 and 2); input outside
  not taken; pointer capture until release; focused island takes keys and text, command chords type nothing;
  idle island sets no deadline and input wakes it; the display list; **the AccessKit graft is valid**: applied
  to an `accesskit_consumer::Tree` with a host root and graft node, the button is reachable from the host root
  inside the subtree, its bounds compose (offset then host scale), a repeat full update applies, and an
  AccessKit `Click` on the island's tree clicks the button; DnD: a presto drag drops on an egui drop zone, a
  leaving drag clears the payload, an egui drag leaving the island becomes a presto drag exactly once.
- `tests/host.rs` (presto-core `Document`, no GPU): a mounted island records its texture and takes a click
  through `Document::dispatch` (press focuses the node); mount refusals; the node is the graft point in the
  commit's AccessKit tree and the island's subtree uses the node's tree id; detach and remount on another node;
  presto-core's drag from a `drag-type` box drops on an egui drop zone in the island with its data; an egui drag
  source in the island dragged out onto a `drop-types` box becomes presto's drop there (type, data, the island
  node as `drag-end` source) and egui's drag ends.
- `tests/input.rs`: physical keys by US position and the Latin fallback for shortcuts; key names, modifiers per platform, clipboard shortcuts, focus gating and text filtering,
  IME char ranges, wheel routing and sign, pinch.
- `tests/render.rs` (GPU, `PRESTO_GPU_TESTS` + an adapter): renders into a registered 64×48 `Rgba8Unorm`
  texture, reads back egui's red rect and the transparent clear, a paint callback's `prepare`,
  `finish_prepare` and `paint` each run once, no render without a pass, a resize to scale 2 re-registers a
  128×128 texture, `release` unregisters; a device reset re-renders on a second device with the font atlas
  replayed (egui's red rect samples its white texel) and registers the new texture.

Run locally: `cargo check -p presto-egui-island --all-targets`. After CI run 40223175641 (on `15ec9267`) failed
two test assertions on Windows and macOS (wrong test expectations: egui's display key names, and Super is no
command key off macOS), one filtered local run of `--test input --test island` to debug them: 7 and 11 passed
after the fix. `--test render` passed on CI's Windows (WARP). After the host work, one filtered run of
`--test host --test island`: 3 and 11 passed; after the physical-key work, `--test input`: 9 passed; after `start_drag`, `--test host --test island`: 4 and
11 passed. **Not run locally:** `render`, `input` (unchanged), clippy.

**CI on `0dd5a347`** (merged `ci/presto-base` `cc4f184d`; runs 40226671183 js, 40226671201 linux-gtk,
40226671185 linux-qt, 40226671215 macos-appkit, 40226671186 presto, 40226671179 windows-winui): every
`presto-egui-island` target passes on Linux (lavapipe), Windows (WARP) and macOS, `--test render` included. Still red,
none of it this crate's:
- presto: `presto-compositor --test engine`, `--test tree_damage`, `presto-core --lib` and `presto-widgets --lib`
  fail identically on base's own run of `cc4f184d` (40226563028).
- linux-gtk: `svelters-egui::eframe_idle::eframe_window_draws_then_idles_at_zero_cpu` (idle CPU 810 ms over 2 s,
  0 frames: load on a backlogged runner). It passed on base's run (40226563040), and this branch changes nothing
  outside presto except `THIRD_PARTY_NOTICES.md` and its own `Cargo.lock` entry. Base's macos-appkit failed while
  this branch's passed: the same kind of flake. Not re-run (runners are backlogged).

## Wave-1 APIs used

- `TextureRegistry` (island.rs) is implemented for `presto_compositor::CompositorHandle`; tests use a recorder.
- The clipboard is `presto_platform::clipboard::Clipboard` (`ClipboardKind::Clipboard`).
- presto-core: `Kind`, `KindRegistry`, `Document` services, `world_rect`, `listen`, `JsEvent`s and its in-app DnD.
- No shims left.

## Not done / left out

- While a presto drag is over an island, egui sees the pointer only where the runner reports it
  (`IslandHost::drag_moved`); core's drag doesn't deliver moves to the target node.
- The bridge's listeners are ordinary document listeners: a JS `unlisten` of the same signal on an island node
  (svelters' `ondrop` on an `<egui-island>`) removes them too.
- egui `OutputCommand::CopyImage` and `TextSelectionSettled`: no presto counterpart; ignored.
- presto `ImeEvent::DeleteSurrounding` (bytes of text the island can't see) is dropped: egui wants chars.
- egui deferred/immediate viewports inside an island are not supported (an island is one egui root viewport).

## Cross-lane edits

- `crates/presto-base/src/event.rs`: one doc line, `GestureEvent::Rotate`'s `delta_rad` is clockwise-positive
  (coordinator's Q1 decision).

- `THIRD_PARTY_NOTICES.md`: one paragraph for the egui-winit rules adapted in `input.rs` (MIT notice).
- `Cargo.lock`: the crate's new deps (presto-compositor, presto-platform, egui, egui-wgpu, wgpu, accesskit;
  dev: pollster 0.4, accesskit_consumer 0.39.1), all already in the lock.

## Questions for the coordinator

1. ~~Rotate sign~~: decided (coordinator, 2026-10-25), clockwise-positive, documented in presto-base.
2. ~~`CompositorHandle::unregister_texture`~~: the compositor skeleton has it.
3. ~~Island render thread~~: decided, UI thread.
4. ~~How an island becomes a node~~: decided and built (`host`).
5. ~~Starting a core drag for a held pointer~~: decided (coordinator, 2026-10-25), CORE-3 adds
   `Document::start_drag`, and the host uses it (above). Original question: For island → presto drags, presto-core needs something like
   `Document::start_drag(window, pointer, source: NodeId, ty, data)` that enters its drag state for a pointer
   already pressed (the island node as source), so `IslandHost` can turn `drag_out` into a core drag.
   Recommendation: add it to presto-core (additive, about 20 lines over `dnd::pointer`'s start branch); until
   then the runner gets `drag_out` and cadatomic's panels keep drags inside one island.
