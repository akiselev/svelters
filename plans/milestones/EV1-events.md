# EV1: capture and bubbling on egui, Rust event filters (SV6), overlays (SV7), `cx.dispatch`

Track: EVENTS (egui). Lane EV1, branch `ci/ev1`, from master `1786957f`, merged with master `d242d83c`.
Answers SV9 (the propagation half; FW1 did the forwarding half), SV6, SV7, and the EV1 seam of
ARCH-WIDGETS (§1.2, §2.3, §7.3: `WidgetCx::dispatch` / `BehaviorCx::dispatch`, retargeting from
parts). Design: TRACKS EVENTS rows EV1/EV2; ARCH-SCENE §7.3-§7.5; CADATOMIC.md §2.1-§2.2; ABI
v2.2 listen phases (`backend-abi.md`). Module: `crates/svelters-egui/src/events.rs` (public,
`svelters_egui::events`; the prelude re-exports its types).

## What landed

| Piece | Where | What |
|---|---|---|
| DOM-style events | `kinds/mod.rs` (`dom_signals!`, `DOM_EVENTS`, `is_dom_event`), `kinds/builtin.rs` (`Widget`), `kinds/containers.rs` (`window`) | `pointerdown`, `pointerup`, `click`, `dblclick`, `contextmenu`, `wheel`, `keydown`, `keyup`, `focusin`, `focusout` on every widget and window. Sync signals with spec `phases: ["capture"]`. Arguments: `target` (handle number), `phase` (1 capture, 2 target, 3 bubble; +8 already prevented, +16 not cancelable), then per event (point, button, modifiers; wheel delta; key name, repeat; related node). |
| Capture listeners | `ops.rs` (`listen_ex`), `scene/node.rs` (`NodeCold::listen_capture`), `scene/mod.rs` | `listen_ex(.., SVL_LISTEN_CAPTURE)` on the DOM events only; any other phase or signal is refused with a reason. |
| Dispatch | `events.rs` (`EventsPlugin`, `EguiBackend::dispatch_input`, `dispatch`, `dispatch_js`) | An `egui::Plugin` input hook, once per context, for every backend bound to it (`install` from `bind_ctx` / `ui`): runs **before the pass** against the viewport's last hit list. Rust filters first (below), then the Svelte listeners through `call_sync` in DOM order: capture root → parent, the target's capture then plain listeners, plain parent → root. The answer is a bit set (1 stopPropagation, 2 preventDefault, 4 stopImmediatePropagation). preventDefault (or a filter's `Prevent`/`Consume`) takes the input out of `RawInput` before egui sees it (table in the module docs: a press and its release; a click by a move beyond the click distance before the release; the wheel event; the key and the text it types). Focus changes are noted at the end of each pass and dispatched at the next (not cancelable). Input of a viewport whose pass runs inside another's walk (legacy immediate viewports) is queued and dispatched at the next pass, not cancelable. |
| Targets | `scene/hit.rs` (`record_box`, `SceneRef::event_target`, `SceneRef::box_rect`), `walk.rs` (one line) | The hit list also records container boxes (before their children; `hit_test` still skips them, its results are unchanged). `event_target` gives the innermost node under the point: leaves and provider parts (`Hit::Part` → `Target::Virtual`), else the innermost box (a click on a box's padding targets the box); a provider recorded after its own shadow children is under them. Keys target the focused node, else the viewport's window. A click targets the innermost node holding both press and release. |
| Retargeting | `events.rs` (`SceneEvent::light_target`, `light_index`) | A target inside a shadow tree (a widget's node part, SG4 `insert_shadow`) is seen by Svelte as the host of the outermost shadow tree (DOM shadow-tree retargeting): the `target` argument and the target phase are the host's. Rust filters see both (`ev.target`, `ev.light_target`). |
| SV6 filters | `events.rs` (`EventFilter`, `FilterScope::{All, Root, Subtree}`, `Flow::{Pass, Stop, Prevent, Consume}`, `Outcome`, `FilterHandle`), `provider/behavior.rs` (`Behavior::filter`, `filter`, `HookRun`, `run_one`, `apply`) | `SceneHandle::register_filter` / `Embed::register_filter`; closures are filters. Order: `All` then `Root(r)` filters at the root, then per node `Subtree(node)` filters and the node's behaviours' `filter`, root → target (Capture, Target) and back up (Bubble), all before any Svelte listener; `EventFilter::after` gets the `Outcome` after bubbling. Behaviour hooks now run against a read-only scene and their writes are applied after (`HookRun`), so filters can run from the walk too. |
| `cx.dispatch` (ARCH-WIDGETS) | `provider/mod.rs` (`WidgetCx::dispatch`), `provider/behavior.rs` (`BehaviorCx::dispatch`), `events.rs` (`dispatch_signal`, `EventKind::Signal`) | **Stable signature** (OW0 builds on it): `pub fn dispatch<T: IntoJs>(&mut self, event: &Event<T>, payload: T) -> events::Outcome`. Runs the Rust filters (capture, target, bubble) for `EventKind::Signal(<event name>)` from the node **before** the default action; queues the JS emit unless a filter stopped it; the caller skips its default action when `outcome.prevented()`. JS listeners still get the event after the pass (no JS in the walk). |
| SV7 overlays | `events.rs` (`Overlay`, `OverlayCx`, `At::{Node, Target, Anchor, Path}`, `OverlayHandle`), `provider/pass.rs`, `windows.rs` | `SceneHandle::register_overlay` / `Embed::register_overlay`: an SG4 `ScenePass` over every viewport (or `Overlay::viewport`), on its own layer (`Overlay::order`). `OverlayCx::rect` resolves a node, a provider part, an anchor or an identity path `"<root key>/<anchor>"` to its last-pass screen rect (boxes from the hit list); `hovered()`, `request_repaint(_after)`. Viewport passes now also run after a presented window's walk, once per viewport and pass (a second call in the same pass, e.g. the host's `show_viewport_passes`, is a no-op). |
| Compiler | `lower/mod.rs`, `lower/native_attrs.rs`, `lower/forward.rs` (`capture_only`) | `on:event|capture={h}` on an element (and `svelte:element`) is the capture listener `oneventcapture={h}`, phase-checked like the attribute. Other modifiers stay `unsupported_feature` with FW1's fix-it. |
| Runtime | `packages/native-runtime/src/index.js` (`PROPAGATING`, `propagatingEvent`) | The event object of a propagating event gets `target` (the node of the `target` argument), `currentTarget`, `eventPhase`, `bubbles`, `cancelable`, `defaultPrevented`, `x`/`y`/`clientX`/`clientY`, `button`, `deltaX`/`deltaY`, `key`, `repeat`, `relatedTarget`, `shiftKey`/`ctrlKey`/`altKey`/`metaKey`; the answer is the bit set; `stopPropagation` works on non-cancelable propagating events. |
| Generated | `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` | The ten events per kind (`phases: ["capture"]`); typings `on<event>?` and `on<event>capture?: (e: SvlUiEvent) => void` with the new `SvlUiEvent` interface. |

## Tests

| What | Test |
|---|---|
| Capture, target, bubble order through `call_sync`; target and point arguments | `crates/svelters-egui/tests/events.rs` `ev1_click_runs_capture_target_bubble_in_dom_order` |
| stopPropagation (bubble, capture), stopImmediatePropagation at the target | `ev1_stop_propagation_ends_the_path_and_immediate_skips_the_rest_of_the_node` |
| preventDefault on `pointerdown` (no egui click) and on `click` (release without click); +8 phase bit | `ev1_prevent_default_keeps_the_input_from_egui` |
| A box's padding targets the box; a press and release on two nodes clicks their common ancestor | `ev1_a_click_on_a_box_targets_the_box_and_on_two_nodes_their_common_ancestor` |
| `wheel` (DOM direction; prevented: no scroll; else it scrolls), `contextmenu` order, `dblclick` | `ev1_pointer_wheel_contextmenu_and_dblclick_events` |
| Keys to the focused node (window without focus), capture on the window, prevented key and its text, `focusin` bubbling | `ev1_keys_go_to_the_focused_node_and_focus_events_bubble` |
| `listen_ex` refusals | `ev1_listen_ex_takes_capture_on_dom_events_only` |
| SV6: filters before Svelte, `Subtree` consumes (no listener, no egui click), `after` sees the outcome, unregister | `ev1_filters_run_before_svelte_and_can_consume` |
| SV7: overlay rects by node, anchor and box, `hovered`, unregister | `ev1_overlay_paints_after_the_walk_with_rects_by_node_and_anchor` |
| **Svelte from source**: a parent's `onclick` sees the child's click in bubble order, `onclickcapture` and `on:click|capture` first (`C1cTbB3!`), `stopPropagation`, `preventDefault` on `pointerdown` | `ev1_svelte_capture_and_bubble_from_source` (`fixtures/motion-egui/src/Propagation.svelte`, loaded from source only) |
| `WidgetCx::dispatch` vetoed by an ancestor's `Behavior::filter` (`Prevent`, capture and bubble), by a registered `Subtree` filter (`Consume`); a behaviour's filter consuming a press (egui never clicks) | `examples/egui-kinds/tests/events.rs` `a_widget_dispatch_runs_the_filters_before_its_default_action` |
| `BehaviorCx::dispatch`; a click on a node part retargets to the host | `a_behaviour_dispatch_and_parts_retarget_to_their_host` |
| Runtime event object and answer bits; deferred delivery not cancelable | `packages/native-runtime/test/ev1.test.mjs` |
| Compiler: `on:click|capture` on egui; the GTK phase error replaces the fix-it | `crates/svelters-compiler/tests/fw1/capture-egui`, `fw1/modifiers` |

## Decisions and deviations inside the design

- **Dispatch runs before the pass, in an egui input hook**, not in `logic`: egui decides clicks, drags
  and focus when its pass begins, so only the raw input can be withheld. The hook runs JS through
  `call_sync` (as `close-request` does), never during the walk. It hit-tests against the last pass,
  which is what the user saw.
- **Rust filters see the whole path (capture, target, bubble) before any Svelte listener** (SV6:
  "only Rust intercepts synchronously" first). A `Subtree` filter or behaviour is called twice for a
  descendant's event (Capture and Bubble); filters that act once check `ev.phase`. `All`/`Root`
  filters are called once, at the root.
- **`Flow` has four answers** (`Pass`, `Stop`, `Prevent`, `Consume`), `#[non_exhaustive]`, as are
  `SceneEvent`, `EventKind`, `EventPhase`, `FilterScope`, `Outcome`, `At` (room for SEC2's principal
  without a break; no principal field now, per the 2026-10-22 decision).
- **The JS answer of a propagating event is a bit set**, not `true`, so stopPropagation and
  preventDefault are told apart. The runtime knows the ten names (`PROPAGATING`); no other backend
  has signals with those names. `true` from an older runtime counts as stop + prevent.
- **`click` follows the DOM**: the innermost node holding press and release, whatever the movement;
  `contextmenu` fires on the secondary press (as on Linux and macOS), so preventing it keeps the
  press (and the context menu) from egui. `dblclick` uses egui's 0.3 s and click distance.
- **Focus events are not cancelable** (they happen in the pass); they are queued at the end of the
  pass and only when something would see them (a filter, a behaviour, a listener on the path), so
  idle stays at 0 frames.
- **`target` across shadow trees**: Svelte never listens inside a shadow tree (it has no handles
  there), so its `target` is always the light-tree host. `WidgetCx::dispatch` from a node part is
  filtered with the part as target (and `light_target` the host); its JS emit stays the part's own.
- **No `pointermove`/`pointerenter`/`pointerleave`** yet (hover is CSS `:hover` natively); add them
  to `dom_signals!` and the hook when a request needs them.

## Notes for the next lanes

- **OW0/OW3** (ARCH-WIDGETS): call `cx.dispatch(&self.event, payload)` for every user-visible intent
  and apply the default action only `if !outcome.prevented()`. A filter that wants to veto a widget's
  action matches `EventKind::Signal("<event name>")`. Outside-press for `Dismissable` can use
  `SceneRef::event_target` (boxes included) or an `All` filter on `pointerdown`.
- **Overlays per Area (ARCH-WIDGETS §3.2)**: `Overlay::order` picks the layer; overlays are
  click-through (SG4's pass layer).
- **SG3**: the hit list carries container entries; `hit_test` and `stats().hit_entries` exclude
  them.

## Verification (local, after the last code change and the merge with master `d242d83c`)

- `cargo check -p svelters-egui --tests`; `SVELTERS_UPDATE=1 nextest -p svelters-egui --test spec`
  (current after the merge); `UPDATE=1 nextest -p svelters-compiler --test fw1_forwarding` (3/3).
- nextest `-p svelters-egui -p egui-kinds-example --test events --test focus_keys --test passes
  --test queries --test widgets --test menus --test scene --test native_focus --test behaviors
  --test hit --test virt --test kinds --lib`: 116/120; the 4 others need built `dist/` bundles
  (`behaviors::use_pick_…`, `kinds::kittest_drives_the_field_by_label`,
  `passes::a_provider_panic_faults_one_root`, `queries::queries_and_hit_tests_…`): CI.
- `node --test test/ev1.test.mjs test/x3.test.mjs test/runtime.test.mjs test/behaviors.test.mjs`
  in `packages/native-runtime` (27/27).
- Not run locally: the full workspace, conformance, the other backends, the full JS suites (CI).
