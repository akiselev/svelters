# RMEGUI-S2: skelter bugs from cadatomic's rmegui wave 1

Lane S2 of the rmegui round. Branch `ci/rmegui-svelters-2` from master `a59acb27`. Brief:
`plans/cloud/rmegui/LANE-S2.md`. Coordinator: the cadatomic coordinator (`session_01Kt5cvt9hDGVcXNCATLsKRK`).

## What landed

| Item | Commit | What |
|---|---|---|
| 1 [FQ-65] | `90061e54` | **Sync-called value signals keep their arguments.** skelter calls a cancelable widget signal (checkbox, switch, slider, radio, toggle-button, frame `changed`) through `call_sync` so a listener can cancel it, and the runtime gave its handlers `(event, ...args)`. New ABI delivery flag `SVL_SIGNAL_ARGS` (8, in the signal id's flag bits, `call_sync` only). svelters-skelter's event-time delivery (`sync.rs`) sets it for a signal its surface does not declare `sync`. The runtime then calls handlers with `(...args, event)` on both paths (sync and a late delivery); the trailing `NativeEvent`'s `preventDefault()` cancels, and return values are ignored. The flag selects no handler set. Real sync signals (`close-request`, `commit`, DOM-style events, row lifecycle) keep `(event, ...args)`. Documented in `runtime-contract.md`, `backend-abi.md` and the Svelte support guide. |
| 2 [FQ-66] | `a161ae5b` | **`<table>` rows and cells.** The table's a11y node gets virtual children. A header `Row` holds the `ColumnHeader`s (title, column index, sort direction; `Click` = `header-clicked` and the sort). Each realised `Row` has its row index, `selected`, a label joined from its cells' text, and `Click`. Each row holds one `Cell` per column in view (text, row and column index, `selected`; `Click` selects the row as a pointer click does). The table requests an a11y update on a selection, cursor, sort, data, prop or scroll change (not on hover). skelter-core: `A11yCx::add_virtual_child_of` (a virtual child under an earlier one) and `KindSpec::conceals_children` (the `table-column` data children stay out of the a11y tree). |
| 3 [SV59] | `edcd6ce9`, `75a6f580` | **ListView rows named and anchorable.** `<ListView label={(item, i) => …}>`: the labels ride `splice`, and a rename (same key) sends them all with the new virtual-list method `setLabels(labels)` (skelter-widgets, last in `LIST_METHODS`; svelters-skelter surface, `metadata/semantics/skelter.json` regenerated). They are the options' accessible names and the type-ahead text. A backend without `setLabels` keeps its splice labels. `<ListView rowAnchor={(item, i) => …}>` writes the row outlet's `anchor` (the node AT sees as the option) at bind and clears it while the slot is unbound (`edcd6ce9` named it `anchor`, which broke `<ListView anchor="…">`, the list's own anchor; `75a6f580` renamed it). Types in `@svelters/lists`. |
| 4 [SV60] | `6cce8a44` | **`aria-activedescendant` / `aria-controls` re-resolve.** skelter-core keeps, per referring node, the targets its references resolved to when it last went out. Each a11y update re-checks them against the tree it sends and sends the node again when they differ: a target appeared, left or moved. |
| 5 | `cdfe9ff5` | **A label that takes clicks is hit on its whole box.** A hosted widget whose own hit test misses (a label's text) takes the point on its whole box when JS listens to the node for a click or press (`click`, `dblclick`, `pointerdown`, `pointerup`, `contextmenu`, or the kind's own `clicked`), as a button does; a `clickable` or `for` label does so itself. A label nobody listens to still lets clicks beyond its text through to its parent. skelter-core: `NodeView::listeners`. |

### Requests added by the coordinator (cadatomic panels-ui lane)

| Item | Commit | What |
|---|---|---|
| FQ-73 | `d4597cf1` | **`PrepareCx::set_native(prop, v)`**: a kind writes native state while preparing (a bound value it settles at mount). Checked at the call (the kind's or a common prop, the right type), applied once the prepare returns as `EventCx::set_native`'s (journalled native for `bind:`, a repaint next frame). |
| FQ-74 | `14e37405` | **Record and `any` props take plain objects.** `describe` gave a record prop `SVL_RECORD` with no type descriptor (the host refused every object: "no descriptor for type 0") and an `any` prop an untyped slot that refuses objects. Both are variant slots now (`value::slot_of`): a plain object arrives as a record with its fields, nested records and lists included. |
| FQ-75 | `4a955acd` | **App kind methods: argument descriptors and coded errors.** svelters-skelter describes an app kind's methods from its core `MethodSpec` (`describe_method`): records, lists and enums as variants, nullable arguments optional, integers integral, the return typed; `call` types an app kind's arguments the same way (lists were string lists). Built-in kinds' methods stay undescribed. skelter-core `PropError::Coded { code, message }` (`PropError::coded`): a kind method refusing with it is a JS `NativeError` with `.code` and `.message` (domain: the kind's tag). |
| non_exhaustive | `6abc4d9a` | skelter-core `PropError` is `#[non_exhaustive]` (coordinator, 2026-10-26). |
| FQ-67 | `583325f6` | **A re-enabled widget is enabled for AccessKit.** A hosted widget's `disabled` mirror was synced in `prop_changed` from the node's state, which core updates after that hook: a button re-enabled by `set_prop(DISABLED, false)` stayed disabled (and inertness never reached the mirror). `Hosted` syncs the mirror in `prepare`, and its a11y takes `disabled` from the node's state for every widget whose `disabled` is that mirror (a widget disabled by a prop of its own keeps what it said). |
| select anchors | `75ecaf3c` | **`<combo-box option-anchors>`**: skelter's `select` already published `option-anchors` as the options' author ids; the Svelte surface now exposes it (a string list parallel to `options`; `skelter.json` regenerated), so tours find an option with `anchor=` and click it. |
| FQ-76 | `b725f2d7` | **JS traffic counters**: `SkelterBackend::traffic()` / `take_traffic()` give a `Traffic` (native → JS: syncs, posts, prop changes, checkpoints, animation frames; JS → native: creates, tree edits, prop writes and reads, method calls, listener changes, style ops; `to_js()` / `from_js()` totals), as svelters-egui's `Stats`. |

## Tests

- svelters-skelter `tests/deliver.rs` `a_cancelable_widget_signal_is_called_with_its_arguments_first`, `tests/run.rs`
  `a_checkbox_click_calls_changed_with_its_value_and_can_be_cancelled` (a harness click on a checkbox; a `true` answer
  keeps it unchecked).
- native-runtime `runtime.test.mjs` "FQ-65: …" (both deliveries, `preventDefault`, an ignored `true`);
  `lists.test.mjs` "SV59: …" (labels with splice, `setLabels` on a rename, the outlet's anchor at bind/unbind).
- skelter-test `tests/rmegui_s2.rs` (consumer, AT-SPI on Linux): `a_table_exposes_rows_and_cells`,
  `set_labels_renames_the_realised_options`, `references_resolve_when_their_target_appears` (checked to fail without
  the fix), `a_stretched_label_with_a_click_listener_takes_clicks_on_its_whole_box`.
- FQ-73: skelter-core `tests::emits::a_kind_writes_native_state_from_prepare`. FQ-74/75: svelters-skelter
  `tests/app_values.rs` (compiled Svelte through the real svelters-host on an app kind:
  `a_record_prop_reaches_the_kind_with_its_shape`, which fails without the change, and
  `methods_take_records_and_raise_coded_errors`). FQ-76: svelters-skelter `tests/run.rs`
  `the_backend_counts_its_js_traffic`. FQ-67: skelter-test `rmegui_s2` `a_reenabled_widget_is_enabled_for_accesskit`
  (fails without the change). Select anchors: svelters-skelter `run` `combo_box_options_take_anchors`, skelter-test
  `rmegui_s2` `select_options_resolve_by_anchor` (svelters-automation's `anchor=` selector).
- skelter-core `tests::a11y::nested_virtual_children_hang_under_their_parent`; skelter-widgets
  `controls::label::tests::a_clickable_label_is_hit_on_its_whole_box`.

Run locally, filtered, all passed: the tests above; svelters-skelter `--test app_values --test app_kinds
--test host_document --test tier1 --test vlist`; svelters-skelter `--test deliver --test run --test spec` (spec
with `SVELTERS_UPDATE=1`, then without); skelter-test `--test qa11y_fixes --test focus_a11y`; skelter-core `--lib
tests::a11y`; skelter-widgets `--lib table controls::label`; native-runtime `lists.test.mjs`. `cargo check
--all-targets` on skelter-core, skelter-widgets, skelter-test, svelters-skelter, skelter. **Not run locally:** the
full suites, the rest of the workspace, clippy, conformance (no compiler change). CI is the gate (below).

## What cadatomic must change

1. **Checkbox/switch/slider handlers** (FQ-65): nothing; `onchanged={(v) => setVisible(v)}` now gets the value. To
   veto a toggle: `onchanged={(v, e) => e.preventDefault()}`.
2. **Tables** (FQ-66): nothing. Probes find rows as `Row` under the `Table` (the first `Row` is the header) and cells
   as `Cell`; AccessKit `Click` on a cell selects its row.
3. **Palette** (SV59): `<ListView label={(r) => r.label} rowAnchor={(r) => 'palette-row:' + r.key}>` (drop the
   `accessible-name` and `anchor` set inside the row content), and `aria-activedescendant={'palette-row:' + key}`
   on the field names the option itself.
4. **activedescendant** (SV60): nothing; the reference resolves when the row is realised.
5. **Clickable labels**: nothing; a stretched `<label onclick>` takes clicks on its whole box.
6. **Own kinds** building `skelter_core::KindSpec` without `..KindSpec::new(..)` need the new `conceals_children`
   field; an exhaustive `match` on `skelter_core::PropError` needs a `Coded` arm.
7. **Combo boxes** (tours): `<combo-box options={names} option-anchors={parts.map((p) => 'ecad.sch.tool.part:' + p.lib + ':' + p.name)}>`.
8. **Code editor** (FQ-73..76): `cx.set_native` in `prepare`; record props and record/list method arguments work
   as plain objects; refuse with `PropError::coded("E_…", "…")` to throw a JS error with `.code`; host tests read
   `backend.take_traffic()` around an interaction and assert `to_js() == 0`.

## Open questions (with a recommendation each)

- **Row names from content (QA11Y-fixes' open question)** stays open: `label` covers ListView rows; a `row_a11y`
  fold of descendant labels is still not done. Recommendation: unchanged, only if more apps need it.
- **svelters-egui's virtual-list** has no `setLabels` (maintenance only): a rename there keeps the old type-ahead
  text until the row is spliced again. Recommendation: leave it.
- **Header row index:** data rows have `row_index` = position and the header row none, with `row_count` = data
  rows. Recommendation: keep it until an AT shows a need to count the header row.

## CI

`cdfe9ff5`: the skelter workflow (run 40377622751) failed on all three OSes in one test, svelters-skelter
`host_document` `a_palette_list_places_its_rows_in_its_own_box` (the `anchor` clash above, fixed in `75a6f580`).
Final runs on `75ecaf3c` (the last code commit): pending.
