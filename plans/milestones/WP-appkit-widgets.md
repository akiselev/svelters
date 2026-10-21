# WP-A: AppKit widget parity

Lane brief: bring AppKit element coverage from the MAC1 slice (9 elements) towards GTK/Qt, with slots, methods,
observe facts and gallery coverage. Builds on MAC1 (`docs/notes/mac1-appkit.md`), AI1 (the AppKit ingest, rules,
probe and ledger), R4 (AppKit observe) and N4 (non-GUI types). The work was done by two agents in turn; the first
one's state is in `docs/notes/wp-appkit-handoff.md`. This file records WP-A as built.

## Result

| Measure (`metadata/coverage/appkit.md` unless noted) | Before (master 8e92af95) | After |
| --- | --- | --- |
| Ingested classes (`metadata/appkit/NS*.json`) | 10 | 32 |
| Element tags | 9 | 30 |
| Properties exposed | 329/336 | 662/679 |
| Signals exposed | 22/22 | 42/42 |
| Methods exposed (`bind:this`) | 7/17 | 79/120 |
| Child slots annotated (own) | 4 | 13 |
| Two-way bindings | 12 | 38 |
| Widget facts verified (manual and inferred) | 226/226 | 579/579 |
| Ledger facts (incl. observe and types) | 696 | 1396 verified (398 manual, 998 inferred), 4 parked |
| Observe facts (`metadata/observe/appkit.json`) | 344 | 691 |
| Observe facts verified by a native change | 154 (exact 149, partial 5) | 336 (exact 334, partial 2) |
| Kinds on a gallery page | 0 | 29 (6 pages) |
| Dangerous members | 15 | 23 |

The 21 new tags: `scroll-view`, `split-view`, `tab-view`, `tab-view-item`, `pop-up-button`, `combo-box`,
`segmented-control`, `date-picker`, `color-well`, `image-view`, `stepper`, `level-indicator`, `search-field`,
`token-field`, `path-control`, `switch`, `text-view`, `visual-effect-view`, `menu`, `menu-item`, `panel`. NSText is
ingested as the base of NSTextView (no tag). NSView gained a non-default `menu` slot (a context menu).

Of the 30 tags, 26 are in MATRIX's ~56 AppKit widget classes (the NSView subclasses plus NSWindow); `menu`,
`menu-item`, `tab-view-item` (not views) and `panel` (an NSWindow subclass) are outside that count.

## As built

### Overrides and schema (`metadata/overrides/appkit`, `tools/metadata/appkit/override.schema.json`)

22 new override files; NSWindow, NSControl, NSButton, NSProgressIndicator and NSView got methods (NSView also the
`menu` slot, `declared_on: NSResponder`). New schema keys:

- `element.test_setup`: props the generated tests set right after create (a pop-up-button needs items before an
  index exists; a segmented control needs segments).
- `element.read_errors`: getters that raise outside a documented context. The readable test expects an error
  from them instead of a value: NSMenu `properties-to-update` (only inside `-menuNeedsUpdate:`) and
  NSSegmentedControl `double-value-for-selected-segment` (only for the momentary-accelerator tracking mode).
- `props[].write {selector, clear}`: a read-only property written through methods (`item-titles` is
  `removeAllItems` + `addItemsWithTitles:`; `index-of-selected-item` is `selectItemAtIndex:`;
  `selected-tab-view-item` is `selectTabViewItem:`).
- `setters[].count`: an indexed setter (`labels` is `setSegmentCount:` + `setLabel:forSegment:`).
- `slots[].child` and `slots[].declared_on`: the accepted child class of a slot whose children are not views
  (tab-view items, menus, menu items), and a slot whose selector is declared above the ingested chain.
- `methods[].args`, `methods[].expect.returns`: method tests with arguments and a return value.
- `signals[].verify`: `select_tab` (a selection through the tab view's delegate) and `app_send_action` (a
  menu item's action, sent the way NSApplication sends it).

### Generator, backend and harness

- `tools/metadata/appkit/gen_backend.py`: the `PROP_WRITES` table, slot tests for non-view children, method
  arguments and returns, tab-view delegate tests, the `SETUP` table, `read_errors`; observe rows never click an
  NSPopUpButton (its menu would open modally).
- `crates/svelters-appkit`: `PropWrite` (`tables.rs`, `lookup.rs`); `backend.rs` `write_through`, a
  `TabViewDelegate` installed on every tab view (`tab-view-should-select-tab-view-item` is a sync veto), generic
  `delegate_veto`/`delegate_post` (window `should_close` uses it), `native_remove` without the superview check for
  non-view children; `testing.rs` `set_setup`, `app_send_action`.
- `crates/svelters-apple/src/objc/bridge.rs`: an NSURL from a path or URL string.
- Shared generators (small and additive): `gen_bindings.py` and `gen_semantics.py` treat `write:` props as
  writable; slot `accepts` uses the `child` class.
- `metadata/rules/appkit.yaml`: observe fallbacks for the new value controls (target-action, the text system's
  notification on `NSText:string`), a deny rule for NSComboBox `uses-data-source` (dangerous).

### Probe and ledger

The probe for the new classes ran on the macOS runner (SDK 26.1) and is committed in
`metadata/probe/appkit/26.1.json`. Its KVO results moved 188 observe facts of the new classes from `set_only`/`none` (rule O8, not
probed) to `exact`. The ledger was imported from CI run 39681421950: 1396 facts, 0 failing; 4 inferred facts parked
(below).

### Gallery

`fixtures/gallery-appkit`: `App.svelte` is a sidebar of family pages (window #1, outer stack-view #2, sidebar
stack-view #3, one button per page), with six pages, each with a `<!-- kinds: ... -->` header:

| Page | Kinds |
| --- | --- |
| 00-overview | the MAC1 slice (window, stack view, text field, button, secure text field, slider, progress indicator, box) |
| 01-buttons | button (push, destructive, check, radio), switch, segmented control, pop-up buttons, stepper; `bind:` on each |
| 02-text | text field, search field, token field, combo box, a text view in a scroll view; `bind:` on each |
| 03-values | slider, progress indicators, level indicators, date picker, color wells (one with an NSColor from `svelters:appkit/appkit`), path control, image views (SF Symbol, named image) |
| 04-containers | tab view with two items (a vetoable selection), split view, boxes, visual effect view, a plain view |
| 05-menus | a pop-up button whose items are menu-item children (with a submenu), a context menu in a view's `menu` slot |

`examples/counter-mac/tests/gallery.rs` (a `harness = false` test, run by the `counter-mac gallery` step of
`macos-appkit`) has one case per page. Each case loads the bundle in a fresh host and backend, clicks the page's
sidebar button with `performClick:`, and checks: no JS errors; the page's classes are live (every handle the
backend holds, so menus and tab items count); every `echo <name>: N` label reads 0; a screenshot lands in
`target/gallery/appkit-<page>.png` (uploaded with `macos-appkit-artifacts`). Then per page: the `$effect` writes
reached the native controls (bound segment, pop-up index, stepper, combo box, text view); the text view is the
scroll view's document view; the path control's URL and the image view's image came from strings; a native
`selectTabViewItemAtIndex:` reaches JS through the delegate (one should-select, one did-select); a menu item's
action sent through `-[NSApplication sendAction:to:from:]` runs its `onaction`; the context menu has its items.

`fixtures/gallery-common/check-pages.mjs` is toolkit-aware: AppKit's sidebar click is the `action` event, and its
tests file is `examples/counter-mac/tests/gallery.rs`. `pnpm test` runs it for `gallery-appkit`.
`tools/metadata/coverage.py` fills AppKit's gallery column from the page headers.

## Deviations and findings

- **Element count.** The handoff counted 31 elements; it is 30 (9 + 21 tags). NSText is ingested but has no tag.
- **Probe-first workflow.** New classes need two CI rounds before their facts settle: ingest (new selectors), then
  probe. The first probe moved most of the new classes' facts to `exact`.
- **First macOS run of the generated tests:** 6 of about 1110 failed. Wrong enum samples (`image-scaling`,
  `image-alignment`, `token-style`), two getters that raise out of context (now `read_errors`), and an observe row
  conflict: a `NSTextView:string` fallback added a KVO-less subclass row that the backend resolves first on
  `<text-view>`, while the declaring class's row (NSText, KVO seen by the probe) expected KVO. The fallback now sits
  on NSText only.
- **Parked.** Four inferred facts: NSSegmentedControl `int-value` and `integer-value`, as props and bindings. They
  alias the selected segment, and the inferred sample 3 is out of range for three segments
  (`NSRangeException`). `bind:selected-segment` is the binding to use.
- **Combo box echo.** `-selectItemAtIndex:` posts `NSComboBoxSelectionDidChangeNotification`, which is not
  documented; the gallery measured two posts for two programmatic writes. The binding's echo strategy is
  `compare` (the host drops the echo), so the binding is correct; the provenance now says so.
- **AppKit hands back private subclasses** (an NSSegmentedControl instance's class name differs), so the
  gallery test matches by `isKindOfClass:` after an exact-name lookup.
- **Local cross-check of the macOS tests.** `cargo check -p counter-mac --tests --target aarch64-apple-darwin`
  fails on Linux in the C build scripts (QuickJS, `objc2-exception-helper`). With a stub C compiler that only writes
  empty objects (`cargo check` never links) and `RUSTC_WRAPPER` unset, all counter-mac test targets, including
  the generated tests and the gallery, type-check locally:
  `RUSTC_WRAPPER= CC_aarch64_apple_darwin=<stub> AR_aarch64_apple_darwin=ar cargo check -p counter-mac -p svelters-apple -p svelters-appkit --all-targets --target aarch64-apple-darwin`.
  The stub (a shell script that writes an empty file for `-o<path>`/`-o <path>`/`-Fo<path>` and exits 0) is not
  checked in.

## Tests

- Generated (`examples/counter-mac/tests/generated.rs`): the 1270 widget facts of
  `crates/svelters-appkit/src/generated/facts.json`; the 126 type facts run in `tests/types.rs`. The run imported:
  1400 results, 4 failing (the parked ones).
- `examples/counter-mac/tests/gallery.rs`: 7 cases (6 pages + `every_page_has_a_case`).
- `fixtures/gallery-appkit` `pnpm test`: build + check-pages (6 pages open in the TestHost without a JS error).
- `tools/metadata` pytest and every generator's `--check` (`pnpm test:meta`).

## Completion record

(Filled in below after the final gates.)

## Open items

- **No layout sizing.** AppKit has no width/height props or constraints from JS. In a stack view, views without
  an intrinsic size collapse or stretch (the split view is a thin strip, a plain view fills the rest, the second
  image view has no size; see the gallery screenshots). A document view or a plain view's subviews need a `frame`.
  This needs a small layout feature (constraint props or style `width`/`height`), which is not in this lane.
- **Transient windows.** A `<panel>` inside a window is still refused by `Inner::insert`, so panels work only at
  the root (not on a gallery page). Idea: `addChildWindow:ordered:` with a sentinel slot, closing children in
  `will_close`.
- **Lists.** No `virtuallist` on NSTableView/NSOutlineView yet (a data source and delegate class plus a row pool,
  as in M5); no grid view or toolbar.
- **Object-typed bindings.** `date-picker` `date-value`, `color-well` `color` and `path-control` `url` have observe
  fallbacks but no `bind:` (the binding types are scalar).
- **Stringification.** `write_through` passes list items as strings (combo box `object-values` become strings).
- **Mutual exclusion.** On a pop-up button, `item-titles` (via `removeAllItems`) and a `menu` child exclude each
  other; `remove-all-items` is marked dangerous.
- **Real hardware.** Everything ran on CI runners only (macOS 26, SDK 26.1 for metadata).

## Conflict risks

- Shared generators with WP-W: `tools/metadata/common/gen_bindings.py` and `tools/metadata/semantics/gen_semantics.py`
  (`write:` props count as writable; slot `accepts` from `child`), and `tools/metadata/coverage.py` (AppKit's
  `gallery` column). WP-W may change the same lines for WinUI.
- `fixtures/gallery-common/check-pages.mjs`: per-toolkit `TESTS`/`CLICK` maps (a WinUI gallery would add an entry).
- Regenerated shared outputs: `packages/native-runtime/src/bindings.generated.js`,
  `packages/native-runtime/types/{bindings,observe}.d.ts`, `packages/style/src/props.generated.js`. A merge
  conflict there is resolved by rerunning the generators, not by hand.
- `.github/workflows/macos-appkit.yml`: one new step (`counter-mac gallery`).
