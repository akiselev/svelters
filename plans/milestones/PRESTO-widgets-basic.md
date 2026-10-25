# PRESTO-widgets-basic: presto-widgets interaction substrate, paint helpers, theme, Tier 0 kinds

Lane WIDGETS-BASIC of presto batch 2 (`plans/cloud/presto-b2/LANE-widgets-basic.md`), session
`session_012eauTkCP7to4arcKCPJvRq`, branch `ci/presto-widgets-basic`. Date: 2026-10-25.
The substrate (shim, interact, paint, theme, controls helpers, button) was merged into `ci/presto-base`
at ac7cc8c4 by the coordinator; the rest of this record is on the branch only.

## What landed (`crates/presto-widgets/src/`)

| Module | Contents | Source (svelters-egui, copied) |
|---|---|---|
| `shim.rs` | the interface the kinds are written against: `Kind` (typed props kept by the kind), `KindSpec`/`PropSpec`/`SignalSpec`, `Value`, the contexts, the per-node `Event`; `text::{RichText, TextLayout, TextShaper}`; `testing::{MonoShaper, MemResources}` (test doubles) | — |
| `host.rs` | the bridge to presto-core: `Hosted<W>` implements `presto_core::Kind` for every kind; `register::<W>`, `register_tier0`; `TextShaper` for `presto_text::TextSystem`; services read: `presto_text::TextSystem`, `interact::Policy`, `theme::Theme` | — |
| `interact/` | `Tracker` (egui 0.37's click/drag/double-click/Space-Enter/AccessKit rules as a per-node state machine), `StateBits` (svelters' bit numbers = `svelters_style::StateMask`), `Policy` rows (GTK, Qt, Windows, macOS, egui), `Unit` trait; units Hoverable, Pressable, Focusable, Toggleable, Draggable, DragToValue, Steppable, Resizable, Expandable, Roving, RovingList, Dismissable, Typeahead, Anchored, Tooltipable | `interact/`, `native/mod.rs`, `provider/state.rs` |
| `paint/` | `StyleView` over the engine's `ComputedStyle` with per-kind UA tables; box/outline (quads, shadows, linear gradients, per-side borders), text (layout, measure, elide, runs), paint parts (P3), image (object-fit, crop), icon (+ `icon/registry.rs`: ICR1 registry, masters, slots, resvg rasterising into A8/RGBA images, icon fonts) | `paint/`, `icons.rs` |
| `theme/` | the DS1 token map with presto `Color` and CSS-semantics shadows; `base.css`, `data/*.json` | `theme/mod.rs`, data (not `egui_style.rs`) |
| `controls/` | `kind_props!`, `common`, `testkit`; kinds: button, toggle-button, checkbox, radio-button, switch, slider (+ rail, numfmt), label (+ rich runs, and the run child kinds span/b/i/code/a it folds: `controls/run.rs`, SVELTERS-PRESTO Q7), hyperlink, separator, progress-bar, spinner, icon, image, box (`FlexBox`), frame, stack, sides, grid | `widgets/`, `elements/`, `layout.rs`, `kinds/` |

Every kind has spec, record (display-list golden) and interaction tests; units carry their ported tests.

## Not run

Only `cargo check`/`cargo clippy -p presto-widgets --tests` (clean). One filtered run of the tracker and
button tests while building the substrate (19/20 passed; the one failure was a wrong expectation, fixed).
Everything else is written, not run: CI runs it.

## Left out

- `Scrollable` (lane WIDGETS-VIEWS), `DragSource`/`DropTarget` (WIDGETS-TIER1), the slider's editable value
  box and label selection (TEXT-INPUT).
- Animations: the switch knob slide, the busy progress sweep, the spinner rotation (static frames; MOTION-CSS).
- Icon sets from svelters_modules `Assets` (`register_set`); image sources other than PNG/SVG files and
  registered bytes; async loading (files load synchronously on first use, as in svelters-egui).
- Label: elided-label tooltip, per-link focus and a11y children, run backgrounds, CSS-styled run
  child nodes. `focus-scope` and the common widget props (presto-core's). Locale number formatting.
- Theme shadows follow CSS (blur radius as written); svelters-egui halved it for egui's blur.

## Requests for presto-core (sent to the coordinator; CORE-2 landed the first six at cb47b49d and host.rs uses them)

Landed: computed style per node, kind timers, AccessKit actions, blur, services, event time.
Open (all additive):
1. Computed style of paint parts (`::part(name)`) in the contexts.
2. Whether JS listens to a signal (icon/image take clicks only while `clicked` is listened; shim `Kind::listen`).
3. Cancelable default actions: emit reports `prevented` (checkbox/radio/frame/slider `changed`, hyperlink/label `open-url`).
4. Peer groups: radio exclusivity, toggle-button groups, roving focus, one Tab stop per group (shim `PeerGroup`,
   `Kind::peer`; a reference implementation is the test-only `interact::roving::group::Group`).
5. Container hooks: chrome padding (frame caption), per-child layout and visibility (stack), inert children.
6. Prop-derived state bits after a prop write (`:checked`, `:busy`, `:indeterminate` without an event).
7. `scale` in `PrepareCx`.
8. Light-dismiss presses routed to open floating surfaces (`Dismissable`).
9. (h) A kind sees its children while measuring and recording (`NodeView::child`/`cx.view`), a child's change
   dirties a folding parent's measure and paint, and the folding parent's children are skipped by layout,
   recording and hit testing (and the parent is still measured as a leaf: taffy only measures nodes
   without children). Until then the bridge folds a label's run children only in `event`.
10. (i) Scope activation: Enter (Escape) in a focus scope, unclaimed by the focused kind, sends the scope's
    `default` (`cancel`) button an AccessKit `Click`; the auto-default rule (`Policy::auto_default`) moves
    `:default` to the focused push button in a dialog scope.
11. (j) `label for`: resolve the shim's `EventCx::focus_anchor` (focus the node with that anchor) and set
    `labelled_by` on the target's AccessKit node. The bridge drops `focus_anchor` until then.

Parity items from SVELTERS-PRESTO (coordinator, 2026-10-25), done on this branch: button `default`/`cancel`
(props, `:default`), label `selectable`/`selection-start`/`selection-end` (drag, double/triple click, Ctrl+A,
Ctrl+C → `copy` signal, Shift+arrows; geometry from presto-text's `hit_test`/`selection_rects` through the
shim's `TextGeometry`) and `for`, slider `submit` (drag release or Enter), label run children.

Until 1–6 land, the bridge ignores the shim's peer requests, `listen` and the container hooks: radio groups
are not exclusive inside a real document, icon/image clicks need (2), and stack shows every child.

## Questions for the coordinator (with recommendations)

1. Space/Enter on a focused slider or sash gives egui's fake click (a zero-length scrub). Recommend: drop the
   fake click for kinds whose keys mean something else (a `Sense` flag).
2. Roving skips disabled members? Recommend yes (Qt, GTK), in core's peer-group target choice.
3. The egui policy row has `focus_on_click: false`; production picks the OS row (`host::os_policy`).
   Recommend tests keep the egui row (kittest parity) and apps the OS row, as now.
4. Box keeps egui's column default through its prop rule rather than a UA sheet rule. Recommend keep.
5. Make the shim's kind interface presto-core's own (typed kind-side props, the peer and container hooks)
   or keep the bridge? Recommend keep the bridge until the consolidation, then decide with CORE.
