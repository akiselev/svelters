# OV1: delivery-verified observation (plans/research/REACTIVITY.md §4.5, decision §13 #8)

Lane: the observe facts and their rules (`metadata/rules/<tk>.yaml` `observe:`), the GTK observe probe inputs and
generated observe rows (`tools/metadata/gtk/{observe_rules,gen_backend}.py`, `crates/svelters-gtk/src/testing_observe.rs`),
the Qt observe triggers, `tools/metadata/common/observe.py`, `tools/metadata/semantics/{gen_semantics,gen_observe_outputs}.py`,
the compiler's `plan_bind` and diagnostics, the R2 goldens. Branch `ci/ov1` (worktree branch
`worktree-agent-a8ec847313838ab49`). The ABI is unchanged.

## Problem

Decision §13 #8 lowers every `bind:` on an `exact` prop onto native observation (R2). That is safe only if the prop
really announces its native changes. R0 left 238 GTK `exact` facts whose generated row only installed the mechanism
(`connect`) and 58 with no row at all (the brief's "309 weak" counts the 71 `refused` constants too, which are not
`exact`); Qt had 14 `connect`-only and 45 row-less `exact` facts, WinUI 8 and 69, AppKit 1. If such a prop is silent,
`bind:` compiles and the bound value goes stale: R2's runtime fallback to the signal plan triggers only when the host
*refuses* the observation, never when it is silent.

## The rule

`exact` in §13 #8 means **`exact` and delivery-verified**:

| Fact on the platform | `bind:` lowering |
|---|---|
| `exact`, `verified: delivery` | `$n.bind_prop` / `$n.bind_readonly` (as R2) |
| `exact`, `verified: weak` or `none`, with a declared signal plan | `$n.bind(node, prop, signal, arg, get, set)`: the signal plan stays, no warning |
| `exact`, `verified: weak` or `none`, no signal plan | `$n.bind_prop` / `$n.bind_readonly` + **`bind_observation_unverified`** warning (error under `--strict-native`) naming the fact |
| anything else | unchanged (R2 table) |

Verification strength (`tools/metadata/common/observe.py` `verification`, `VERIFIED`):

- `delivery`: every ledger entry of the fact passes and its row made a native change (a native set not through our
  `set_prop`, a native trigger, or the forwarded object's set) and saw it announced through the real ABI
  (`observe` -> mechanism -> `host.prop_changed`), with the read value changed; for a `silent` row (a missed cause of
  a `partial`/`none` fact) the change was made and *not* announced.
- `weak`: the passing row only installed and removed the mechanism (`connect`) or checked the backend's answer
  (`refused`, `unsupported`).
- `none`: no passing row.

The strength comes from the toolkit's committed facts manifest (GTK, AppKit, WinUI: their generators decide the
check) or, for Qt, from the fact's `verify` block as `qt/gen_cpp.py` maps it; the ledger decides pass. It reaches
the compiler as `ObserveSpec.verified` (`delivery` | `weak` | `none`; absent reads `none`) next to `fact`
(`<Ns>:<Type>:<prop>`, for messages) in `metadata/semantics/<tk>.json`.

Unaffected on purpose: the GTK `SVL_M_OBSERVABLE` descriptor flag and the reactive getters (R1). They answer "can
this prop be observed" (class `exact`/`partial` with a mechanism), not "may `bind:` drop its signal plan"; a weakly
verified prop still has a working mechanism (its row installs it), and a getter that observes a silent prop is no
worse than the plain read it replaced. Only `bind:` lowering changes.

### Diagnostic `bind_observation_unverified`

Warning (error under `--strict-native`), in `codes::STRICT_NATIVE`:

```text
warning[bind_observation_unverified] (gtk): bind:is-active on <window>: `is-active` is exact on gtk (fact Gtk:Window:is-active),
but no native change has been seen delivered (its test only checks that the observation installs), so the bound value could go stale
  help: read it explicitly where it matters (node.getProp), or opt out with `|nonreactive`; verifying the fact (metadata/observe) removes this warning
```

Golden `crates/svelters-compiler/tests/r2/unverified-gtk.{svelte,expected}`; unit test
`r2_observe.rs::an_unverified_exact_fact_does_not_replace_the_signal_plan`. The typings
(`packages/native-runtime/types/observe.d.ts`) remark `delivery not verified (weak|none)` and how the compiler binds it.

## Earning the evidence

### GTK (generated rows, `testing_observe.rs`)

- **Generic native values** (`next_value`): flags (toggle a single-bit member), `GStrv` (append), `GVariant` (`s`, `*`,
  `b`, `i`, `u`), plus enum props of the non-GUI types (their `ref` is `enum`, which the rules' scalar test missed).
- **Sample values** for object and boxed props: `observe.values` in `metadata/rules/gtk.yaml`, by value GType (50
  types: widgets, models, factories, adjustments, files, icons, paintables, textures, RGBA, font descriptions, attr
  lists, tab arrays, transforms, ...). The rules copy them into the fact (`verify.values`), the generator emits
  `Check::Values`; each call sets a fresh value. Object-like values are compared by identity natively (the ABI may
  not read them: an expression reads Null); the row passes on a normalized read-back (GtkScrolledWindow wraps a
  child in a viewport) but requires the read value to change.
- **Triggers** (`observe.triggers`) for props whose plain set is ignored on a bare instance (no model, children,
  columns, pages) or that are read-only: Stack visible child, ComboBox/DropDown/SingleSelection/Notebook selection,
  TreeView expander column, Window focus widget, PopoverMenu submenu, TextBuffer can-redo, Image resource, every
  wrapping list model's `n-items`, DirectoryList loading, Gio.Application id/resource path, PrintOperation n-pages,
  BoolFilter expression, MessageDialog secondary markup.
- **Instances** (`observe.instances`) where the probe made none: bare Gio/Gtk Application, PrintOperation,
  MountOperation, and a tree view's selection (`verify.make: borrow:Gtk.TreeView.get_selection`; `On::Borrow`, also
  `On::Static`).

### Facts that turned out not exact (through the rules: `observe.missed`)

| Fact | Now | Why (verified by a `silent` row) |
|---|---|---|
| `Gtk:Image:file` | partial, missed `programmatic` | `gtk_image_set_from_file` with a loadable file changes `file` without `notify::file` (a property set notifies, through GObject) |
| `Gtk:TreeView:headers-clickable` | partial, missed `cross_prop` | it reads whether every column is clickable; appending a non-clickable column flips it without notify |

### Qt

Native triggers (`metadata/rules/qt.yaml` `observe.triggers`) for the 12 `connect`-only facts that have a reachable
change: animation `currentLoop`/`currentTime`/`direction`/`loopCount`/`state`, `QSequentialAnimationGroup:currentAnimation`,
`QVariantAnimation` `currentValue`/`easingCurve`, `QAbstractProxyModel:sourceModel`,
`QSortFilterProxyModel:filterRegularExpression`, `QItemSelectionModel:selectedIndexes`,
`QRegularExpressionValidator:regularExpression`.

### Unconfirmable groups

`observe.connect_only` in `metadata/rules/<tk>.yaml` names every `exact` fact without delivery evidence, by group
with the reason (`tools/metadata/common/observe.py` `unverified_groups`; coverage reports them). None is
unclassified on any toolkit.

| Toolkit | Group | Facts | Why |
|---|---|---:|---|
| GTK | print-backend | 8 | PrintUnixDialog (7), PrintJob: they enumerate printers; no instance is made |
| GTK | container-page | 7 | NotebookPage `position`, StackPage (6): pages belong to their container; a bare StackPage crashes |
| GTK | media-backend | 6 | MediaStream/MediaFile: abstract; `gtk_media_file_new` returns a private subclass the backend's tables do not name |
| GTK | display-server | 5 | Window `is-active`, Device `seat`, Drag `actions`/`selected-action`, DragIcon `child` |
| GTK | gio-service | 4 | Window `application`, Application `menubar` (need a registered application), DBusConnection, FileMonitor |
| GTK | realize | 4 | GLContext, Surface `cursor`, ATContext (2) |
| GTK | list-view | 3 | TreeListRow `expanded`, TreeListModel/ListHeader `n-items` |
| GTK | normalized-read | 2 | PasswordEntry `extra-menu` (reads its own wrapper menu), FileDialog `initial-file` (reads back unchanged) |
| GTK | font-config | 2 | Pango FontFamily/FontMap `n-items` |
| GTK | async-io, stateless-instance, layout-child, deprecated-content, newer-gtk | 1 each | BookmarkList `n-items`, SimpleAction `state`, FixedLayoutChild `transform`, ShortcutsWindow `section-name`, EnumList `n-items` (not in GTK 4.22) |
| Qt | no-element-or-type | 41 | QGraphics* effects/objects/transforms/scene/widget, QHeaderView, QRhiWidget, QScroller, QStackedLayout, QDataWidgetMapper: no element or NT2 type, so no row and no `bind:` can name them |
| Qt | crashes-bare | 4 | QDockWidget (3 of them are `<dock-widget>` props: `bind:` now warns) |
| Qt | window-input | 2 | QWidget `focus`, `isActiveWindow` |
| WinUI | not-exposed | 69 | props the backend does not expose (no row, not in the spec) |
| WinUI | input-state / edit-history / layout | 3 / 3 / 2 | IsPointerOver, IsPressed, FocusState / CanUndo, CanRedo, CanPasteClipboardContent / IsTextTrimmed, IsItemsHost |
| AppKit | read-only-on-class | 1 | NSView `tag` (read-only on NSView) |

## Completion record

Branch `ci/ov1` (worktree branch `worktree-agent-a8ec847313838ab49`), based on master be84a8cc. Not merged.

### Commits

| Commit | What |
|---|---|
| 34c854df | safety net: `ObserveSpec.verified`/`fact` from the ledger (`observe.verification`), `plan_bind` gate, `bind_observation_unverified`, typings remarks, `unverified-gtk` golden + unit test, docs |
| 0f08e454 | evidence: GTK generic values, `observe.values` samples, triggers, instances (`On::Borrow`/`On::Static`), `observe.missed` (2 facts partial), groups for all toolkits, Qt triggers (12), coverage, GTK/Qt ledger refresh; GTK `observe` no longer panics formatting an error for a kind outside the tables |
| (this record) | milestone file, REACTIVITY §13 #8 note already in 34c854df |

### Verification strength of `exact` facts (`metadata/coverage/observe.json` `exact_by_verification`)

| Toolkit | before (master be84a8cc): delivery / weak / none | after: delivery / weak / none |
|---|---|---|
| GTK | 1127: 831 / 238 / 58 | 1125: **1079** / 13 / 33 (2 facts became `partial`, verified silent) |
| Qt | 180: 121 / 14 / 45 | 180: **133** / 2 / 45 |
| AppKit | 150: 149 / 1 / 0 | unchanged (CI only; open) |
| WinUI | 207: 130 / 8 / 69 | unchanged (CI only; open) |

GTK: 248 facts gained a native-change row (238 weak -> 13, 58 none -> 33, minus the 2 now partial); 1182 observe
rows run locally, all pass. Qt: 2349 generated tests, 0 failed (12 new native rows). The brief's "309 weak" was
238 `connect` exact facts + 71 `refused` constants (constants are not `exact` and never lower `bind:`), so the
`exact` population without delivery evidence was 296 (238 + 58 without a row).

### `bind:` lowering changes (`svelters-compile` over every `.svelte` with `bind:` in the repo, new spec vs the same spec with every fact `delivery` = the R2 rule; R2 goldens excluded)

| Toolkit | After the safety net alone (34c854df) | After the evidence (0f08e454) |
|---|---|---|
| GTK | 1: `gallery-gtk` 06-layout `<notebook bind:page>` back to its signal plan (`Gtk:Notebook:page` was `connect`-only) | **0** (Notebook `page` now has a trigger row; the binding observes again, as on master) |
| Qt, AppKit, WinUI | 0 | 0 |

No repo `bind:` hits `bind_observation_unverified`. Spec props that would: GTK 35 (non-element kinds such as Window
`is-active`/`application`, Application, PasswordEntry `extra-menu`; none with a signal plan), Qt 3 (`<dock-widget>`
`allowed-areas`/`features`/`floating`), AppKit 1 (NSView `tag`), WinUI 8 (the weak ones above).

### Gates (all after the last code change, 0f08e454)

| Gate | Result |
|---|---|
| `ledger.py refresh --toolkit gtk` | 2703 facts, 2728 tests, 0 manual / 0 inferred not passing, 5 parked (the pre-existing 4 observe + 1) |
| `ledger.py refresh --toolkit qt` | 1252 facts, 2361 tests, 0 not passing, 0 parked |
| `ledger.py check` | pass: gtk 550 manual + 2153 inferred verified (5 parked); qt 634 + 631; appkit 79 + 491; winui 60 + 480 (8 parked, pre-existing) |
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast` | **2327 run, 2327 passed**, 7 skipped, no retries (1168 s) |
| `cargo nextest run -p svelters-compiler` | 171/171 (10 r2_observe, incl. the new golden and unit test) |
| `pnpm install`, `cargo build -p svelters-compiler`, `pnpm -r build` | rc 0 |
| `pnpm test` (incl. test:meta) | rc 0: types 19/19, style 17/17, native-runtime 85/85, svelte-check 93 files 0 errors 0 warnings, canvas 3/3, lists 2/2, compiled harness 81 PASS 0 FAIL (12 expected rejections), test:meta pytest 141 passed 1 skipped (+1 `test_verification_strength`), every `--check`, `ledger.py check` |
| `python3 tools/docs/build_ir.py`, `pnpm docs:test` | rc 0: 11 Python tests OK (1 skipped), 17/17 node |
| conformance (`UPSTREAM` = svelte@5.57.2 tarball, `TIMEOUT_MS=60000`) | **500 / 774** (rejected 168, runtime errors 27, mismatches 76, timeouts 0, skipped 3); baseline 500: +0 -0. A first run with the default 20 s timeout during the full nextest gave 498 (`effect-loop-infinite`, `error-boundary-20` timed out, the two RT1 notes as load-sensitive) |

### CI (`ci/ov1` @ 0f08e454)

| Workflow | Run | Result |
|---|---|---|
| js | 39582517077 | success |
| linux-gtk | 39582517121 | success |
| linux-qt | 39582517134 | success |
| macos-appkit | 39582517157 | success |
| windows-winui | 39582517078 | success |

### Open items

- **AppKit/WinUI weak facts** (1 + 8 `connect`, 69 WinUI not exposed): not attempted. A TextBox edit + Undo trigger in
  the WinUI harness (`Trig`) would verify `CanUndo`/`CanRedo`; IsPointerOver/IsPressed/FocusState need UI Automation.
  Both crates are being changed by N4-AppKit/N4-WinUI. Until then those props keep their signal plan or warn.
- **GTK backend panics on kinds outside the tables**: `backend.rs` `set_prop`/`get_prop` format their "does not belong
  to" error with `KINDS[kind - 1]` like `observe` did (fixed only in `backend/observe.rs`): a handle whose GType the
  tables do not name (GtkNoMediaFile from `gtk_media_file_new`) panics there instead of failing. Out of this lane.
- **Ledger counts a skipped row as pass**: a row the runner skips ("no kind X on this GTK") reports `pass` with a
  `skipped:` message the ledger drops. Locally (GTK 4.24) no observe row skips after the TreeSelection fix, so no
  delivery claim rests on a skip; the ledger could record skips as non-delivery.
- Rows assert at least one backend announcement plus a changed read value (as R0's 831); "exactly one delivery" is
  the host's per-checkpoint coalescing (R1 batch tests), not re-asserted per row.
- `Gtk:Image:file` and `Gtk:TreeView:headers-clickable` are now `partial` (their `SVL_M_OBSERVABLE` stays set; the
  GTK `OBSERVE` table class changed 3 -> 4).
- On merge: N3 also refreshes the GTK/Qt ledgers. Regenerate in order `gtk/infer.py`, `common/observe.py`,
  `gtk/gen_backend.py`, `ledger.py refresh`, `gen_semantics.py`, `gen_observe_outputs.py`, `coverage.py`, then
  `UPDATE=1` the `tests/r2` goldens if lowering changed.
