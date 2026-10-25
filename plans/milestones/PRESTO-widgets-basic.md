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

## Requests for presto-core

All landed and used by `host.rs`: CORE (computed style, timers, AccessKit actions, blur, services, event
time), CORE-2 (h: child views, `folds_children`), CORE-3 at ci/presto-core-3 eb669f1e, merged into this
branch (a: part styles; b: listened signals; c: `emit_sync`, used by hyperlink `clicked` and label
`link-clicked` before `open-url`; d: peer groups; e: chrome padding, `places_children` + `child_layout`,
`children_inert`; f: `prop_state`/`prop_states`; g: scale; i: scope default/cancel activation, `:default`,
`Document::set_auto_default` (call `host::install`); j: `focus_anchor`, `for` → `labelled_by`).

Still open:
- (done, on CORE-3's sync delivery at ci/presto-base b387ef42) `changed` of checkbox, radio-button, switch,
  toggle-button, frame and slider is cancelable: emitted through `emit_sync` before the write, which a
  prevented `changed` skips (a sibling's follow-up `changed` in a radio or toggle group stays a plain signal).
- (done) Light dismiss: CORE-3's `Kind::light_dismiss`/`outside_press` (ci/presto-core-3 342cf6be) map to the
  widget's `light_dismiss` and `Event::OutsidePress`, which `Dismissable` reads.
- Run children: computed colour, weight and italic fold (done); per-run size and background do not (the
  records have no field).

## Questions for the coordinator (with recommendations)

1. Space/Enter on a focused slider or sash gives egui's fake click (a zero-length scrub). Recommend: drop the
   fake click for kinds whose keys mean something else (a `Sense` flag).
2. Roving skips disabled members? Recommend yes (Qt, GTK), in core's peer-group target choice.
3. The egui policy row has `focus_on_click: false`; production picks the OS row (`host::os_policy`).
   Recommend tests keep the egui row (kittest parity) and apps the OS row, as now.
4. Box keeps egui's column default through its prop rule rather than a UA sheet rule. Recommend keep.
5. Make the shim's kind interface presto-core's own (typed kind-side props, the peer and container hooks)
   or keep the bridge? Recommend keep the bridge until the consolidation, then decide with CORE.

## Kind traits beyond the spec (coordinator, 2026-10-25)

A widget declares its paint parts, whether it places its children and whether it folds them through the
shim `Kind`'s associated functions `parts()`, `places_children()` and `folds_children()` (beside
`prop_states()`); `host.rs` reads them per type, so view and tier-1 kinds need no edit there.
