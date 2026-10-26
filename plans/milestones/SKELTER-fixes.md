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

## Coordinator requests (batch 12 lanes), 2026-10-26

Merged to master so far: 4cc51839 (6e1c17c2), 4a837595 (e98d2a64), a867e915 (4bf37234). CI note: a push that
touches only skelter crates runs only the skelter workflow (paths-ignore of the five svelters ones).

| Item | What | Test |
|---|---|---|
| capture on move | a same-window move keeps capture, focus, hover | `tests::tree::a_move_inside_the_window_keeps_capture_focus_and_hover` |
| OverlayStack | per-window stack in skelter-core's Document; `RecordCx::overlay`, light dismiss, Escape, `Kind::overlay_closed`; `Document::set_window_overlay` app list; drag layer moved by fast commits | `tests::overlay` (3), skelter-test `overlay.rs` (3) |
| A1 | app kinds: `Kinds::with_app`, surface specs from core KindSpecs, `spec::fragment` | svelters-skelter `app_kinds.rs` |
| A2 | `use:` behaviours: `SkelterBackend::register_behavior`, `add_event_tap` | `behaviors.rs` |
| A3 | common `aria-live`, `aria-busy` | `tests::a11y::aria_live_and_busy_apply_after_the_kinds_own_node` |
| A4 | `App::set_zoom` (scale = os × zoom; pointer/IME converted) | `harness::a_zoom_rescales_...` |
| A5, A5b | EguiHarness kittest parity: a pass per step, kittest's settle rule, one event per pass; `with_pass_every_step(false)`; out-of-pass repaint callback | skelter-test `island.rs` |
| A6, A7 | build-time a11y check; PREDICTABLE renderer + `with_renderer_options` | `island.rs`, `island::tests` |
| A8 | select options as AccessKit virtual children (`A11yCx::add_virtual_child`, `Event::A11yChild`), `option-anchors` | core a11y, widgets select, skelter-test `select.rs` |
| A9, A10 | tier-1 kinds on svelters-skelter (scroll-area, collapsing-header*, table, tree-view, combo-box, context-menu, action*); `kinds::register_builtin` | `tier1.rs`, `host_document` |
| A11 | `CoreDocument<H: DocHandle>`, `CoreDocument::shared(Rc<RefCell<Document>>)` | skelter-test `shared_doc.rs` |
| A12 | `DocumentApi::commit_applied` (Document::busy no longer keeps COMMIT_IN_FLIGHT) | `shared_doc.rs` |
| A13 | Theme service reaches stock kinds (`StyleView::with_theme`, `host::set_theme`, `Document::repaint_all`) | `host::tests::the_documents_theme_reaches_a_stock_buttons_tokens` |
| A14 | raw pointer motion (`InputEvent::RawMotion`, `PlatformCx::set_raw_motion`) on Wayland/Win32/macOS/winit | 7 tests; Win32/macOS ones CI only |
| A15 | `RecordCx::external_layer` in paint order | `tests::commit::external_layer` |
| A16 | a frame sends prop/layer removals before additions (moved nodes reused freed ids and drew at the origin) | `tests::absolute::a_subtree_under_...` (5 cases) |
| A17 | common `role`, `aria-level`, `aria-expanded`, `aria-selected`, `aria-description` | `tests::a11y::role_level_...` |
| A18 | svelters-skelter wire ids masked to 28 bits (host flag bits) | `host_document::a_skelter_click_runs_the_svelte_onclicked_handler`, `spec::no_wire_id_reaches_the_flag_bits` |
| A19 | `Harness::set_textures`, public `Registrations`/`register_textures` | `render::a_documents_own_textures_hook_runs_before_render` (GPU) |
| A20 | `Harness::take_calls`, `with_calls_cap` | `harness::calls_can_be_taken_and_capped` |
| A21 | island pass time monotonic | island `a_pass_earlier_than_the_last_does_not_move_time_backwards` |
| A22 | Kind::layout_style sees the style just computed (inline box layout CSS) | widgets `box_::inline_css` |
| A23, A24 | `PointerInput.history`; `Event::Gesture` to the hovered node | `tests::dispatch::canvas_input` |
| A26 | a will-change/animated layer gets a continuation layer (later siblings stay above it) | `tests::commit::a_will_change_layer_keeps_later_siblings_above_it` |
| A27 | `:state()` in sheets resolves through the host's provider states (`Document::set_state_names`) | `tests::style::a_state_rule_matches_the_hosts_state_bit`, svelters-skelter `ops::a_state_rule_in_an_app_sheet_matches_the_hosts_bit` |
| A29 | in-flow nodes created after the first frame draw at their parent's origin: the cause is A16's freed-id reuse (destroy + create, or Svelte's remove-and-insert in one frame); fixed by A16 (in master since 4bf37234), so no code change. With A16 disabled, the destroy + recreate case fails; inserting a row, inserting a leaf into a nested slot, and showing after display:none pass either way | `tests::absolute::in_flow_nodes_created_after_the_first_frame_draw_where_they_are_laid_out` (5 cases, checked against the committed transform locals) |
| (1) | `PrepareCx::{scroll_offset, scroll_to, set_caret}`; text-edit self-scrolls with its box fixed | `a_prepare_scrolls_...`, `text::kind::tests::a_long_line_scrolls_...` |
| (4) | a Layouter over the document's TextSystem for hosted kinds; hex box caret | `pickers::kinds::tests::the_editing_hex_box_...` |
| (5) | automation `Session` over `SourceDriver`; skelter-test `SkelterNativeDriver` | `native_session.rs` |
| (6) | `caret-color` in svelters-style; `CompositorConfig::caret_color` removed | `cascade::caret_color_auto_or_a_colour_inherits` |

\* placeholder/incomplete, being made real in A9b. In progress: A9b, A25 (virtual-list). Queued: A28 (window overlay carrying AccessKit anchors, low).

Build note: `.cargo/config.toml` sets `build-dir = {workspace-root}/target/build`; a second checkout needs both
`CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` set, and a target shared between checkouts mixes their sources.

## Local runs (after the last merge, 10241d82)

skelter-core `--lib tests::` 134/134; skelter-test `harness monitor` 1/1; skelter-widgets `table:: pickers:: tabs::
dnd:: text:: host::` 131/131; compositor `engine caret` 2/2; svelters-style `overflow` 2/2. `cargo check --all-targets`
of the skelter crates, svelters-automation and svelters-egui clean; skelter-platform-win32 and -macos cross-checked
(x86_64-pc-windows-msvc, aarch64-apple-darwin). Workers ran their filtered tests (svelters-automation's suites, skelter-test
core_islands/island/scene_source/automation/harness/queries/menu, skelter-platform-wayland lib 57).

Not run: Win32/macOS tests and counter-mac (CI only), GPU tests, full workspace, GTK/Qt crates, nothing on hardware.

## Not done / open questions (recommendations)

Later (2026-10-26): (2) and (8) stay follow-ups (coordinator). (7) Win32 fullscreen/grabs/tear-off/custom-MIME and
macOS drag source need hardware. New: ordinary will-change/animated layers paint later siblings below them (A15's
splitting would fix it); Wayland pointer lock (zwp_pointer_constraints_v1) for orbiting; JS `rawmotion` signal when
a Svelte viewport needs one; table and picker looks still ignore the Theme service; text-edit's scrolled text is
clipped at the padding box.


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
