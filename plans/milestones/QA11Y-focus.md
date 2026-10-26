# QA11Y-focus: one focus owner, and focus that reaches AccessKit (svelters batch qc-a11y, wave 1)

Lane FOCUS, branch `ci/qa11y-focus` from `origin/claude/determined-gates-c0yj5i` (`9e3781e5`). Brief:
`plans/cloud/qc-a11y/BRIEF.md`, `LANE-focus.md`; design: `PLAN-focus-and-text.md` §1.1, §1.3, §2, §3.3, §5, §6, §8.
Steps: Q1's focus half, F1–F5. `origin/master` merged (nothing new since the branch point, 2026-10-26).

## What landed

| Step | Commit | What |
|---|---|---|
| Q1 | `9ac6d4ab` | `skelter_test::consumer`: `Consumer` folds `TreeUpdate`s into a real `accesskit_consumer::Tree` (`is_host_focused = true`) and records focus moves; on Linux an `accesskit_atspi_common::Adapter` beside it whose `AdapterCallback` records registered interfaces and events, and whose action handler records requests (`Atspi`). `HarnessBuilder::with_consumer(true)`, `Harness::consumer()`. |
| F1, F2 | `dea59e35` | Deferred focus requests; `focus::after_style` after the frame's restyle (the request, dialog scopes appearing/going away, autofocus, the focus fixup rule); the generalised restore stack; `initial-focus` (opt-in). svelters-skelter delivers the focus events a frame queued right after it. |
| F3 | `042548c2` | `FocusTarget { node, sub }`; the graft guard (`Document::a11y_subtree_sent`); ancestor fallback; root `scale(window.scale)`; AT-SPI window bounds at adapter creation; skelter-test's `A11yTree` stays in points. |
| F4 | `f2302ccc` | ROLES additions; `aria-checked`, `aria-haspopup`, `aria-modal`, `aria-activedescendant`, `aria-controls`; svelters-skelter Widget props (and `initial-focus`), `metadata/semantics/skelter.json` regenerated. |
| F5 | `c892fb25` | skelter-egui-island's tab-out protocol (`EguiIsland::focus_entered`, `IslandOutput::tab_out`, `IslandHost::run` moves focus on); skelter-core `Document::tab`, `EventCx::tab_direction`. |

Added on the coordinator's request (2026-10-26), in the commit after the record's first version:
- **The AccessKit fork** (owner's decision, replaces plan §2.5's "file upstream"): root `Cargo.toml` `[patch.crates-io]`
  takes `accesskit`, `accesskit_atspi_common` and `accesskit_consumer` from `github.com/akiselev/accesskit` rev
  `b22e1418` (0.25.2 / 0.21.1 / 0.39.2). The fork's AT-SPI layer depends on its own `accesskit` and
  `accesskit_consumer` by path, so patching only `accesskit_atspi_common` would have linked two copies of AccessKit's
  types; all three come from the rev, one copy each in `Cargo.lock`. Test: skelter-test `tests/focus_a11y.rs`
  `aria_expanded_reaches_atspi_as_expandable_and_expanded` (Expandable when collapsed, Expanded when open,
  `StateChanged(Expanded, true)` on the toggle).
- **Lane text's request:** `Document::set_scroll_offset` (the path wheel, `scroll_to` and compositor flings through
  `apply_scroll` all take) marks a self-scrolling node `A11Y`, so its text runs re-send with the offset. Test: skelter-core
  `tests/hooks.rs` `a_kind_that_scrolls_itself_scrolls_on_the_compositor` (extended).

### The Q1 helper's API (lane text uses it)

`skelter_test::consumer::Consumer`: `new()` (consumer tree plus, on Linux, AT-SPI) or `tree_only()`; `apply(&TreeUpdate)`
(host update first; subtree updates before the first host update wait for it); `set_window_bounds(outer, inner)`;
`updates()`, `tree()`, `state()`, `focus()` (effective focus: through grafts, active descendant applied),
`focus_located()`, `focus_author_id()`, `focus_moves()` / `take_focus_moves()`, `node((NodeId, TreeId))`,
`by_author_id(&str)`, `find(|n| …)`. Linux: `atspi() -> Option<&Atspi>`: `node(FullNodeId) -> PlatformNode` (AT-SPI's
Text, Component, … methods), `interfaces(id)`, `log()` (events, interfaces), `take_events()`, `focused_events()`
(targets of `StateChanged(Focused, true)`), `take_actions()` (requests made through `PlatformNode`, deliver them with
`Harness::accesskit_action`). Re-exports `CoordType`, `Interface`, `AtspiRect`. The consumer and the adapter number trees
in update order, so their `FullNodeId`s agree.

## Behaviour (what changed for apps)

- **`Document::focus(node)`** focuses now when it can. A node that only is hidden (it or an ancestor: `hidden`,
  `display: none`, `visibility: hidden`, concealed) keeps the request; the frame applies it after its next restyle, or
  drops it if the node is still hidden then. The request marks the node `A11Y`, so that frame comes even when nothing
  else changed. `Document::pending_focus(window)` shows it. Focus events fire in the frame that applies it.
- **After every restyle** (`frame.rs`, before layout): the pending request; then each watched node (`focus-scope=dialog`
  or `autofocus`, kept in `Document::focus_watch`, so this is one pass over those nodes only):
  - a `dialog` scope that became available (in its window, shown, not inert, its content not inert) records the opener
    (the window's focus before this pass, unless already inside) on the restore stack and focuses its first `autofocus`
    node that can take focus, else its first tab stop. Focus already inside (the app's own `focus()`) stays;
  - one that stopped being available (hidden, inert, no longer a dialog scope) pops its entries and gives focus back to
    the opener when focus was inside it or lost; removal keeps the existing path (`scopes_closed`);
  - with **`initial-focus`** (new common prop) on the window's root, an `autofocus` node outside dialog scopes takes
    focus once when it appears (the first frame included), only if nothing in the window has focus;
  - **the focus fixup rule:** a focused node that can no longer take focus loses it: to its available dialog scope's
    initial node, else the restore stack's top that can take focus, else nothing (with `blur`).
- `open_dialog` still works directly (and the frame then counts the scope as open).
- **`TreeUpdate.focus`**: the `FocusTarget` resolved — the node, or `virtual_id(node, sub)` when `sub` names one of its
  virtual children (`Document::set_focus_sub`, `EventCx::set_focus_sub`; cleared on every focus move); a graft node
  only after `Document::a11y_subtree_sent(node)`; else the nearest ancestor the tree has; the root when nothing has
  focus. `validate` keeps the virtual children of known owners as known.
- **Bounds**: the root's AccessKit transform is `scale(window.scale)` (OS scale × zoom), so AccessKit sees physical
  window pixels; `resize_window` marks the root `A11Y` when the scale changes (only the root re-sends).
- **ARIA props** apply after the kind's node; `aria-activedescendant` and `aria-controls` name anchors
  (`Document::find_anchor`); references the tree doesn't have are dropped in `validate` (accesskit_consumer unwraps
  `controls`).
- **Islands**: the `egui-island` node is one Tab stop whose inside egui manages: Tab entering it focuses egui's first
  widget (Shift+Tab: its last); while it has focus Tab goes to egui; when egui runs off its last (first) widget the
  island reports `tab_out` and `IslandHost::run` calls `Document::tab`. A widget's `lock_focus` (a code editor) keeps Tab.

## Tests

- skelter-core `src/tests/focus.rs`: `focus_under_a_backdrop_shown_in_the_same_transaction_applies_after_restyle` (both
  orders; `TreeUpdate.focus` names the field), `focus_on_a_node_still_hidden_after_restyle_is_dropped`,
  `hiding_the_focused_nodes_container_moves_focus_to_the_restore_entry_or_none`,
  `a_dialog_that_appears_focuses_its_autofocus_node_and_gives_focus_back_on_hide`,
  `a_dialog_focused_into_before_it_shows_still_restores_the_opener`, `initial_focus_is_opt_in`,
  `autofocus_outside_a_dialog_takes_focus_once_and_never_steals_it`,
  `a_graft_is_never_named_before_its_subtree_went_out` (the release-panic guard, plan risk 1),
  `a_focused_virtual_child_is_the_trees_focus`, `the_root_transform_is_the_window_scale`.
- skelter-core `src/tests/a11y.rs`: `menu_roles_and_the_aria_props_of_qa11y_f4`.
- skelter-core `src/tests/hooks.rs` `no_caret_for_an_editor_that_isnt_rendered`: adjusted, the hidden editor now loses
  focus (fixup rule) and the test focuses it again once shown.
- skelter-test `tests/harness.rs`: `the_consumer_follows_focus_and_atspi_announces_it`.
- skelter-test `tests/focus_a11y.rs` (consumer and AT-SPI): `tab_order_wraps_and_a_dialog_gives_focus_back` (AT-SPI's
  `StateChanged(Focused)` for the field, then the opener), `programmatic_focus_under_a_backdrop_reaches_the_consumer`,
  `bounds_follow_the_ui_zoom` (atspi_common `extents(CoordType::Window)` = world rect × 1.25),
  `focus_on_an_island_waits_for_its_subtree_and_never_panics_the_consumer`,
  `an_active_descendant_is_what_assistive_technology_sees_focused`,
  `tab_enters_an_island_moves_through_its_widgets_and_leaves_it`, `a_widget_that_locks_focus_keeps_tab`.

Run locally (all passed, filtered): the tests above; `cargo test -p skelter-core --lib` once after F1 (to check the fixup
rule against existing tests: one adjusted, see above); skelter-test `--test harness --test core_islands --test queries
--test island`, skelter-egui-island's tests, svelters-skelter `--test spec` (with `SVELTERS_UPDATE=1`). **Not run
locally:** the full workspace, other crates' suites (skelter-widgets, svelters-skelter beyond `spec`, skelter), clippy,
conformance; CI is the gate. CI: see the end of this record.

## Deviations from the plan, with reasons

1. **Deferral is not limited to stale style** (plan §2.2 item 1 says "fails only because style is stale"): a request on
   a node that only is hidden is kept until the next restyle whatever the order of `focus()` and the style change in
   the transaction. cadatomic's palette calls `focus()` before clearing `display: none` (§1.1 c); with this it works
   in either order. A request still hidden after that restyle is dropped, and the request marks the node so a frame
   comes promptly (no request lingers to apply later).
2. **The initial-focus opt-in gates all `autofocus` outside dialog scopes,** not only the first frame: with Svelte the
   first frame may be empty and content mounts a frame later, so a first-frame-only gate would not keep other apps'
   behaviour. Dialog scopes taking focus when they appear is always on (it is documented svelters-skelter behaviour,
   `focus-scope` and `autofocus` docs, that the skelter port lost). `autofocus` outside a dialog takes focus only when
   nothing has focus (the DOM's rule), so it never steals it.
3. **The graft guard lives in `a11y_update`'s focus resolution, not in `validate`:** `validate` sees only one update and
   can't know whether a subtree went out. The "subtree sent" bit is set by whoever sends `Commit::a11y_subtrees`
   (`Document::a11y_subtree_sent`; `CoreIslands` does), not by `skelter::App`, which is generic over `DocumentApi`.
4. **Island Tab is answered after the pass, not at dispatch** (plan §2.1: "consumed only if egui moved focus without
   wrapping"): egui doesn't expose its lock filter, so the island forwards Tab to egui and decides from egui's focus
   after the pass (Shift+Tab one pass later, through egui's AccessKit order, since egui moves backwards focus at the
   next pass). The move out lands one frame after the key. This respects `lock_focus` exactly (plan risk 2).
5. **Overlays** keep their own restore (`Closed::restore_focus` in `toplayer.rs`); the restore stack covers `dialog`
   scopes. An overlay part's node that becomes unfocusable is covered by the fixup rule.
6. **skelter-test `A11yTree` bounds stay in document points** by skipping the host root's transform. Pre-existing and
   unchanged: under a UI zoom other than 1 the harness's node verbs (`click()` at a node's centre) use document points
   as window points; tests that zoom click with `click_at` in window points.
7. `none`/`presentation` map to `GenericContainer` (AccessKit has no presentational role; its filters drop unnamed
   generic containers). `separator` stays `Splitter` (unchanged; AccessKit has no static separator role).

## Risks left

- `aria-controls` names a node that later leaves the tree while the referring node isn't re-sent: accesskit_consumer's
  `controls()` unwraps. Same exposure as the existing `labelled_by`. A reverse index (re-send referrers when an anchored
  node leaves) is the fix if it shows up.
- svelters-skelter `frame` now pumps once more after `Document::frame` (focus events of the frame). JS that then changes
  the tree gets the next frame as usual.
- `lane text`: both lanes touch `skelter-core/src/cx.rs`. Mine: `focus_anchor` (pending counts), `tab_direction` and
  `set_focus_sub` between `focus()` and `set_cursor()`. Lane text adds `request_a11y` (likely after `request_paint`):
  a merge may need a hand at that spot.

## Open questions (with a recommendation each)

- **Upstream (public, coordinator → user):** nothing filed. The Linux expanded state is the AccessKit fork, a later step.
- **cadatomic's `ShellDocument` should call `Document::a11y_subtree_sent`** for each island whose subtree it puts on a
  commit (wave 2, below). Recommendation: yes, the same three lines as `CoreIslands::frame`. Alternative: a
  `DocumentApi` hook in `skelter::App` — more general, but every document type would have to forward it.
- **Should `initial-focus` be the default for svelters-skelter apps?** Recommendation: no, keep opt-in (risk 8); cadatomic
  sets it on its windows.

## cadatomic wave 2 notes (what cadatomic must adopt)

1. **Graft guard:** after pushing an island's subtree update into `Commit::a11y_subtrees`, call
   `doc.a11y_subtree_sent(island_node)` (see `skelter-test/src/core_islands.rs` `frame`). Without it focus on the island
   node reports the island's parent forever (safe, never a panic, but AT never enters egui).
2. **Island focus (C1):** make `cadatomic-island` focusable and drive its egui flag from `Event::FocusIn`/`FocusOut`; to
   get the tab-out protocol, use the same pattern as `EguiIslandKind::event`: on `FocusIn` read `cx.tab_direction()` and
   call `island.focus_entered(backwards)` when it is `Some`; forward keys to the island while the node has focus; after
   each pass apply `IslandOutput::tab_out` with `doc.tab(window, backwards)` (re-enter with `focus_entered` if focus
   stayed on the node). Remove the direct `set_focused` calls.
3. **Initial focus (C2):** set `initial-focus` (Bool) on each window's root and `autofocus` on the main working surface
   (the main island; the start page's first action).
4. **Palette (C3):** `focus-scope=dialog` on the palette panel. `focus()` on the field may now come before or after the
   class change that shows it, in one transaction. Focus applies at the next frame (read `doc.focused` after a frame,
   or `doc.pending_focus` before). Use `aria-activedescendant` (the selected row's `anchor`) on the field, `role=listbox`
   on the list, `role=option` on rows. When the palette hides, focus goes back to the opener by itself.
5. **Menus (C4):** roles `menubar`, `menuitem`, `menuitemcheckbox` (+ `aria-checked` `true`/`false`), `menuitemradio`,
   `menu`, `separator`; `aria-haspopup=menu` (or `true`) and `aria-expanded` on the title-bar buttons; each menu level a
   `focus-scope=dialog`.
6. **Root scale (plan risk 6):** skelter now puts `scale(os × zoom)` on the host root's AccessKit transform. The probe
   (`probe.rs`, which multiplies by `scale` itself) and the tour's `A11yFold` (`tour/generic/skelter.rs`) must skip the
   host root's transform, as skelter-test's `A11yTree::walk` does now (`parent.is_none()` → don't apply), or stop
   multiplying by `scale`, or they double-scale.
7. **Fixup rule:** a focused node that gets hidden now loses focus (to its dialog's first stop, the restore stack, or
   nothing). Code that hid a focused field and expected it to keep focus while hidden must refocus it when shown.
8. **Dialog scopes** (`focus-scope=dialog`) now take focus when they become shown, not only through `open_dialog`.
9. **The AccessKit fork:** cadatomic needs the same `[patch.crates-io]` block (all three crates, same rev
   `b22e1418f323f15c93ed0d76e16d4c097ba6cce0`, not only `accesskit_atspi_common`) and a `forks.toml` row for
   `akiselev/accesskit`.
10. **`FocusTarget.sub`** is the hook for canvas semantic objects (§2.3, §7): a kind that publishes virtual children calls
   `cx.set_focus_sub(Some(i))` while focused.

## CI

(filled in below after the last push)
