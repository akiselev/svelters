# EM2: viewports and moves (EMBED, for cadatomic's DK3/DK4)

Design: `plans/research/ARCH-EMBED.md` ("Decided": seam items 2–4; §5.2, §5.3, §7.1, §10 EM2 row) and
`plans/research/ARCH-SCENE.md` "Decided" (R17, R20, R21). Starts from master `8ce6d7d0` (EM1 + SG1). Lane: EM2
(EMBED writer), branch `ci/em2`.

## What landed

### API (`svelters_egui::embed`)

```rust
// deferred viewports (§5.2): a Send + Sync closure draws through a painter or the thread-local registry
let painter = embed.painter(slot);                     // SlotPainter: Copy + Send + Sync (embed id, slot, UI thread)
ctx.show_viewport_deferred(vp, builder, move |ui, _| { painter.show(ui); });   // None off the UI thread or once gone
svelters_egui::embed::with_current(|e| …);             // the thread's current embed (Embed::make_current, else the oldest)
svelters_egui::embed::with_embed(id, |e| …);           // Embed::id() -> EmbedId
embed.attach(&ctx);                                    // pass callbacks on a context (new/logic do it; runners do it early)

// the dock seam (§7.1)
embed.set_viewport(slot, vp);    // repaint routing, focus restore at the next draw, root().viewport/scale (SV18)
embed.set_visible(slot, false);  // no layout/walk/repaint; its rAF callbacks wait (also those from handlers)
embed.move_root(root, to_slot)?; // reparent into an empty slot at the next logic, no remount, same egui ids
embed.request_close(root);       // -> EmbedEvent::CloseDecided { root, allow } (EM1, unchanged)
embed.show_viewport_passes(ui);  // R21 call site: end of each viewport's ui (passes are SG4's)
embed.take_warnings();           // SlotWarning::{DrawnWhileHidden, NotDrawn { passes: 60 }, WrongViewport}
embed.has_slot(slot); embed.stats().moves;
```

`EmbedConfig::prepass_plugin` (default `true`) now works. With it, the embed's begin-pass callback runs `logic`
before each pass of a deferred viewport. Input in window 2 is then handled in window 2's next pass, with no
root-viewport frame. With `false` (cadatomic, seam 4: the host owns the per-frame order), only the root's
`App::logic` runs JS, and input in a deferred window costs one root frame, whose logic routes the repaint back to
that window.

### Repaint routing (§5.1 step 7, §5.5)

- Every host op records the scene root it touched (`ops.rs`: `touched(Some(root))`). At the end of `Embed::logic`,
  `route_repaints` asks `request_repaint_of` for each viewport showing a touched root. It skips the viewport whose
  pass is running (it draws the change in that same pass). It skips hidden slots. Windows (the desktop path) map to
  the root viewport. The backend keeps the map `RootIx → Option<ViewportId>` (`root_vp`), updated by
  `create_slot`, `set_viewport`, `set_visible` and `destroy_slot`. `touched()`'s blanket `request_repaint()` is
  gone.
- Pending JS work (outbox, rAF, wakes, deferred writes) asks for the viewport that runs logic next
  (`Shared::logic_vp`). That is the root viewport, or the deferred viewport whose pass is running with the plugin.
  The waker reads it too, because it may run on another thread. Timers (`deadline`, `schedule_wake`) always go to
  the root viewport.
- **Bug fixed (EM1/G1):** `before_pass` short-circuited `first || wake || due || checkpoint_pending.replace(false)`.
  After a wake, `checkpoint_pending` therefore stayed set, which cost one extra frame per interaction.

### Focus, scale, debug checks (`show_slot`)

- After each draw, the slot records the node inside it that has keyboard focus in that viewport (a scan only when
  the focused id changed). `set_viewport` and `move_root` turn that into a `request_focus` before the slot's next
  walk, in the new viewport. `root().focused` is sent to JS (it was always false in EM1).
- `root().scale` is the drawing viewport's `pixels_per_point`. When it changes, that slot's root is marked fully
  dirty (one relayout of that slot only).
- Debug checks (§5.3), each reported once until the slot behaves again (printed in debug builds with
  `echo_errors`):
  - a slot drawn while it is set hidden;
  - a visible slot with a root that goes undrawn for 60 passes of its viewport (counted by the end-pass callback,
    for the viewports that had a pass);
  - a slot drawn in a viewport other than the one set.

### Runtime (`packages/native-runtime`)

- `index.js`: `setDispatchHook(hook)` wraps every `dispatch`/`dispatch_sync`; `targetNode(h)` creates a mount
  target whose handle can change.
- `embed.js`: a handler on a root's node runs inside that root. `root()` works in handlers, and
  `requestAnimationFrame` from a handler goes to the root's queue, so a hidden root's handler-started loops stop.
  The root is found through the node's native parents up to its mount target. `__svelters_embed.retarget(id,
  { target })` supports `move_root`.
- svelters-host: `Host::retarget_root(root, target)`.

### Runners and test helpers

- **The SG1 gap is fixed:**
  - `TestApp` attaches the embed through kittest's `with_context_setup`, and `run_desktop` attaches it in eframe's
    creation callback. The pass callbacks are therefore registered before the first frame.
  - `Embed::new` attaches to its own context.
- `harness::Viewports` (`runner/viewports.rs`): a headless multi-viewport runner with per-viewport pass counts.
  - It does what eframe does. The root ui is the test's. Deferred viewports run their own passes when egui asks
    for them: `repaint_delay == 0` in a pass's output, or the repaint callback between passes.
  - Input goes to a single viewport, one event per pass.
  - `SVELTERS_VIEWPORTS_LOG=1` prints each pass with egui's repaint causes.
- `harness::dock` (`runner/dock.rs`): the §7.1 adapter shape (`SvelteDock`, `PaneEvent`, `CloseReason`), with the
  seam items as decided:
  - lazy mount on the first `Shown`, from a pending table;
  - `request_close` → `CloseDecided` → the host's `DockCommand::Close` → `Closed`;
  - a wildcard arm for berth's non-exhaustive enum;
  - `pane_ui` and `viewport_done` through `with_current`.

### Tests

`crates/svelters-egui/tests/viewports.rs` (fixtures `Pane`, `Editor`, `Animator` added to `fixtures/embed-egui`):

| Acceptance | Test |
|---|---|
| DOCK-W2: 3 viewports (2 deferred). A click in window 2 runs only window 2's passes (root 0, window 3 0). A props change made from Rust repaints window 3, not window 2. Every window is then idle: nothing due, no delayed repaint, settled, 20 steps run 0 passes. | `dock_w2_a_change_in_window_2_repaints_only_window_2` |
| The same with `prepass_plugin = false`: one root frame (egui's 1–2 passes per request), window 3 at 0 | `dock_w2_without_the_prepass_plugin_input_costs_one_root_frame` |
| DOCK-W4: a text field focused and typed into in the root viewport, then moved to window 2 and on to window 3. Caret, focus (restored in the new viewport, typing continues there) and egui id are kept. Mounts and the JS instance counter (`init` emits) are unchanged. `root().viewport` and `root().focused` reach JS. | `dock_w4_a_root_moved_between_viewports_keeps_caret_focus_and_instances` |
| A hidden root's rAF callbacks (a loop started from an event handler) do not run; it settles while hidden and resumes when shown | `a_hidden_roots_animation_frames_do_not_run` |
| The §7.1 adapter: lazy mount, close flow | `dock_close_flow_and_lazy_mount` |
| `move_root`: no remount, local state and ids kept, new nodes go to the new slot, the old slot is reusable, an occupied target is refused | `move_root_moves_a_root_into_another_slot_without_a_remount` |
| Debug checks | `debug_checks_flag_hidden_draws_undrawn_slots_and_wrong_viewports` |

JS: `test/embed.test.mjs` adds two tests: a handler runs in its root (`root()`, with its rAF held while hidden),
and `retarget`.

## Deviations, and why

- **No "moved" `EmbedEvent`** for `move_root`. The host initiated the move, so it knows; JS sees the new
  `viewport`/size through `root()`. Adding a variant would break exhaustive matches on `EmbedEvent`.
- **Scroll.** The egui backend has no scroll container yet. Scroll offsets are egui memory keyed by the node id,
  like the caret (`TextEditState`), which DOCK-W4 checks. They survive for the same reason.
- **R20** (`hit_test` skips other viewports' roots) is not implemented: `scene/hit.rs` is empty, and SG3 writes
  it. What SG3 needs: the slot's viewport is the backend's `root_vp[RootIx]` (`EguiBackend::viewport_of`), which
  is up to date from the `set_viewport` call on.
- **R21:** `show_viewport_passes` is the call site only. `ScenePass` is SG4's (`provider/pass.rs`).
- **`with_current`** resolves the embed made current with `make_current`, or else the oldest live embed on the
  thread. The design did not say which.
- Not done from EM1's "moved to EM2" list: `restart_runtime`, `SlotSizing::FitHeight`, the mounting spinner, the
  duplicate-anchor diagnostic, per-entry `warn` budgets.
- `prepass_plugin = true` stays the default (as designed); cadatomic sets `false`.

## Notes for cadatomic's berth adapter

- `harness::dock::SvelteDock` is the shape to copy. Feed it every `PaneEvent` in `App::logic` before
  `embed.logic(ctx)`:
  - `Shown { viewport }` → `set_viewport` + `set_visible(true)` (+ the lazy mount);
  - `Hidden` → `set_visible(false)`;
  - `MovedToViewport { to }` → `set_viewport(slot, to)`;
  - `Closed { reason }` (any reason) → `destroy_slot`;
  - `_` → ignore.
- berth's draw callback is `SvelteDock::pane_ui(ui, pane_id)` (`with_current` plus `show_slot`). It needs only
  the pane id, so it works inside berth's deferred-viewport closures. Call `embed.show_viewport_passes(ui)` at the
  end of each viewport's ui.
- With `prepass_plugin = false`:
  - a click in a deferred window costs 1–2 root passes (egui repaints twice per request) and the window's own;
  - a change that only touches window 3 repaints only window 3 (and the root, if logic ran there);
  - an idle app runs 0 frames in every window.
- **Call `set_viewport` before (or in the same logic as) the first draw in the new window.** Otherwise the slot
  is drawn in a viewport other than the one set, which gives `SlotWarning::WrongViewport`, and its repaints go to
  the old window until then.
- Focus moves with the pane: berth does not need to re-focus anything.

## Verification (this lane, local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets` and `cargo check --workspace --all-targets`:
  clean (existing warnings only).
- `cargo nextest run -p svelters-egui --test viewports`: 7/7. Then `cargo nextest run -p svelters-egui`: 45/45,
  all egui fixtures built with `pnpm --filter "./fixtures/*-egui" build` after `cargo build -p svelters-compiler
  --bin svelters-compile`. After the R21 commit (`show_viewport_passes`, `viewport_done`) `--test viewports` ran again: 7/7.
- `pnpm --filter @svelters/native-runtime test`: 107/107. Its `check` step ran separately, because `pnpm` is
  only available through corepack here: 0 errors.
- Not run locally: workspace tests, svelters-host tests, `egui-kinds-example`, conformance, the other backends,
  and the eframe runner on a display. CI runs them.
