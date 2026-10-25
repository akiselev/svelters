# SKELTER-test-v2: skelter-test at kittest parity for cadatomic (W-T2)

Lane TEST-V2 of batch presto-b3 (`plans/cloud/presto-b3/LANE-test-v2.md`). Branch `ci/presto-test-v2`, session
`session_01FZ1DXSABohru158URTe1mR`, 2026-10-25. Owns `crates/skelter-test/**`. Based on `ci/presto-base` (merged up to
8a810ca9), with `ci/presto-egui-island` merged in up to 6c97eabe (`IslandHost`, the island API the harness codes against).

## What landed

- **Queries at kittest's semantics** (`a11y.rs`): every tree the app sends is kept; a node with a `tree_id` is a graft
  point and the walk descends into that subtree (egui islands, `skelter_egui_island::A11yGraft`), composing the graft
  node's and the subtree root's transforms into window points. `By` is kittest's: a label query matches a `Label`
  node by its text and any other node by its accessible name (its label, else its `labelled_by` nodes' text), drops
  label nodes that label another match (`include_labels` keeps them), matches hidden nodes (`exclude_hidden` is an
  opt-in), takes predicates over the node (`AkNode`, accesskit_consumer's methods: `label()`/`value()` as `String`,
  `description`, `data`, `is_disabled`, `is_selected`, `is_focused`, `bounding_box`, `locate`), and
  `recursive(false)`. Queries never match their scope node (kittest searches below the root).
- **Every query on the harness and on nodes** (kittest's `Queryable`): `query_all`, `get_all`, `query`, `get`, and
  `*_by_label`, `*_by_label_contains`, `*_by_role`, `*_by_role_and_label`, `*_by_value`, `*_by` (predicate), plus
  `*_by_author_id`. `get_*` panics on none or several, `get_all_*` on none, `query_*` on several. Results are
  iterators (`std::vec::IntoIter`). `root()`, `Node::children`, `children_recursive`, `parent`.
- **Verbs in either vocabulary** (`convert.rs`): keys, points, sizes, modifiers and buttons are skelter's or egui's,
  so `h.key_press(egui::Key::Enter)` and `h.hover_at(egui::pos2(..))` work unchanged; `event` takes a skelter
  `InputEvent` or an `egui::Event`. New verbs: `run_steps`, `run_ok`, `event_modifiers`, `key_down_modifiers`,
  `key_up_modifiers`, `key_combination`, `key_combination_modifiers`, `drag_at`, `drop_at`, `remove_cursor`,
  `set_pixels_per_point`, `check_accessibility`/`unnamed_widgets` (kittest's check; on by default after `run`,
  `with_accessibility_check(false)` turns it off), `accesskit_action`. Node: `click_button`, `click_modifiers`,
  `click_button_modifiers`, `increment_accesskit`, `decrement_accesskit`, `set_value_accesskit`,
  `set_text_value_accesskit`, `replace_selected_text_accesskit`, `scroll_to_me`, `scroll_{up,down,left,right}`,
  `action_with`. `Node::rect()` is kittest's `egui::Rect` (`bounds()` is skelter's `Option<Rect>`); `Node::value()` and
  `label()` return `String`s; `Node::type_text` no longer focuses first (kittest's).
- **The injection adapter, completed** (`egui_event.rs`): held modifiers are tracked (`ModifiersChanged`, keys,
  buttons) and carried by pointer moves and leaves; the harness's own verbs keep its pointer position, so a wheel
  event after `hover_at` lands where the pointer is.
- **`EguiHarness`** (`island.rs`): egui_kittest's `Harness` for egui closures, hosted in a
  `skelter_egui_island::EguiIsland` that fills the window of a real `skelter::App` (`IslandDoc`, a `DocumentApi`).
  skelter input reaches the island through the app's dispatch, AccessKit actions through `handle_a11y_action`, egui
  `Paste` through the shared clipboard and the paste chord. `new_ui`, `new_ui_state`, `new`, `new_state`,
  `builder()` with `with_size`, `with_pixels_per_point`, `with_step_dt` (default 1/4 s, kittest's), `with_max_steps`,
  `with_theme` (default dark), `with_context_setup`, `with_render_every_step`, `with_render_cursor(false)`,
  `with_accessibility_check`, `with_options`, `with_renderer_setup`, `build_ui`, `build_ui_state`, `build`,
  `build_state`; `ctx` (the island's context), `state`, `state_mut`, `into_harness`, `island_output`; everything
  else derefs to `Harness`.
- **`CoreIslands`** (`core_islands.rs`, `HarnessBuilder::build_core_islands(core, ui)`): a skelter-core document whose
  `egui-island` nodes carry islands mounted through `skelter_egui_island::IslandHost` (cadatomic's panes as real
  nodes, W-D1). It takes the island steps the app runner does not take yet: `IslandHost::run` before each frame, the
  passes' AccessKit subtrees handed to the harness (queries descend into them under the kind's graft node, tree id
  `island_tree_id(node)`), `IslandHost::render` before `render`, AccessKit actions for an island's tree routed to it,
  `IslandHost::next_wakeup` as the document's wakeup. Input goes through skelter-core's dispatch (hit testing, the
  kind's forwarding). Not wired: `IslandHost::bridge_events` (in-app drags into islands), since `CoreDocument` hands
  its JS events to the script host.
- **Steps run state changes.** kittest runs a pass on every step; skelter frames only when asked. `Harness::step` now
  asks the app for frames for documents with uncommitted changes (`App::request_dirty_frames`), and
  `EguiHarness::state_mut` asks the island for a pass, so `*h.state_mut() = …; h.run()` shows the change as in kittest.
- **`render` through the real renderer** (`island.rs`, `harness.rs`): before drawing, the island's last pass is
  rendered by egui-wgpu into its texture on the harness's device and registered with skelter-render; then the
  committed tree (the island's `Prim::Surface`) is drawn as before (`HeadlessTarget`; lavapipe with
  `WGPU_BACKEND=vulkan` on Linux CI). `with_renderer_setup` opens the GPU at build so paint callbacks' resources are
  installed on the island's renderer before its first frame.
- **The pixel tier** (`snapshot.rs`): kittest's `snapshot`, `try_snapshot`, `snapshot_options`,
  `try_snapshot_options`, `image_snapshot`, `try_image_snapshot(_options)`, `SnapshotOptions` (`threshold` 0.6,
  `max_failed_pixels`/`failed_pixel_count_threshold` 0, `output_path` `tests/snapshots`), `.new.png`/`.diff.png`/
  `.old.png`, `UPDATE_SNAPSHOTS` (`1` failing, `force` all), dify 0.8's comparison (the same call kittest makes),
  failures of `snapshot` reported when the harness drops, `take_snapshot_results`. Frames are compared unpremultiplied.
- **The automation bridge** (`automation.rs`, feature `automation`): `SkelterDriver<D>` implements
  `svelters_automation::Driver` over a harness and the svelters-egui `SceneHandle` an island renders, so `Session`
  (selectors, acts, waits, JSON/MCP methods) drives a skelter app as it drives the egui backend. It moves the
  session's egui events from scene (island) points into window points and injects them through the adapter (so they
  take skelter's input path), reports the island's grafted AccessKit nodes in island points with the island's tree
  id (semantic acts target the island's tree), crops `screenshot` to the island, and reports the virtual clock.
- **skelter (cross-lane, additive):** `DocumentApi::next_wakeup(window) -> Option<Time>` (default `None`), and the app
  arms a platform timer for the earliest future wakeup and asks for a frame when it fires. Without it an island's
  delayed repaint (egui's `repaint_after`: tooltips, caret blink, animations) never ran: the app requested frames only
  on input and compositor messages.

## Tests (written, not run locally; CI runs them)

- `tests/queries.rs` (fixed tree): the label rule with `labelled_by`, `include_labels`, scoped queries, `children`,
  `children_recursive`, `parent`, predicates over `AkNode`, hidden nodes, `recursive(false)`, the panics.
- `tests/island.rs` (no GPU): queries descend into the island (subtree tree id, bounds inside the window), a click
  through skelter input reaches egui, AccessKit actions reach the island, text, `Backspace` and `Paste` reach a
  focused `TextEdit`, egui-typed `hover_at`/`click_modifiers`/`drag_at`/`drop_at`, an idle island runs no passes,
  `ctx`/`state`/`with_context_setup`/`check_accessibility`.
- `tests/snapshot.rs` (no GPU): missing golden, round trip, diff with `.diff.png`, failed-pixel budget, size mismatch,
  unpremultiplied comparison.
- `tests/island_render.rs` (`SKELTER_GPU_TESTS=1`): an egui rect reaches the pixels, a later pass changes them,
  `render_every_step`, an island snapshot compares with itself.
- `tests/automation.rs`: the counter fixture compiled live from source runs in an island; a `Session` over
  `SkelterDriver` resolves selectors, clicks synthetically and semantically, waits for the label, and the driver's
  AccessKit nodes line up with the scene's nodes. It needs the workspace's JS packages: without them (the skelter
  workflow runs no `pnpm install`) it prints "skipped" and passes; linux-gtk's workspace nextest has them.
- `tests/core_islands.rs`: an island mounted with `IslandHost` on a skelter-core node: queries reach its button, a
  click goes through the document into egui, an AccessKit action on the island's tree reaches it.
- `tests/island.rs` also: a `state_mut` change shows after `run`.
- `tests/harness.rs` adjusted to the kittest-shaped `Node` (`value()`/`label()` as `String`, `bounds()`, `focus()`
  before `type_text`).

## cadatomic's kittest verbs and their skelter-test status

Inventory: `crates/cadatomic-app/tests/*.rs` at cadatomic `master` 75b6a47e, 46 files using egui_kittest (counts are
call sites / files). "Same" = same name and semantics on `Harness` (skelter documents) and `EguiHarness` (egui closures
in an island). "Adapted" = available with a different shape, what to change in the test is stated.

| Verb or query | Uses | Status |
|---|---|---|
| `Harness::builder()` / `HarnessBuilder` | 50/45 | Same (`EguiHarness::builder()`) |
| `with_size` | 50/45 | Same (egui `Vec2` or skelter `Size`) |
| `build_ui_state` | 49/45 | Same |
| `build_ui` | 1/1 | Same |
| `Harness::new_ui_state` | 2/2 | Same |
| `Harness::new_ui` | 1/1 | Same |
| `with_max_steps` | 32/30 | Same; a skelter step is one vsync and input reaches the screen through a commit round trip, so tight budgets (kittest's 4) may need raising: to be seen in the first port |
| `with_step_dt` | 7/7 | Same (the virtual vsync interval; default 1/4 s as kittest) |
| `with_pixels_per_point` | 1/1 | Same |
| `with_render_every_step` | 3/3 | Same (needs an adapter) |
| `with_render_cursor` | 3/3 | Same for `false` (all of cadatomic's calls); `true` panics: skelter never draws the cursor |
| `with_context_setup` | 3/3 | Same |
| `renderer(WgpuTestRenderer::from_render_state(rs))` | 21/21 | Adapted: `with_renderer_setup(\|device, queue, renderer\| …)` installs paint-callback resources on the island's egui-wgpu renderer, on the harness's device (the test's own `create_render_state` device is not used) |
| `create_render_state`, `default_wgpu_setup` (egui_kittest::wgpu) | 42 imports | Adapted: not needed; the harness opens its device (`Harness::gpu()` after the first render, or at build with `with_renderer_setup`) |
| `run` | 379/42 | Same (then kittest's accessibility check, on by default) |
| `run_ok` | 2/2 | Same |
| `try_run` | 1/1 | Same (`Err(NotSettled)` instead of `ExceededMaxStepsError`) |
| `run_steps` | 113/11 | Same |
| `step` | 131/36 | Same name; a step is one vsync with all queued input delivered at event time (kittest runs one pass per queued event) |
| `state` / `state_mut` | 690/45, 137/39 | Same on `EguiHarness` (the closure's state); `Harness::state` is the document |
| `ctx` (field) | ~25 | Same on `EguiHarness` (the island's context) |
| `event(egui::Event)` | 96/23 | Same (translated by the adapter: `PointerMoved`, `PointerButton`, `Key`, `Text`, `Ime` preedit/commit, `MouseWheel`, `Paste`, `Copy`, `Cut`, `ModifiersChanged`, `Touch`, `Zoom`, `Rotate`, `WindowFocused`, AccessKit requests) |
| `event_modifiers` | 3/2 | Same |
| `key_press` | 72/14 | Same (egui or skelter `Key`) |
| `key_press_modifiers` | 41/12 | Same |
| `key_down`, `key_up`, `key_combination(_modifiers)`, `key_down/up_modifiers` | — | Same (not used by cadatomic today) |
| `hover_at` | 16/9 | Same |
| `drag_at` | 5/4 | Same (primary press at the point) |
| `drop_at` | 5/4 | Same (release, then the pointer leaves) |
| `remove_cursor` | 6/1 | Same |
| `set_size` | 2/2 | Same (through skelter's resize handshake) |
| `render` | 37/17 | Same name, returns `Result<skelter_test::Image, RenderError>` (premultiplied RGBA8, device pixels) instead of `image::RgbaImage`; `snapshot::to_rgba_image` converts |
| `snapshot` / `try_snapshot` / `SnapshotOptions` | 0 (cadatomic's goldens use their own `snapshot()` helper over `tests/snapshots/*.png`) | Same API as kittest, ready for those goldens |
| `check_accessibility` | 15/5 | Same |
| `root` | 15/6 | Same (`root().children_recursive()`) |
| `input()` (`predicted_dt`) | 1/1 | Adapted: `h.interval()` is the step's virtual time |
| `input_mut().events.extend(..)` | 2/2 | Adapted: `h.event(e)` for each event |
| `output()` (`platform_output.commands`) | 8/5 (some are cadatomic's own `output()`) | Adapted: `island_output()` gives copied text, opened links, cursor, IME (skelter-facing) |
| `get_by_label` | 100/26 | Same |
| `query_by_label` | 48/16 | Same |
| `query_all_by_label` | 44/15 | Same |
| `get_all_by_label` | 2/1 | Same |
| `get_by_label_contains` / `query_by_label_contains` / `query_all_by_label_contains` | 11/7, 12/7, 12/7 | Same |
| `get_by_role_and_label` / `query_by_role_and_label` / `query_all_by_role_and_label` / `get_all_by_role_and_label` | 83/20, 10/6, 3/2, 2/1 | Same |
| `get_by_role` / `query_by_role` / `query_all_by_role` | 1/1, 2/1, 21/7 | Same |
| `get_by_value` / `query_all_by_value` | 1/1, 5/1 | Same |
| `get_by` / `query_by` / `query_all_by` (predicate) | 8/3, 1/1, 1/1 | Same; the predicate gets skelter-test's `AkNode` (accesskit_consumer's methods) |
| `query_all(kittest::by().predicate(..))` | 1/1 | Same (`skelter_test::by()`) |
| `get` / `query` on nodes | 43/21 | Same |
| `Node::click` | 129/33 | Same |
| `Node::click_modifiers` | 3/2 | Same |
| `Node::click_accesskit` | 2/1 | Same |
| `Node::hover` | 4/4 | Same |
| `Node::focus` | 28/11 | Same |
| `Node::type_text` | 31/15 | Same (an IME commit; no focus change) |
| `Node::rect` | 37/20 | Same (`egui::Rect`, window points) |
| `Node::is_focused` | 1/1 | Same |
| `Node::children`, `children_recursive` | 1/1, 2/2 | Same |
| `Node::accesskit_node()` → `.role`, `.label`, `.value`, `.is_disabled`, `.is_selected`, `.description`, `.data`, `.author_id`, `.is_focused` | 64/17 | Same |
| `NodeT`/`Queryable` trait imports | ~80 | Adapted: inherent methods; drop the `use egui_kittest::kittest::{NodeT, Queryable}` lines |

## Not done (left out, not faked)

- **No `kittest.toml`**: snapshot options come from `SnapshotOptions` and the builder only.
- **`with_render_cursor(true)`** panics (no cursor in skelter's render), `fit_contents`, `with_os`,
  `wait_for_pending_images`, recording/video, `diagnostic_max_steps` and repaint causes: not implemented; cadatomic
  uses none of them.
- **Island AccessKit updates bypass the commit.** `Commit::a11y` carries one `TreeUpdate`, so `IslandDoc` hands the
  island's grafted update to the harness through a hook (`Hooks::a11y`); the host tree (root and graft node) goes
  through the commit and `update_a11y` as before. In production the subtree has no path to the platform adapter yet
  (question 1).
- **The automation bridge needs a svelters-egui scene.** Selectors run on svelters-egui's retained scene, so the
  driver covers Svelte UI in islands (cadatomic during the migration). Svelte UI rendered natively by skelter
  (svelters-skelter) needs the engine generalised over a scene source (question 2).
- **The tour live driver re-target** (plan §2.4, W-T2) is cadatomic's (`tours.rs` runs on its `KittestDrive`); with
  `SkelterDriver` it can drive skelter once cadatomic owns the skelter window (W-D1). Not started here.
- Physical springs in the headless compositor and the other APP-TEST gaps stand as recorded in `SKELTER-app-test.md`.

## Cross-lane edits

- `crates/skelter/src/seams.rs`: `DocumentApi::next_wakeup` (a default method; no implementor changes).
- `crates/skelter/src/app.rs`: the app's handlers wrap their bodies (`input_one`, `window_event_one`, `a11y_event_one`)
  and arm a platform timer for documents' wakeups after each; `timer` asks every window with a due frame for one
  (`App::request_dirty_frames`, public: the harness calls it each step). Reason: an island's delayed repaints never
  ran, and a host's state change outside dispatch never got a frame.
- Merged `ci/presto-egui-island` (0dd5a347): this branch carries that lane's commits unchanged.
- `Cargo.lock`: skelter-test's new edges (skelter-egui-island, skelter-platform, egui-wgpu, dify, image, and, for tests and
  the `automation` feature, svelters-automation, svelters-egui without default features, svelters-host,
  svelters-modules), all versions already in the lock.

## Builds and tests run

- `cargo check -p skelter -p skelter-test --all-targets` (with the self dev-dependency turning `automation` on): clean.
- CI run 40230141525 (skelter, b0754e75): island rendering worked on lavapipe, WARP and Metal (the first frame's
  pixels, the snapshot round trip, `render_every_step`); one assertion failed (a `state_mut` change never got a frame,
  fixed above) and `automation` failed for lack of `pnpm install`. The svelters workflows were green on 07a4a30f
  (linux-gtk's workspace nextest included skelter-test). The run's other failures (`skelter-compositor` engine and
  tree_damage, `skelter-core` and `skelter-widgets` lib tests) are in other lanes' crates.
- **The `island_render` failure (`blue after the next pass`, also on integration 0890801c, run 40241095094) was the
  harness, not the island:** the test changed the closure's state and nothing asked the app for a frame, so no second
  pass ran; the island's re-record and texture update were never reached. Fixed in 9a4f1213 (`Harness::step` asks for
  frames for dirty documents, `EguiHarness::state_mut` requests a pass, the test uses `run`). skelter run 40239572275
  on 9a4f1213: green on Linux, Windows and macOS, `island_render` included.
- After those fixes, one local run of the non-GPU tests this lane touched (`harness`, `queries`, `island`,
  `snapshot`, `core_islands`, `automation`, with `pnpm install`): 41 passed. `island_render` and `render` not run
  locally (no Vulkan driver in the container).
- The skelter workflow now also builds svelters-automation, svelters-egui (no default features), svelters-host
  (QuickJS-NG, C) and svelters-modules with `source` (the compiler) for skelter-test's tests: more build time on all
  three OSes. If that is too slow, the automation test can move behind its own feature, run in linux-gtk's workspace job.

## Questions for the coordinator (each with a recommendation)

1. **AccessKit subtrees in commits.** An island (or offscreen viewport) produces its own `TreeUpdate`, which the
   platform adapter needs alongside the host's. Recommend an additive `Commit::a11y_subtrees: Vec<TreeUpdate>`
   (skelter-base; struct-literal builders use `..Commit::default()`, so it breaks nothing in tree) and that the app
   sends each through `update_a11y`. skelter-test then drops its hook and reads them from the platform calls.
2. **Selectors on skelter-native UI.** svelters-automation's engine resolves selectors on svelters-egui's scene. For Svelte
   UI rendered by skelter itself, recommend a `SceneSource` trait in svelters-automation (nodes, roles, names, rects,
   styles, journal) implemented by svelters-egui's scene and by skelter-core's `Document`, so `SkelterDriver` can drop
   the island requirement. It is a design choice in svelters-automation (AU1's owner), hence this question.
3. **Step budgets.** cadatomic's tests set `with_max_steps` for kittest's one-pass-per-event model; skelter needs a
   vsync or two more for the commit round trip. Recommend that the cadatomic port raise budgets where CI shows
   `NotSettled`, rather than skelter-test scaling them silently.
