# SKELTER-fixes (cadatomic batch 12, svelters lane)

Branch `ci/skelter-fixes` from master fc7903fe. Coordinator: `session_01CqZ417NuvkRujZaYSm7Pwo`. The COORDINATOR.md
items were done by four workers in their own worktrees and merged here `--no-ff`.

## Landed

### FIX-QUEUE

| Item | Change | Test |
|---|---|---|
| F13 | `StateBits` import moved into `controls/box_.rs` tests | skelter-widgets checks with no warning |
| F18 | svelters-style `overflow` shorthand: 1 or 2 values onto `overflow-x`/`overflow-y` | `shorthand::overflow_one_or_two_axes`, `ow_css::overflow_shorthand_sets_both_axes` (egui and skelter tables) |
| F21 | none: the red run (40240364474, APP-TEST 7bd9ee8f) lacks F14's fix 0876a38f; the current test passes on windows-latest (skelter 40246270706). Closed as a duplicate of F14 (coordinator) | existing test |
| F22 | two timing causes in `counter-mac` `tests/backend.rs`: (a) ce2f4ecc, the resumed CADisplayLink ticked after the 64 ms watchdog, so the timer delivered frame 2 ("both frames came from CADisplayLink" 1 vs 2); (b) a9503acb, `schedule_wake(20)` fired after a fixed 120 ms pump (2 vs 3). `AppKitBackend::set_frame_watchdog_ms` (the test raises it to 800 ms while it checks the link) and a bounded wait of up to 1 s for the wake. No assertion on elapsed time | `request_frame_delivers_one_frame`, `schedule_wake_runs_a_checkpoint` (macOS CI) |

### Coordinator request (blocking berth)

A same-window move keeps pointer capture, focus and hover; they drop only when the node leaves the tree or changes
window (decided, coordinator 2026-10-26). `Document::insert` → `detach_as(.., keep_input)`, `WindowState::retire(..,
keep_focus)`. Tests: `tests::tree::a_move_inside_the_window_keeps_capture_focus_and_hover`,
`::a_move_to_another_window_drops_capture_and_focus`. Commit b1eb3135.

### COORDINATOR.md items

- **FastCommit::Caret colour and space**: `color` and `transform: PropNodeId` (rect in that node's space); the compositor
  maps the caret through the node's world transform. Core sends the node's computed `color` (no `caret-color` yet).
  Test: compositor `engine::caret_draws_in_its_colour_and_follows_its_transform_node`.
- **No caret for hidden editors**: `skelter-core/src/caret.rs`; detached, `display: none`/concealed, `visibility:
  hidden`, zero-size or unrecorded sends `rect: None` once, re-sends the last caret when rendered again. Test:
  `tests::hooks::no_caret_for_an_editor_that_isnt_rendered`.
- **`RecordCx::fixed()`**: a self-scrolling kind's fixed part is its own list in the parent layer, below the scroll
  layer. Tests: `a_self_scrolling_kinds_fixed_part_stays_out_of_its_scroll_layer`,
  `a_fixed_part_of_a_kind_that_does_not_scroll_itself_goes_first`. text-edit does not use it yet (below).
- **Q13**: `Monitor::ppi` (+ `ppi_from_mm`), `PlatformCx::monitor_of`, `WindowEvent::MonitorChanged`, in Win32, macOS,
  Wayland and winit (winit: no ppi); the facade's `sync_monitor` sends refresh and ppi (`CompositorApi::set_ppi`).
  Tests: `api::tests::ppi_from_physical_size`, `headless::tests::monitor_of_follows_the_set_monitor_then_the_position`,
  skelter-test `harness::monitor_changes_send_refresh_and_ppi`, per-backend ppi helpers.
- **Drag-source, popup, fullscreen in the platform contract** (WAYLAND Q2's recommendation): `PlatformCx::{create_popup,
  create_child_window, layer_frame_source, client_side_decorations, set_window_state -> bool, set_ime_hints, start_drag,
  attach_window_to_drag}` as defaults, `AppHandler::drag_ended`, `PlatformError::Unsupported`; Wayland's `WaylandCx` is
  gone. `FrameSource::before_present_sized`. Tests: `headless::tests::state_requests_are_recorded_and_unsent_and_drags_end_through_the_handler`,
  `api::tests::popup_origin_sits_on_the_gravity_side`, Wayland `present::tests::pending_state_waits_for_a_buffer_of_its_size`,
  Win32 `drag::tests::platform_payloads_become_text_and_files`.
- **WAYLAND Q3–Q8**, as recommended: Q3 CSD border documented, pure `window::csd_border` (tested); Q4 surrounding text
  is UTF-16 everywhere (tests in widgets editor, Win32, Wayland); Q5 key codes `page << 16 | usage`, else
  `0xFFFF_0000 | native` (winit/Win32/macOS fall back to the native code now; tests); Q6 fifo stays opt-in
  (Mailbox/Immediate only, documented); Q7, Q8 documented.
- **TEST-V2 Q1**: `Commit::a11y_subtrees`, sent after the host tree. Test:
  `core_islands::the_island_subtree_reaches_the_platform_adapter_under_its_node`.
- **TEST-V2 Q2**: `svelters_automation::SceneSource`; engine and snapshot generic over it (svelters-egui's `SceneRef`
  implements it); skelter-test `SkelterScene` (feature `automation`). Tests: `scene_source::{selectors_resolve_on_a_skelter_document,
  the_snapshot_reads_a_skelter_document}`.
- **`IslandHost::bridge_events`**: `CoreDocument::add_event_tap`, `IslandHost::event_tap`. Test:
  `core_islands::a_skelter_drag_over_the_island_reaches_egui_drag_and_drop`.
- **Emit from prepare, and on a child**: `PrepareCx::emit`, `EventCx::{emit_on, set_native_on}` (self or a
  descendant); table and tab-view use them. Tests: `tests::emits::*`, table and tabs kind tests.
- **Drag-preview hook**: `DragPreview`, `Kind::drag_preview`, `Document::set_drag_preview_provider`; the dnd ghost is
  the default. Tests: `tests::emits::a_drag_keeps_the_preview_its_source_or_the_app_supplies`,
  `dnd::ghost::tests::a_core_drag_carries_the_ghost_as_its_preview`.
- **Picker overlay layers**: `DatePickerKind`, `ColorPickerKind` with `popup::overlay_spec`, light dismiss. Tests:
  `pickers::kinds::tests::the_date_popup_opens_in_the_overlay_stack_and_light_dismisses`, `..colour_popup..`.
- **Hex and cell editors on text-edit**: table `text::cell::CellEditor` (`commit(row, column, value)`), color-editor
  `text::hex::HexField`. Tests: `table::kinds::tests::typing_on_an_editable_cell_edits_through_the_text_field_and_commits`,
  `pickers::kinds::tests::typing_in_the_hex_box_edits_through_the_text_field`.
- **Table scroll bars on the views scroll layer** (partial): the table's offset is a `views::scroll::Scroller` with the
  scroll-area's bar parts. Test: `table::kinds::tests::the_body_scrolls_by_its_bars_and_emits_scroll`.

## Local runs (after the last merge, 10241d82)

skelter-core `--lib tests::` 134/134; skelter-test `harness monitor` 1/1; skelter-widgets `table:: pickers:: tabs::
dnd:: text:: host::` 131/131; compositor `engine caret` 2/2; svelters-style `overflow` 2/2. `cargo check --all-targets`
of the skelter crates, svelters-automation and svelters-egui clean; skelter-platform-win32 and -macos cross-checked
(x86_64-pc-windows-msvc, aarch64-apple-darwin). Workers ran their filtered tests (svelters-automation's suites, skelter-test
core_islands/island/scene_source/automation/harness/queries/menu, skelter-platform-wayland lib 57).

Not run: Win32/macOS tests and counter-mac (CI only), GPU tests, full workspace, GTK/Qt crates, nothing on hardware.

## Not done / open questions (recommendations)

1. text-edit on `RecordCx::fixed()`: needs a scroll setter on `PrepareCx` (the caret-into-view scroll runs in
   prepare). *Recommend* a follow-up: add it, then text-edit becomes `self_scroll` + `cx.fixed(draw_box)`.
2. Table fast scroll: a table scroll still re-records (frozen header and sticky columns would move with a
   `self_scroll` layer). *Recommend* `fixed()` for the header plus a third space (scrolled in y, fixed in x), or keep
   sticky columns re-recorded.
3. Who owns a window's `OverlayStack` and the drag layer: *recommend* the runner in `crates/skelter` pushes overlay
   specs, draws `record_popup` in the layer, routes presses, and puts `DragPreview` into the compositor's drag layer.
4. Hosted kinds have no text system in events (hex box draws no caret/selection): *recommend* the shim lends a
   `Layouter` over the document's `TextSystem`.
5. Automation `Session`/`Driver` still need a svelters-egui scene (MCP acts and waits): *recommend* a follow-up making
   `Session` generic over `SceneSource`.
6. `caret-color`: *recommend* adding it to svelters-style and preferring it over `color`; drop
   `CompositorConfig::caret_color` at the next consolidation.
7. Win32: fullscreen, popup grabs, `attach_window_to_drag`, custom-MIME drags not done; macOS: no drag source, no popup
   grabs. *Recommend* with the tab tear-off work.
8. `before_present_sized` has no target time, so commit-timing-v1 stays unused until the compositor provides one.

Lane records not edited by the workers (SKELTER-test-v2, SKELTER-widgets-tier1): this record answers their open items.
