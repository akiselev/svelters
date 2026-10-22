# SG4: provider passes (`crates/svelters-egui`)

Design: `plans/research/ARCH-SCENE.md` §4.4, §7.2, §7.3, §7.5, §8.2 and the §14 SG4 row, with the cadatomic review
items R16 (provider half), R21 and R23 from its "Decided" section; `plans/research/ARCH-EMBED.md` §6.2, §8.2–§8.5.
Starts from master after SG3 (`cb22de7b`) and EV2b (`747392a3`, merged in, with master's `edefc916` fixes).
Lane: SG4 (SCENE writer), branch `ci/sg4`.

## What landed

| Where | Content |
|---|---|
| `provider/mod.rs` | **Payload take-out** (`with_taken`): while a provider's own code runs (`ui`, `layout`, `prepare`) its payload is out of the scene (`Payload::Vacant`, which reads as an empty box through `Scene::w`). `WidgetCx` now holds the scene: `scene()`, `node()`, `child_ids()`, `children(ui)`, `child(ui, id)`, besides RA1's `id`/`rect`/`state`/`set`/`emit`/`ext`/`ext_mut`/`request_measure` (unchanged signatures). A panic drops the widget, a fresh one (props at their defaults) takes its place, an open batch is closed, and the panic goes on to `show_slot`'s `catch_unwind`, which faults that root. New defaulted `Widget::layout` (`Caps::CUSTOM_LAYOUT`) and `Widget::prepare` (`Caps::PREPARE`). |
| `provider/pass.rs` | `StateProvider` (+ `ProviderCx`, `ProviderHandle<P>`: `with`, `with_mut`, `is_kicked`, `unregister`), `ScenePass` (+ `PassScope`, `PassCx`, `PassHandle<P>`), `PrepareCx`, and the runners: state providers (logic step 6), root passes (step e), viewport passes (R21), `prepare_root` (step b), busy probes. |
| `provider/behavior.rs` (EV2b's file, small) | `Behavior::before_ui` and `Behavior::paint_over` (defaulted), run by `around` through one shared hook runner (`hooks`) next to `after_ui`. `BehaviorBox` gets `layout`/`prepare` (never called). |
| `layout.rs` | Provider kinds with children (shadow children) are containers: flexbox from their style, or `Widget::layout` through `LayoutCx` (`children`, `measure`, `compute_child`, `place`, `default_layout`, `scene`). **Hoisting**: `compute_oof_layout` after each container's `PerformLayout`, hoisted lists in `NodeCold::hoisted`, `is_out_of_flow` from the style; `place` and `round_from` place an absolute box relative to its containing block (`positional_parent`); a boundary relayout whose subtree has an absolute box with a containing block above it falls back to a whole-root layout. `style_of` maps `position` and the insets; a provider with `Caps::CONTAINER`/`CUSTOM_LAYOUT` takes the box props. `RootEntry::laid` records what was laid out for `ScenePass::after_layout`. |
| `kinds/builtin.rs`, `kinds/mod.rs`, `scene/props.rs`, `scene/keys.rs`, `scene/node.rs` | New common props on every kind: `position` (`static`, `relative`, `absolute`), `left`, `top`, `right`, `bottom` (`keys::widget::{POSITION, LEFT, TOP, RIGHT, BOTTOM}`). `metadata/semantics/egui.json` and `packages/native-runtime/types/egui.d.ts` regenerated. |
| `scene/tree.rs` | **Shadow children** (ARCH-SCENE §4.4): `Scene::insert_shadow`, stored in the host's `children` with `NodeFlags::SHADOW` on each shadow top; `remove`/`destroy` of a shadow top check only the child's owner; destroying a host frees its shadow subtrees. **Overlay roots**: `TreeRoot::Overlay { node, layer }`, `TreeRoot::slot_node()`. |
| `scene/access.rs`, `scene/mod.rs`, `scene/id.rs`, `scene/hit.rs`, `scene/dump.rs` | `SceneMut::insert_shadow`, `NodeRef::is_shadow`, the pass registry in `SceneCell`, `SceneHandle::{register_state_provider, register_pass, pass_counts}`, `Scene::abort_batch`, `Target::node()`, R20's viewport check also for overlay roots, `SceneStats::{provider_panics, provider_updates, custom_layouts}`. |
| `frame.rs` | Logic step 6: `run_state_providers` after the checkpoint and rAF, before the announcements; the visible roots are touched (repaint routing) when a provider changed the scene; a provider still wanting asks for another logic. |
| `embed/mod.rs`, `embed/current.rs`, `embed/settle.rs`, `runner/dock.rs` | `Embed::{register_state_provider, register_pass, create_overlay, show_overlay}`; `show_viewport_passes` runs the passes (returns how many); the thread-local `svelters_egui::embed::show_viewport_passes(ui)` for deferred closures; `SvelteDock::viewport_done` calls it (the body of berth's `PaneHost::viewport_done`). `Busy::PROVIDER` is set by a wanting, kicked or busy state provider and by `Widget::busy()` (R16, R8). Root passes run after a slot's walk. |
| `walk.rs` | `paint_child` (for `WidgetCx::children`), `prepare_root` before each root's layout, the walk origin passed to `provider::paint`. |

## API

```rust
use svelters_egui::prelude::*;   // + LayoutCx, PrepareCx, StateProvider, ProviderCx, ScenePass, PassScope, PassCx,
                                 //   Behavior, BehaviorCx, NodeId, NodeRef, NodeMut, SceneRef, SceneMut

// a provider container: builds shadow rows in prepare, lays them out itself, draws them
#[svelters::widget(namespace = "cad", tag = "stair-view", caps = CUSTOM_LAYOUT | PREPARE)]
impl Widget for StairView {
    fn prepare(&mut self, cx: &mut PrepareCx<'_>) {            // before layout, every drawn frame
        let host = cx.host();
        let mut m = cx.scene();                                // SceneMut, Origin::Rust(<kind id>)
        let l = m.create_tag("label")?; m.insert_shadow(host, l, None)?;
    }
    fn layout(&mut self, cx: &mut LayoutCx<'_>, inputs: taffy::LayoutInput) -> taffy::LayoutOutput {
        for &c in cx.children().to_vec().iter() { let s = cx.measure(c, avail); cx.place(c, at, s); }
        taffy::LayoutOutput::from_outer_size(size)            // or cx.default_layout(inputs)
    }
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let n = cx.scene().len();                              // the rest of the scene, read-only
        cx.children(ui);                                       // or cx.child(ui, id)
        ui.interact(cx.rect(), cx.id(), egui::Sense::hover())
    }
}
scene.write(app, |m| m.insert_shadow(host, child, None))?;   // from Rust, any provider host

// a state provider (R23, R16)
impl StateProvider for PickProvider {
    fn name(&self) -> &'static str { "cad.pick" }
    fn update(&mut self, scene: &mut SceneMut<'_>, cx: &ProviderCx<'_>) { /* one batch */ }
    fn wants(&self) -> bool { self.dirty }
    fn busy(&self) -> bool { false }
}
let pick = embed.register_state_provider(PickProvider::default());   // ProviderHandle<PickProvider>
pick.with_mut(|p| p.want = Some(ty));      // marks it wanting, asks for a logic; Busy::PROVIDER until it ran

// a scene pass (R21)
impl ScenePass for PickOverlay {
    fn scope(&self) -> PassScope { PassScope::AllViewports }      // or Root(r), Viewport(vp)
    fn order(&self) -> egui::Order { egui::Order::Foreground }
    fn overlay(&mut self, ui: &mut egui::Ui, scene: SceneRef<'_>, cx: &mut PassCx<'_>) { … }
    fn after_layout(&mut self, scene: SceneRef<'_>, damage: &Damage) { … }
}
let h = embed.register_pass(PickOverlay::default());               // PassHandle<PickOverlay>
embed.show_viewport_passes(ui);                  // end of each viewport's ui (root and deferred)
svelters_egui::embed::show_viewport_passes(ui);  // the same, from a deferred closure (berth's viewport_done)

// overlay roots (§7.5, §10.1)
let dims = embed.create_overlay("viewport.dims", egui::Order::Foreground, SlotOptions::fill());
embed.show_overlay(ctx, dims, viewport_rect);    // click-through except over nodes that take input
// absolutely positioned boxes: position="absolute" left/top (Svelte or keys::widget::{POSITION, LEFT, TOP})

// behaviours (EV2b + SG4)
impl Behavior for Pick {
    fn before_ui(&mut self, host: NodeRef<'_>, cx: &mut BehaviorCx<'_>) { … }
    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) { … }
    fn paint_over(&mut self, host: NodeRef<'_>, painter: &egui::Painter, cx: &mut BehaviorCx<'_>) { … }
}
```

## Acceptance (§14 SG4)

- **A provider container lays out shadow children:** `examples/egui-kinds/tests/passes.rs::a_provider_container_lays_out_shadow_children`.
  `stair-view` (`CUSTOM_LAYOUT | PREPARE`) builds its rows in `prepare`, places row k at (30k, 20k) in its own
  `layout` (size = extent), draws them with `cx.children`; rows are shadow tops, in the hit list; a count change
  rebinds (more rows; fewer, the extra ones destroyed). `shadow-column` (`CONTAINER`) gets flexbox (a column) for
  shadow children inserted from Rust. Also: the scene is readable from `ui`, the node's own payload is out while it
  draws; a built-in box hosts no shadow children; `remove` takes one out; destroying the host frees its rows.
- **A `StateProvider` toggles 5k bits in one batch with one repaint:**
  `crates/svelters-egui/tests/passes.rs::a_state_provider_toggles_5k_bits_in_one_batch_with_one_repaint`. 5000
  labels; `with_mut` marks the provider kicked and the embed `Busy::PROVIDER`; one `update`, one `Batch` record
  labelled with the provider's name, 5000 `State` records, one tick for all, 1–2 logic passes (one frame; egui may
  run its pass twice), settled after. Also `wants()` keeping it running one update per logic, and a panic in
  `update` disabling that provider only (the other one runs, later writes get their own ticks, the embed settles).
- **An overlay root over a canvas passes pointer input through:**
  `crates/svelters-egui/tests/passes.rs::an_overlay_root_over_a_canvas_passes_pointer_input_through`. A
  click-sensing canvas under an overlay of absolutely positioned labels (placed at their left/top) and a checkbox:
  clicks on a label and on the empty overlay reach the canvas; the scene's hit list still reports the label; a
  click on the checkbox toggles it and does not reach the canvas; moving a label with a batch of left/top writes
  moves only it.
- **A provider panic faults one root:** `examples/egui-kinds/tests/passes.rs::a_provider_panic_faults_one_root`.
  Two `Pane` roots (fixture bundle `fixtures/embed-egui`); a `boom-box` provider built from Rust next to root a's
  content panics in `ui`: root a is `Faulted` (event), root b is live and still drawn, the widget was replaced by a
  fresh one, the scene validates.
- Also: `absolute_boxes_are_placed_in_their_containing_block` (relative offset, `right`/`bottom` in a positioned
  parent, an absolute box in a static boundary contained by the root, text changes in both cases), and
  `scene_passes_run_per_root_and_per_viewport` (root and all-viewport passes, `after_layout` damage, the dock's
  `viewport_done`, unregistering).

## Deviations from ARCH-SCENE, and why

- **Shadow children live in the host's `children` list**, flagged `SHADOW`, not in a separate `cold.shadow` list.
  Provider kinds take no light children (no slot), so the list is theirs; layout, the root cache, validation,
  journaling and frees work unchanged. `NodeRef::children()` therefore lists a provider's shadow children
  (`is_shadow()` tells them apart). When RA3 gives providers light children too, the list splits.
- **`WidgetCx::ext_mut` is not buffered** (§7.3 says buffered and applied in step f): with the payload out the cx
  holds the scene, so the write goes straight in (journaled `Origin::Widget`), as in RA1/SG2.
- **`PrepareCx::scene()` is not restricted** to the host's shadow subtree (convention, documented). Its writes are
  one batch per `prepare`. R22 (this frame's scroll offset) is SG5's.
- **Overlay roots are embed slots** with a `TreeRoot::Overlay` scene root, drawn by `show_overlay(ctx, slot, rect)`
  (the layer is the root's). "Unless a node captures input": the area is interactable only while the pointer is
  over a node of the overlay that takes input (button, checkbox, text edit, slider, selectable label, a
  `FOCUSABLE`/`EDITABLE` provider), per last pass's hit list. An interactable area blocks the content below over
  its whole rect, so this is a per-frame switch (egui 0.37 skips non-interactable layers in its hit test).
- **A panicking `StateProvider` is disabled**, not faulted per root (it has no root). Reported once on stderr.
- **State provider repaints** touch every visible root when a provider changed anything (the change set is not
  mapped to roots); repaint routing still asks each viewport once and skips the one drawing.
- **`ScenePass::after_layout` runs at the root's next draw** (before its root passes), with `Damage::relayout` only.
- **`Widget::layout` exposes taffy types** (`svelters_egui::taffy` re-exported; unstable per §16).
- `NodeFlags::SHADOW` is bit 20 (EV2b took 15 and 16).

## Notes for the next lanes

- **SG5:** `PrepareCx` is the place for R22's scroll offset and `RowPool`; `with_taken` is how virtualisers get
  `&mut self` and the scene. `Descendants::with_shadow` can filter on `is_shadow()`.
- **RA3:** containers with light children (`Caps::CONTAINER` + a slot) need the light/shadow split noted above;
  `cached_shapes` can sit on `WidgetCx` (it has the scene now).
- **CS5e (landed first):** `position`/`left`/`top`/`right`/`bottom` are kind props (style table `via: setter`), and
  `style.rs::layout_style` also maps their computed values onto the taffy style (small SG4 addition in the STYLE
  file). In `layout.rs`, a container picks custom layout, then `display: grid` (CS5e), then flexbox, and runs the
  out-of-flow pass after each. Hoisting handles `absolute`; `fixed` is not mapped.
- **cadatomic:** berth's `PaneHost::viewport_done(&mut self, ui, window)` → `SvelteDock::viewport_done(ui)`.

## Verification

- `tools/dev/cargo-slot cargo check -p svelters-egui -p egui-kinds-example --all-targets`: clean after the last
  merge of origin/master (which brought Q1/LD1/TK1/CS0/Q3 changes to `embed/mod.rs` and `runner/kittest.rs`).
- Local nextest after that merge: the five `passes` tests that need no bundle (5/5). Earlier, before that merge:
  the svelters-egui lib tests (scene, journal, queries, random interleavings, the 11k-node boundary test) and
  `checked_in_kinds_json_and_typings_are_current`, all passing. `SVELTERS_UPDATE=1 … --test spec` regenerated
  `egui.json` and `egui.d.ts`, and `tools/metadata/style/gen_props.py` regenerated `props.generated.js`. The
  `--check` runs of gen_semantics, gen_observe_outputs, coverage, gen_props, gen_bindings and observe passed.
- CI on `dfcec15a`: linux-gtk 2335/2336. The one failure was `a_provider_panic_faults_one_root` reading the
  boom node after its root was disposed (disposal frees the slot's content, the Rust node too); the test now
  allows that. The fault isolation assertions before it passed. js failed only on the stale
  `props.generated.js`, now regenerated. macos-appkit and windows-winui passed.
- Not run locally: the rest of the svelters-egui and egui-kinds suites (they need the fixture bundles), the
  workspace, conformance, JS tests and the other backends.
