# EV2b: native behaviours behind `use:` (`#[svelters::behavior]`, SV2)

Design: `plans/research/ARCH-EMBED.md` §8.5 and the §10 EV2b row; `plans/research/ARCH-SCENE.md` §4.4 (attached
relation) and §7.3 (`Behavior` hooks), "Decided" R13–R24. Builds on RA1 (`plans/milestones/RA1-widget-kinds.md`),
SG1, SG2 (merged master `db7f62c5` before relying on ext data) and EM2.

## What landed

An app crate defines a behaviour in Rust, and Svelte attaches it to any element with `use:`, typed end to end:

```rust
// cadatomic-ui (any crate depending on svelters + svelters-egui)
use svelters_egui::prelude::*;

svelters_egui::state_bits! { pub mod cad { PICK_TARGET = "pick-target", PICK_HOVER = "pick-hover" } }

#[derive(svelters::Record, Clone, Debug, Default, PartialEq)]
pub struct PickRef { pub kind: String, pub label: String }

/// Makes its host pickable while a pick runs.
#[svelters::behavior(namespace = "cad", name = "pick")]
pub struct Pick {
    #[prop] accepts: Vec<String>,          // a param (required: no default, not Option)
    #[prop(default = true)] active: bool,  // an optional param
    #[event] picked: Event<PickRef>,       // the params' `onpicked` callback
    #[state] frames: u64,                  // Rust-only node state
}

impl Behavior for Pick {
    fn sense(&self) -> egui::Sense { if self.active { egui::Sense::click() } else { egui::Sense::hover() } }
    fn after_ui(&mut self, host: NodeRef<'_>, resp: &egui::Response, cx: &mut BehaviorCx<'_>) {
        cx.set_host_state(&cad::PICK_TARGET, self.active);
        cx.set_host_state(&cad::PICK_HOVER, self.active && resp.hovered());
        if self.active && resp.clicked() {
            cx.emit(&self.picked, PickRef { kind: self.accepts[0].clone(), label: host.text().into() });
        }
    }
    fn detached(&mut self, host: &mut NodeMut<'_>) {
        host.set_state(&cad::PICK_TARGET, false);
        host.set_state(&cad::PICK_HOVER, false);
    }
}
```

```svelte
<script lang="ts">
  import type { PickRef } from 'svelters:app/cad';
  import { pick } from 'svelters:behaviors/cad';
  let picking = $state(true);
</script>
<label use:pick={{ accepts: ['mcad.face'], active: picking, onpicked: (r: PickRef) => … }}>Face</label>
<box use:pick={{ accepts: ['mcad.body'] }}>…</box>
```

The app binary's `svelters_egui::dump(dir)` writes `behaviors/cad.js` (`export const pick =
nativeBehavior('cad.pick', ['picked'])`) and `behaviors.d.ts` (`declare module 'svelters:behaviors/cad'` with
`PickParams` and `pick: Action<any, PickParams>`); Vite resolves the import to the dumped module. The working
example is `examples/egui-kinds` (`Pick` in `src/lib.rs`, `app/src/App.svelte`, `tests/behaviors.rs`).

| Piece | Where |
|---|---|
| `Behavior` (`sense`, `after_ui`, `detached`), `BehaviorCx` (`node`, `host`, `id`, `rect`, `host_state`, `set_host_state`, `with_host(|n: &mut NodeMut| ..)`, `emit`, `request_repaint`), `BehaviorBox<B>` (the payload: an `ErasedWidget` over the behaviour, so props, `NodeRef::widget::<B>` and the registry treat it as any provider kind), `__new_behavior` | `crates/svelters-egui/src/provider/behavior.rs` |
| The walk's hook `behavior::around` (host senses merged; a container host gets its own response at its rect, registered before its children; then each `after_ui`; host writes applied after the hook with `Origin::Widget`, journaled; events flushed like a widget's) | `provider/behavior.rs`, called from `walk.rs::paint_host` (one branch at the top of `paint_node` on `NodeFlags::HAS_ATTACHED`; `paint_own` is the old body, now returning the leaf's response) |
| Attached lists: `Scene::{insert_attached, unlink_attached, orphan_attached, attached_of}`, `NodeRef::attached()` | `provider/behavior.rs` (`impl Scene`), called from `scene/tree.rs` |
| `KindEntry::attaches_to` (`None` for widgets, `Some("Widget")` for behaviours), `KindEntry::behavior_name`, `ErasedWidget::{behavior, behavior_ref}` (defaulted), `written_prop`/`queue_emit`/`flush_writes` shared by `WidgetCx` and `BehaviorCx`; `provider::paint` and `leaf::leaf` return the response | `provider/mod.rs`, `leaf.rs` |
| `KindRegistry::{attaches, behaviors, elements}`; a behaviour kind has no `Widget` base | `kinds/registry.rs` |
| `#[svelters::behavior(namespace, name)]` (same expansion as `#[svelters::widget]`, mode flag: tag `<ns>.<name>`, `attaches_to: Some("Widget")`, `new` = `BehaviorBox`; rejects `bind`, `measure`, `caps`, `tag`), re-exported as `svelters::behavior` | `crates/svelters-macros/src/lib.rs`, `crates/svelters/src/lib.rs` |
| Generator: `spec::behavior_modules()`, `spec::behaviors_typings()`, `dump` writes `behaviors.d.ts` + `behaviors/<ns>.js` when the binary has behaviours; the fragment lists behaviours under `behaviors` (kind JSON + `attaches_to`, `module`, `export`, `tag`) and keeps them out of `kinds`/`tags`/`caps`/`kinds.d.ts` | `crates/svelters-egui/src/spec.rs` |
| Runtime: `nativeBehavior(kind, events)` (the action), `EventTarget` stub for QuickJS (see Bugs) | `packages/native-runtime/src/index.js` |
| Vite: `svelters:behaviors/<ns>` → `<root>/.svelters/app/behaviors/<ns>.js` | `packages/native-runtime/vite.js` |
| Scene edits (small, for SG3's awareness): `NodeFlags::{ATTACHED, HAS_ATTACHED}` (bits 15, 16), `NodeCold::attached`; `validate` counts attached lists; `tree.rs` branches in `insert` (attached kinds first), `unlink` (attached nodes), `destroy` (`orphan_attached`), `free_subtree` and `set_root_subtree` (walk attached lists too); `may_edit` is `pub(crate)`; `NodeRef`'s `s`/`i` are `pub(crate)` | `scene/node.rs`, `scene/mod.rs`, `scene/tree.rs`, `scene/access.rs` |

## Acceptance (ARCH-EMBED §10)

- **`use:pick` from a provider runs with zero JS entries per frame, counted with `Timing`**:
  `examples/egui-kinds/tests/behaviors.rs::use_pick_runs_in_rust_with_no_js_per_frame_and_a_params_update_is_one_set_prop`
  (`HostConfig::timing`): 10 frames hovering the host give `Timing { checkpoints: 0, js_ns: 0, ops: 0 }`, the
  hook's frame counter grows by ≥ 10 and the host carries `cad.pick-hover`. A click delivers `onpicked` with the
  record. A container host (`<box use:pick>`) runs its hook and keeps drawing its children.
- **A params update is one `set_prop`**: same test, flipping `active` (`accepts` unchanged) costs `Timing.ops ==
  1`; the host's pick bits clear on the next frame. `packages/native-runtime/test/behaviors.test.mjs` checks the
  action against the TestHost: create, set params, listen, insert (no slot); a fresh but equal array and a new
  callback produce no op; a removed param resets to `null`; destroy destroys the node once.
- Also: `attached_nodes_are_not_children_and_detach_cleans_the_host` (Rust-owned nodes: a leaf takes a behaviour
  but no children, `validate` holds, `remove` runs `detached` and clears the host's bits, a behaviour does not take
  behaviours, destroying the host frees its behaviours); svelte-check of `App.svelte` against `behaviors.d.ts`
  (checked locally that `accepts: 3` fails); `checked_in_kinds_json_and_typings_are_current` now walks
  `behaviors/`.

## Decisions and deviations inside the spec

- **Events go through the params, not onto the host element.** ARCH-EMBED §8.5 sketches
  `<label use:pick={…} onpicked={…}>`; the compiler rejects `onpicked` on a `label` (no such signal), and
  accepting it needs the compiler to resolve the `use:` import to the behaviour's kind. With `onpicked` in the
  params there is no compiler change (as §8.5 wants), svelte-check types it (`PickParams`), and a new callback
  per update costs no host op. If cadatomic wants the element spelling, it is a compiler task (resolve
  `svelters:behaviors/<ns>` imports from the fragment's `behaviors`, lower `on<event>` onto the action).
- **Param names are the prop names** (kebab-case, as on elements: `'units-shown'`), no camelCase mapping.
- **No `bind:` on behaviour params** (`#[prop(bind)]` is rejected): a Svelte action has no binding path. Report
  values with an event.
- **Kind tag `<ns>.<name>`** (`cad.pick`): `create` needs a tag, and a dotted tag is a component in a template,
  so no template can write a behaviour as an element. The spec id stays `<ns>.<StructName>` (`cad.Pick`).
- **Hook signature.** `after_ui(&mut self, host: NodeRef, resp, cx)` as ARCH-SCENE §7.3. The hook reads the whole
  scene through `host`; writes are buffered in `cx` (host state bits as a copy, other host writes as
  `with_host` closures) and applied after the hook with `Origin::Widget`, so `NodeMut::set_state`'s journal path
  is used. `detached(&mut NodeMut)` is new (not in §7.3): a behaviour that marks its host needs a place to
  unmark it (the R19 concern for pick bits).
- **Sense merge.** A leaf host's response gets the union of its behaviours' `sense()` via `Response::interact`
  (egui merges senses for a re-registered id). A container host gets `ui.interact(rect, host egui id, sense)`
  before its children, so children stay on top in egui's hit test.
- **A host destroyed with `SceneMut::destroy`/`free_subtree` frees its behaviours**; the runtime's `destroy` of a
  single host (`Scene::destroy`) orphans them alive (the action's own destroy frees them), like light children.

## Bugs fixed on the way

- **`use:` with an object param never updated on QuickJS.** Svelte's `action` reads the param with
  `deep_read_state`, which tests `value instanceof EventTarget`; QuickJS has no `EventTarget`, the render
  effect threw on its first run, lost its dependencies, and `update` never ran (no error reached
  `TestApp::errors`). `index.js` now defines an empty `EventTarget` when there is none. This affected every
  `use:` action with a non-primitive param, not only behaviours. The effect error being silent is worth a
  look by whoever owns error reporting.

## Notes for SG3, SG4, RA3, EV1

- **SG3** (owns `scene/*`): attached nodes have `core.parent = host` but are not in `children[host]`; they are in
  `NodeCold::attached` with `NodeFlags::ATTACHED`, the host has `HAS_ATTACHED`. `Children`/`Descendants` skip
  them (use `NodeRef::attached`). Queries and hit lists should skip `ATTACHED` nodes (they have no rect). The
  journal records `Inserted { before: None }`/`Removed` for attach/detach.
- **SG4**: `before_ui` and `paint_over` go in `behavior::around` (before `paint`, and after with
  `ui.painter()`); `BehaviorCx` has a lifetime parameter for the scene references they may need. A `PickProvider`
  (`StateProvider`) can bulk-mark from `with_ext(&PRESENTS)`; a `use:presents` behaviour writes `PRESENTS` with
  `cx.with_host(|n| n.ext_mut(&PRESENTS).set(r))` and removes it in `detached`.
- **EV1**: `Behavior::filter` is not declared yet; add it as a defaulted method.
- **RA3**: `KindEntry` grew `attaches_to`; the macro shares one expansion for widgets and behaviours
  (`widget_impl(.., behavior: bool)`).
- **Busy**: `Behavior` has no `busy` (nor is `Widget::busy` collected yet).

## Verification (local)

- `tools/dev/cargo-slot cargo check -p svelters-egui -p egui-kinds-example --all-targets` and
  `cargo check --workspace --all-targets` after the macro/API change.
- `nextest -p egui-kinds-example`: 6/6 (2 new behaviour tests, RA1's 4 unchanged); `pnpm --filter
  ./examples/egui-kinds/app test` (vite build + svelte-check, 0 errors); `node --test test/behaviors.test.mjs` in
  `packages/native-runtime`.
- Not run locally: the full workspace nextest, conformance, the other backends, the full JS test run (CI).
