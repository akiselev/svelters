# WP-W (WinUI widget parity): handoff

The first agent stopped here because it ran out of context. A fresh agent should continue from branch `ci/wp-winui`.
The task brief is the WP-W lane brief: bring WinUI's element coverage close to GTK/Qt and write
`plans/milestones/WP-winui-widgets.md`.

## State: work in progress, nothing verified yet

- **Builds:** not green yet. The last `cargo check -p svelters-winui -p counter-win --tests --target x86_64-pc-windows-gnu`
  failed on one error: `windows_future` was unresolved. ContentDialog.ShowAsync had been selected into the bindgen filter.
  - **Fixed but not re-checked:** custom methods are now left out of the filter (`gen_filter` in `gen_backend.py`), and
    `bindings.rs` was regenerated with no `windows_future` references.
  - **First step:** re-run that check.
  - **Earlier errors:** before this one, the check reported only a missing payload getter, which a regen fixed.
- **CI:** no run yet on `ci/wp-winui`.
- **Ledger:** not imported.
- **Generated files not refreshed yet:** semantics, `gen_bindings`, coverage and the docs IR.
- **Tests not run yet:** Python tests and `ledger.py check`.

## What was built

### Ingest
- **Overrides:** 57 new override files in `metadata/overrides/winui/`, each marked `element:`.
  - One authoring helper, kept outside the repo, wrote them; the YAML files are now the source of truth.
  - Existing files edited:
    - `ToggleButton.yaml`: now an element, `text_prop: Content`.
    - `Button.yaml`: new `flyout` slot.
    - `UIElement.yaml`: new `context-flyout` slot.
  - New non-element override: `ItemsControl.yaml`, which adds the `items` slot.
- **Ingest result:** `ingest.py` now ingests 83 classes (was 17), plus 214 side types.
- **Offline regen:** the SDK cache and the winmd-tool binary were copied from another worktree into `target/` (neither is
  committed).
  - Copy them again from `../agent-a4956e8a76f5ce748/target/{winsdk,winmd-tool}`, or run `tools/metadata/winui/fetch.py`.
  - The winmds list in `target/winsdk/winmds.txt` holds absolute paths, so it is rewritten by `ingest.py`.

### Generator (`tools/metadata/winui/gen_backend.py`)
- **New wire types:**
  - **Values:** F32, CornerRadius, Color (`#rrggbb[aa]`), FontWeight, DateTime (`Value::DateTime`; accepts ISO text or
    ms), TimeSpan (ms).
  - **Objects:** Brush and SolidBrush (a color string becomes a SolidColorBrush), FontFamily, Uri, ImageSource (a URI
    becomes a BitmapImage), Icon (a Symbol name becomes a SymbolIcon, any other text becomes a FontIcon glyph).
  - **Nullable:** F64, DateTime, TimeSpan, Color.
  - **Where they live:**
    - The table is `VALUE_TYPES`.
    - Rust: `table.rs` `Ty`; parsing in `parse.rs` (Linux unit tests added); conversions in `conv.rs`.
    - Descriptors: `backend.rs` `prop_desc`.
    - Spec types: `gen_semantics.py` `wtype`.
- **Generalized slots:**
  - Single slots of any class, with the child cast to the property type. Example: `context-flyout` / `flyout` take a
    FlyoutBase.
  - Collections: UIElementCollection, ItemCollection, `IVector<T>`, `IObservableVector<T>`.
  - New `src/slots.rs`: `SlotOps` and `ManyOps`, plus the `many_ops!` macro over a generated
    `slotN_items() -> IVector<T>`.
  - `backend.rs`: insert, remove and `native_slot_items` now use these ops, and the up-front UIElement cast is gone.
  - Override key `child:` names the tag the generated slot test inserts.
- **Attached properties:** f64 ones are supported (Canvas.Left/Top). `Attr` gained `ty` and a `default: f64`.
- **Methods:**
  - `custom:` calls custom.rs. Example: `ContentDialog.ShowAsync` → `custom::content_dialog_show`, which calls IContentDialog
    vtable slot 55 through `types::rt::vcall`.
  - `check:` makes the generated test call a hand-written check in `testing.rs` (`content_dialog_show`).
  - `args:` gives the test arguments (`t::call_args`).
- **Events:** `check:` works the same way for events.
- **Schema:** `override.schema.json` was extended for `child`, `custom`, `check` and `args`, and attached values are now
  numbers.

### Bindings size
- windows-bindgen repeats base members on every derived class: with 65 kinds, `bindings.rs` grew to 340k lines.
- New `tools/metadata/winui/strip_inherited.py` drops the inherited copies (15,405 methods), leaving about 68.6k lines.
  - It runs as step 6 of `regen.sh`.
  - Hand-written code must therefore call base members through the declaring class
    (`d.cast::<UIElement>()?.XamlRoot()`).
  - Generated code already does this.

### Inference (`infer.py`, `metadata/rules/winui.yaml`)
- **Wire types:** `wire()` maps the new types (snake-case names from `VALUE_TYPES`).
- **New samples:** one per new wire type.
- **`string_objects` rule:** promotes Header, Title, Description and similar object props to `as: string` at 0.9.
- **`method_args` rule:** M1 at 0.9 for methods with listed arguments.
- **Current output:** props 596, notify 522, bindings 306, events 182, methods 17, observe exact 771.

### Other changes
- **Manual bindings:** `selected-index` on RadioButtons and on TabView, changed by `selection-changed`.
- **Backend:** `backend::first_xaml_root()`, used by ContentDialog.

## Target elements (65 kinds, up from 8)

All are "in progress": generated, but neither compiled green nor run on CI.

| Group | Kinds |
| --- | --- |
| Layout | border, scroll-viewer, viewbox, canvas (attrs left, top, z-index), split-view (content, pane), expander |
| Buttons | toggle-button, radio-button, repeat-button, hyperlink-button, drop-down-button, split-button (flyout), toggle-switch |
| Input | radio-buttons (items), combo-box, combo-box-item, number-box, date-picker, time-picker, color-picker, calendar-date-picker, calendar-view, password-box, auto-suggest-box, rich-edit-box, rating-control |
| Display | image, progress-ring, progress-bar, info-bar (content), info-badge, person-picture, teaching-tip (hero-content), symbol-icon, font-icon |
| Lists | list-view, list-view-item, grid-view, grid-view-item (`items` slot from ItemsControl) |
| Navigation | tab-view (tab-items), tab-view-item, navigation-view (menu-items, footer-menu-items, pane-header, pane-footer), navigation-view-item (menu-items), selector-bar (items), selector-bar-item |
| Menus | menu-bar, menu-bar-item, menu-flyout (a FlyoutBase, not a UIElement), menu-flyout-item, menu-flyout-separator, toggle-menu-flyout-item, menu-flyout-sub-item, command-bar (primary-commands, secondary-commands), app-bar-button, app-bar-toggle-button, app-bar-separator |
| Dialogs | content-dialog (show-async, hide, opened, closed with a result) |

## Counts

| Measure | Before | After (generated) |
| --- | --- | --- |
| Element kinds | 8 | 65 |
| Props | 145 | 596 |
| Events (including notify) | 76 | 701 |
| Slots | 3 | 27 |
| Attrs | 4 | 7 |
| Methods | 11 | 20 |
| Facts | 395 (WinUI verified) | 1,812 (1,702 inferred) |

Verified counts come only after the CI ledger import.

## What's left, in order

1. **Gnu cross-check.** Re-run it and fix compile errors. Typical fixes: add missing filter members to `RUNTIME_FILTER`,
   or route hand-written calls to inherited members through the declaring class.
   ```
   tools/dev/cargo-slot cargo check -p svelters-winui -p counter-win --tests --target x86_64-pc-windows-gnu
   ```
2. **Regenerate the other outputs.**
   - Run `tools/metadata/common/observe.py --toolkit winui`, `gen_semantics.py`, `common/gen_bindings.py` and
     `coverage.py`.
   - Note that `bindings.generated.js` is shared with WP-A, so expect conflicts.
   - Then run the Python tests: `pnpm test:meta`, or pytest under `tools/metadata`.
   - Then `cargo test -p svelters-winui --lib`.
3. **Harness resume after crashes (recommended).** A WinUI fail-fast kills the test process, and only one crash is parked
   per CI run.
   - Add `SVELTERS_TEST_RESUME` to `src/harness.rs`: skip tests that already have a result line.
   - In `.github/workflows/windows-winui.yml`, loop the generated step while the last results line is `"result":"start"`.
   - Raise the step timeout to about 30 min; there are now about 1.8k tests.
4. **CI loop.**
   - Commit, then `git push origin HEAD:ci/wp-winui`.
   - Download the `windows-winui-report` artifact:
     ```
     gh run download <id> -n windows-winui-report -D <dir>
     python3 tools/metadata/ledger.py import --toolkit winui <dir>
     ```
     The import parks failing inferred facts and regenerates.
   - After an import that parks facts, re-run bindgen and strip. `ledger.py` only re-runs `gen_backend`, so the filter
     shrinks but `bindings.rs` keeps the extra members, which is harmless.
   - Commit, push, and repeat until the job is green and `ledger.py check` passes.
   - Manual facts must pass: ContentDialog `show-async`/`closed` (check `content_dialog_show`), the 27 slots, the
     Canvas attrs, and the manual props (`Header`/`Title`/`Text`/`Label` as strings, `SelectedIndex = -1`).
5. **Gallery.**
   - Create `fixtures/gallery-winui` with `src/pages/*.svelte`, each with a `<!-- kinds: ... -->` header; `coverage.py`
     `gallery_kinds` reads these.
   - Add a counter-win `--fixture gallery-winui --selftest` mode, run in the workflow, that clicks through the pages.
6. **Style table.** Add color, background, border-color, border-radius, font-weight and font-family setters to
   `metadata/style/winui.yaml`, now that the props exist.
7. **Milestone record.** Write `plans/milestones/WP-winui-widgets.md`: the target-set justification (GTK/Qt gallery
   families), the design above, the CI table and the final counts.
8. **Final gates.**
   - `cargo check --workspace --all-targets` plus the gnu cross-check.
   - Full-workspace nextest under the flock.
   - `pnpm -r build`, `pnpm test`, `pnpm docs:test`.
   - All five CI workflows green.

## Known risks

- **Inferred samples may not round-trip on Windows** and will be parked by the import:
  - DatePicker.Date (time-of-day may be normalized);
  - SelectedIndex samples (3 and 7 fail on empty selectors);
  - brush props on templated parts.
- **Contained crashes:** a crashing test, such as a popup without a XamlRoot, stops the run; the harness-resume step above
  handles it.
- **ContentDialog in the tree:** inserting it into a panel and then calling ShowAsync is untested on WinUI 3 islands. If it
  fails, change the check to an uninserted dialog (the `first_xaml_root` path).
- **Virtual lists:** the `@svelters/lists` virtual-list protocol is not mapped for ListView. Keep this open.
- **Conflict risk with WP-A:** `packages/native-runtime/src/bindings.generated.js`, `metadata/coverage/*`,
  `tools/metadata/ledger.py` (untouched so far) and `docs/site` sidebar.
