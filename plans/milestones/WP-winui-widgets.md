# WP-W: WinUI widget parity

Bring the WinUI 3 backend's element coverage from the 8-kind slice to the widget families that the GTK and Qt galleries
show. Each new kind comes with slots, methods, observe facts, a gallery page, and a fact ledger verified on Windows CI.

## Target set

The target is the families that `fixtures/gallery-gtk` and `fixtures/gallery-qt` cover, mapped to their WinUI 3
counterparts.

| GTK/Qt family | WinUI kinds |
| --- | --- |
| Layout and containers | stack-panel, grid, border, scroll-viewer, viewbox, canvas, split-view, expander |
| Buttons | button, toggle-button, check-box, radio-button, repeat-button, hyperlink-button, drop-down-button, split-button, toggle-switch |
| Text and input | text-box, password-box, auto-suggest-box, rich-edit-box, number-box, slider, rating-control, radio-buttons, combo-box, combo-box-item |
| Pickers and dialogs | date-picker, time-picker, calendar-date-picker, calendar-view, color-picker, content-dialog, teaching-tip |
| Display | text-block, image, progress-bar, progress-ring, info-bar, info-badge, person-picture, symbol-icon, font-icon |
| Lists | list-view, list-view-item, grid-view, grid-view-item |
| Navigation | tab-view, tab-view-item, navigation-view, navigation-view-item, selector-bar, selector-bar-item |
| Menus, toolbars, context menus | menu-bar, menu-bar-item, menu-flyout, menu-flyout-item, menu-flyout-separator, toggle-menu-flyout-item, menu-flyout-sub-item, command-bar, app-bar-button, app-bar-toggle-button, app-bar-separator |
| Toplevel | window |

That makes 65 kinds, against a brief of "~30+".

**Why all 65 were kept:** every kind passed on Windows CI. Every fact has a ledger entry: 2,240 entries, 0 pending.
41 inferred facts are parked, each with its reason (see [Open items](#open-items)). So there was no long tail to cut.

## As built

### Ingest and overrides (`metadata/overrides/winui/`)
- **New element overrides:** 57, each with `element:`.
- **Existing overrides changed:**
  - `ToggleButton` is now an element (`text_prop: Content`).
  - `Button` gained a `flyout` slot.
  - `UIElement` gained a `context-flyout` slot.
  - `ItemsControl` (new) adds the `items` slot.
  - The `RangeBase.Value` sample is now 0.5, inside RangeBase's default range of 0..1. ProgressBar does not widen that
    range, so the old sample of 7 clamped to 1.
- **Ingest result:** `ingest.py` now ingests 83 classes (was 17) and 214 side types.

### Generator (`tools/metadata/winui/gen_backend.py`)
- **Value wire types:** F32, CornerRadius, Color (`#rrggbb[aa]`), FontWeight, DateTime and TimeSpan.
- **Object wire types:**
  - Brush and SolidBrush: a color string becomes a SolidColorBrush.
  - FontFamily and Uri.
  - ImageSource: a URI becomes a BitmapImage.
  - Icon: a Symbol name becomes a SymbolIcon, any other text a FontIcon glyph.
- **Nullable variants:** F64, DateTime, TimeSpan and Color.
- **Where the code lives:**
  - The table is `VALUE_TYPES`.
  - Parsing is in `parse.rs`, with Linux unit tests.
  - Conversion is in `conv.rs`.
  - Descriptors are in `backend.rs` `prop_desc`.
  - Spec types are in `gen_semantics.py` `wtype`.
- **Generalized slots (`src/slots.rs`, `SlotOps`/`ManyOps`):**
  - Single slots of any class, with the child cast to the property type (`flyout` and `context-flyout` take a
    FlyoutBase).
  - Collections: UIElementCollection, ItemCollection, `IVector<T>` and `IObservableVector<T>`.
  - There are 27 slots in all.
- **Attached f64 properties:** Canvas `left`, `top` and `z-index`.
- **Method and event keys:**
  - `custom:` calls hand-written code. `ContentDialog.ShowAsync` calls `custom::content_dialog_show`, a vtable call that
    keeps windows-future out of the bindings.
  - `check:` makes the generated test call a hand-written check (`testing::content_dialog_show`).
  - `args:` gives the test arguments.
  - Custom methods are left out of the bindgen filter.
- **Manual bindings:** `selected-index` on radio-buttons and tab-view, changed by `selection-changed`.

### Bindings size (`tools/metadata/winui/strip_inherited.py`)
- **Problem:** windows-bindgen repeats every base-class member on each derived class. With 65 kinds, that would make
  `bindings.rs` 340k lines.
- **Fix:** `strip_inherited.py` runs as step 6 of `regen.sh` and drops the 15,405 inherited copies. The file ends at
  68.7k lines.
- **Consequence:** hand-written code calls base members through the declaring class
  (`d.cast::<UIElement>()?.XamlRoot()`).

### Inference (`infer.py`, `metadata/rules/winui.yaml`)
- **Wire types:** inference maps all the new types.
- **New rules:**
  - `string_objects` promotes Header, Title, Description and similar object props to `as: string`.
  - `method_args` covers methods that take listed arguments.
- **Fix:** `infer.py` now fingerprints the observe layer when it prunes `_parked.json`, as `gtk/infer.py` does. Before
  this fix, it dropped parked observe facts on every run, so `infer.py --check` failed after any import that parked an
  observe fact.

### Shared generators
- **`gen_bindings.py` `winui_bindings`:** walks the class chain nearest first, so a subclass binding shadows a base
  binding of the same name. CheckBox's manual `is-checked` now shadows ToggleButton's. A duplicate within one class is
  still an error.
- **Style table (`metadata/style/winui.yaml`):** new setters `color`, `background`, `border-color`, `border-radius`,
  `font-family` and `font-weight`.
  - **Colors:** the text becomes a SolidColorBrush. Named colors are not accepted.
  - **Font weight:** numeric weights only.
  - **`border-radius`:** a single length. `style_table.py` now lets a `length` target WinUI's CornerRadius record.

### Test harness and CI (`crates/svelters-winui/src/harness.rs`, `.github/workflows/windows-winui.yml`)
- **Resume after a crash:** with `SVELTERS_TEST_RESUME=1`, the harness skips every test that already has a line in
  `SVELTERS_TEST_RESULTS`.
  - The `generated` step reruns while the last line is a bare `start`, up to 60 times.
  - It still fails if any test crashed.
  - The ledger records a test with only a `start` line as `crash`.
  - So one CI run now records every crash instead of only the first.
- **Timeouts:** the step limit is now 30 minutes and the job limit 90 minutes. The step actually takes about 2.5 minutes.

### Gallery (`fixtures/gallery-winui`)
- **Layout:** nine family pages in a sidebar App.
- **Kinds header:** each page starts with `<!-- kinds: ... -->`, which `coverage.py` `gallery_kinds` now reads for
  WinUI.
- **Selftest:** `counter-win --fixture gallery-winui --selftest` runs as a new CI step. It:
  - clicks every sidebar button through UI Automation;
  - checks that each page created the kinds its header lists;
  - checks that there are no JS errors;
  - writes `gallery-report.json`.
- **Result:** 64 of the 65 kinds are on a page. The exception is `window`, which is the gallery's own toplevel.

## Counts

| Measure | Before (master `a1d62a61`) | After |
| --- | --- | --- |
| Element kinds | 8 | 65 |
| Props exposed | 145 | 585 |
| Signals exposed | 76 | 182 |
| Methods exposed | 11 | 19 |
| Child slots | 3 | 27 |
| Attrs | 4 | 7 |
| Two-way bindings | 48 | 298 |
| Observe facts verified by a native change | 130/251 | 545/846 |
| Ledger facts verified | 395 | 2,240 (155 manual, 2,085 inferred); 41 parked, 0 pending |
| Kinds on a gallery page | 0 | 64 |

## Measurements

These are release builds of `counter-win` for `x86_64-pc-windows-gnu` on the 31 GB dev VM with 4 jobs. Build times
are a rebuild of `svelters-winui` + `counter-win`, with sccache off (`RUSTC_WRAPPER=`).

| | master | WP-W |
| --- | --- | --- |
| `bindings.rs` lines | 64,859 | 68,736 |
| `generated/dispatch.rs` lines | 3,373 | 12,405 |
| Rebuild time | 60.6 s | 118 s |
| Rebuild peak RSS | 0.78 GB | 1.04 GB |
| `counter-win.exe` size | 10.8 MB | 13.2 MB |

- **Debug build:** the debug gnu build of `counter-win` takes 110 s, with a peak RSS of 1.2 GB.
- **CI:** the `generated` step runs 1,902 tests in about 2.5 minutes.
- **Without the strip:** `bindings.rs` would be about 340k lines.

## Tests and gates

All were re-run after the last code edit.

### Windows CI
- **Runs:** 39680069745, 39687194974 and 39692006835.
- **First run:** 18 inferred fails and 1 manual fail; no crashes.
  - The manual fail was `RangeBase.Value` on progress-bar; the sample was fixed.
  - Importing the run parked 40 inferred facts.
- **Second run:** 1 fail, `observe_radio_buttons_selected_index`, which was parked.
- **Third run:** green.
  - `generated`: 1,902/1,902.
  - `types_core` and `types_verify`: 45 + 8.
  - counter-win selftest and test, and the gallery selftest: passing.

### Local gates
- `cargo check --workspace --all-targets`: passes.
- `cargo check -p svelters-winui -p counter-win --tests --target x86_64-pc-windows-gnu`: passes.
- `cargo test -p svelters-winui --lib`: 31 passed. This includes the new harness round-trip test.
- Full-workspace nextest: see the completion record.
- `pnpm -r build`, `pnpm test` (with `ledger.py check` and `coverage.py --check`) and `pnpm docs:test`: pass.

## Completion record

- **Branch:** `ci/wp-winui`.
- **CI:** all five workflows green on `e6f946d7`.
  - linux-gtk 39692006790
  - linux-qt 39692006772
  - macos-appkit 39692006852
  - js 39692006763
  - windows-winui 39692006835

## Open items

### Parked inferred facts (41)
Each is listed in `metadata/inferred/winui/_parked.json` with its reason, and none passes silently.

- **TextReadingOrder** on TextBlock, TextBox, PasswordBox, RichEditBox and NumberBox: the property reads back
  `default` after `use-flow-direction` is written.
- **Invalid inferred sample:** the `"svelters"` sample is rejected by `PasswordChar`, `ClockIdentifier` and
  `CalendarDatePicker.DateFormat`. A valid sample would let these be retried.
- **RatingControl `Value` and `PlaceholderValue`:** the value is clamped to `MaxRating` (5).
- **`Selector.SelectedIndex` and `IsSynchronizedWithCurrentItem`** on an empty combo-box. The manual
  `selected-index = -1` facts still pass.
- **`FlyoutBase.ShowMode`:** reads back `standard`.
- **`Control.IsFocusEngaged`:** needs focus.
- **`ListViewBase.SelectAll`:** needs multiple selection mode.
- **RadioButtons `selected-index`:** neither the notify callback nor the observe delivery fires for a native change.

Each parked prop also takes its notify and binding facts with it.

### Other open items
- **Virtual lists:** the `@svelters/lists` virtual-list protocol is not mapped for ListView/GridView. Items are child
  elements only.
- **ContentDialog:** `show-async` is verified by the generated check. The gallery's dialog page does not open the dialog
  in the selftest.
- **Window:** `window` is not on a gallery page, since it is the gallery's toplevel.
- **Element typings and docs pages:** still GTK/Qt only. The docs IR has no WinUI frontend.
- **Unknown denominator:** the MATRIX widget denominator (87 WinUI widget classes) is not produced by `coverage.py`,
  which counts 65/69 of the ingested concrete kinds.

## Conflict risks

These files are shared with WP-A (AppKit widgets) and other lanes:
- `packages/native-runtime/src/bindings.generated.js` and `types/bindings.d.ts` (regenerated);
- `packages/style/src/props.generated.js` (WinUI tag sets);
- `metadata/coverage/{observe.json,winui.*}`;
- `tools/metadata/common/gen_bindings.py` (`winui_bindings` only);
- `tools/metadata/coverage.py` (WinUI rows read the gallery);
- `tools/metadata/style/style_table.py` (the CornerRadius `length` case);
- `tools/metadata/common/test_gen_bindings.py` (`toggle-button` text props);
- `pnpm-lock.yaml` (new `fixtures/gallery-winui` importer).

After a merge, regenerate in this order:
1. `observe.py`
2. `gen_backend.py`
3. `gen_bindings.py`
4. `gen_semantics.py`
5. `gen_observe_outputs.py`
6. `gen_props.py`
7. `coverage.py`

`gen_backend` reads `metadata/observe/winui.json`. Running it before `observe.py` leaves its output stale.
