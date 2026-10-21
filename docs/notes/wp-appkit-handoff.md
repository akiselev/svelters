# WP-A (AppKit widget parity): handoff

Branch `ci/wp-appkit`, based on local master `8e92af95` (E2 merged). Written by the first lane agent when it hit its context
limit; a fresh agent should continue from here. Lane brief: AppKit widget coverage towards GTK/Qt, milestone record
`plans/milestones/WP-appkit-widgets.md` (not written yet).

## State in one paragraph

Overrides for 22 new AppKit classes were written. Their metadata was ingested twice on the macOS CI runner (Xcode SDK
26.1) and committed. The generator, backend, harness, schema and downstream generators were extended. Everything was
regenerated locally. **Nothing new has run on macOS yet.** No probe has run for the new classes, no generated test has
run, and the ledger has not been imported. The local metadata pytest passes (155 passed, 1 skipped). The
`aarch64-apple-darwin` cross-check of svelters-apple and svelters-appkit (all targets) and a type-check of the regenerated
`examples/counter-mac/tests/generated.rs` both passed. The full gates (workspace check, nextest, pnpm) have **not** been run.

## Target elements

Before: 9 elements. After this branch: 31 elements (9 old + 22 new). "Ingested" means `metadata/appkit/<Class>.json` came
from the CI ingest. "Generated" means it is in `generated/tables.rs` and has tests in `generated.rs`. "Unverified" means
no macOS run yet.

| Tag | Class | State |
| --- | --- | --- |
| scroll-view | NSScrollView | ingested, generated, unverified; slot `document-view` (one) |
| split-view | NSSplitView | ingested, generated, unverified; slot `arranged-subviews` (many); 2 notification signals |
| tab-view | NSTabView | ingested, generated, unverified; slot `tab-view-items` (child `tab-view-item`); `selected-tab-view-item` written via `selectTabViewItem:`; 4 delegate signals via a new `TabViewDelegate` (should-select is a sync veto) |
| tab-view-item | NSTabViewItem | ingested, generated, unverified; not a view; slot `view` (one); text_prop `label` |
| pop-up-button | NSPopUpButton | ingested, generated, unverified; `item-titles` (removeAllItems + addItemsWithTitles:), `index-of-selected-item` (selectItemAtIndex:), `bind:index-of-selected-item` (action); slot `menu` (default, child `menu`); `test_setup` gives it 3 items |
| combo-box | NSComboBox | ingested, generated, unverified; `object-values` and `index-of-selected-item` written through methods; 4 notifications; bind on index (echo compare) |
| segmented-control | NSSegmentedControl | ingested, generated, unverified; indexed setter `labels` (setSegmentCount: + setLabel:forSegment:); `bind:selected-segment` |
| date-picker | NSDatePicker | ingested, generated, unverified; no bind (object type, see open items) |
| color-well | NSColorWell | ingested, generated, unverified; no bind |
| image-view | NSImageView | ingested, generated, unverified |
| stepper | NSStepper | ingested, generated, unverified; `bind:double-value` |
| level-indicator | NSLevelIndicator | ingested, generated, unverified |
| search-field | NSSearchField | ingested, generated, unverified (text binding inherited from text-field) |
| token-field | NSTokenField | ingested, generated, unverified |
| path-control | NSPathControl | ingested, generated, unverified; `url` takes a string (bridge: NSURL from a path or URL string) |
| switch | NSSwitch | ingested, generated, unverified; `bind:state` |
| text-view | NSTextView (+ base NSText, ingested, not an element) | ingested, generated, unverified; 4 notification signals; `bind:string` via text-did-change arg0 |
| visual-effect-view | NSVisualEffectView | ingested, generated, unverified |
| menu | NSMenu | ingested, generated, unverified; not a view; slot `items` (child `menu-item`); 3 notifications |
| menu-item | NSMenuItem | ingested, generated, unverified; slots `submenu` (default, child `menu`) and `view`; `action` via target-action, verified by the new `app_send_action` |
| panel | NSPanel | ingested, generated, unverified; toplevel like window (utility style mask) |
| (view) | NSView | new non-default slot `menu` (context menu, `declared_on: NSResponder`, unchecked by the generator); 4 new methods |

Not started: table-view / outline-view (an AppKit `virtuallist` over NSTableView: a data-source/delegate class plus a
row pool; see `plans/milestones/M5-lists.md`), grid-view, toolbar, transient windows (inserting a toplevel into a
window is still refused in `Inner::insert`; idea: `addChildWindow:ordered:` with a sentinel slot, closing children in
`will_close`), the gallery, and the hand-written widgets test.

Methods added to existing classes (bind:this coverage): NSWindow +11, NSControl +3 (size-to-fit, validate-editing,
abort-editing), NSButton +2, NSProgressIndicator +1 (`increment-by` with args), NSView +4.

## Counts so far (local generator output, unverified)

- Elements: 9 → 31.
- `generated/facts.json`: 1110 facts (839 inferred). Before: 696 in the ledger (226 widget facts + 126 type facts + 344
  observe).
- Observe facts (`metadata/observe/appkit.json`): constant 0, set_only 341, exact 150, partial 20, none 181 (692). Before: 344.
  The new classes have no probe yet, so their facts are only `set_only`/`none` from rules O8. They become exact or
  partial once the probe for the new classes is committed.
- `gen_bindings`: 410 bindings on 87 tags (all toolkits).
- Dangerous members: appkit 15 → 23.
- Coverage (`metadata/coverage/appkit.md`) **not regenerated yet**. Run `python3 tools/metadata/coverage.py` after the
  ledger import. MATRIX numbers come from that file.

## CI runs (macos-appkit)

| Run | Head | Result |
| --- | --- | --- |
| 39673353659 | 5b79543c (overrides only) | metadata job: ingest OK, infer --check failed (expected); artifact committed in fce206cf. appkit job: the probe step failed on schema validation (`args` unknown; the schema was fixed later) |
| 39674344149 | fce206cf (ingest round 1 + ingest reads write/count selectors) | metadata job: ingest OK; artifact (4 changed JSON files) committed in the handoff commit. appkit job was still queued at handoff |

Other workflows were not checked. js will fail until the ledger is imported, because `ledger.py check` and the generated
facts have new fingerprints.

## What changed (files)

- `metadata/overrides/appkit/*.yaml`: 22 new files, 5 edited (methods, NSView `menu` slot).
- `tools/metadata/appkit/override.schema.json` adds these keys:
  - `element.test_setup`;
  - `props[].write {selector, clear}`;
  - `setters[].count`;
  - `slots[].child` and `slots[].declared_on`;
  - `methods[].args` and `expect.returns`;
  - `signals[].verify`: `select_tab` and `app_send_action`.
- `tools/metadata/appkit/ingest.py` also ingests the `write`/`count` selectors.
- `tools/metadata/appkit/gen_backend.py`:
  - new `PROP_WRITES` table;
  - slot tests for non-view children;
  - method args and returns;
  - tab-view delegate tests (`select_tab_body`);
  - `SETUP` table plus `Harness::set_setup` in `main`;
  - observe rows never click an NSPopUpButton (that would open its menu modally); NSSwitch is clickable; NSTextView types.
- `crates/svelters-appkit/src/tables.rs` (`PropWrite`), `lookup.rs` (`prop_write`).
- `crates/svelters-appkit/src/backend.rs`:
  - `write_through`;
  - `Native.delegate` is now `Retained<NSObject>`, set by `install_delegate`;
  - `TabViewDelegate`;
  - generic `delegate_veto`/`delegate_post`; `should_close` uses `delegate_veto`;
  - `native_remove` skips the superview check for non-view children.
- `crates/svelters-appkit/src/testing.rs`:
  - `set_setup`, applied in `create`;
  - `app_send_action`;
  - `send_action` falls back to `app_send_action` for menu items.
  - This edit re-fingerprints all observe facts, which is expected; the ledger import covers it.
- `crates/svelters-apple/src/objc/bridge.rs`: `object_in` makes an NSURL from a string.
- Shared files (small, additive):
  - `tools/metadata/common/gen_bindings.py`: props with `write:` count as writable;
  - `tools/metadata/semantics/gen_semantics.py`: `write:` props are writable; slot `accepts` uses the `child` class.
- `metadata/rules/appkit.yaml`: observe fallbacks for the new controls.
- Regenerated:
  - `generated/{tables.rs,facts.json}` and `counter-mac/tests/generated.rs`;
  - `metadata/inferred/appkit/**` and `metadata/observe/appkit.json`;
  - `metadata/semantics/appkit.json`;
  - `packages/native-runtime/src/bindings.generated.js` and `types/*.d.ts`;
  - `packages/style/src/props.generated.js`.

## What's left (in order)

1. Look at run 39674344149's appkit job, or push again and look at the next one. The `generated` step runs the new
   tests for the first time. Expect failures, and fix them:
   - manual facts must pass;
   - inferred ones get parked on import;
   - suspects: the `readable_*` cases (a getter that raises on an unconfigured object), the popup `menu` slot clear (setMenu: nil), the tab-view veto test, the menu-item `submenu` and `view` slots, the NSPanel window tests.
2. Download the `appkit-probe` artifact. Copy `26.1.json` (named after the SDK in `_enums.json`) to `metadata/probe/appkit/`.
   Then run, in order:
   - `infer.py`;
   - `observe.py --toolkit appkit`;
   - `gen_backend.py`;
   - `gen_bindings.py`;
   - `gen_semantics.py`;
   - `gen_observe_outputs.py`;
   - `style/gen_props.py`.

   Push. Then import the run that has the matching fingerprints:
   `python3 tools/metadata/ledger.py import --toolkit appkit appkit-results/generated.jsonl`. After that run
   `coverage.py`, `ledger.py check`, and `pytest tools/metadata`.
3. Gallery:
   - `fixtures/gallery-appkit/src/pages/*.svelte` with `<!-- kinds: NS... -->` headers, and an App.svelte sidebar
     (window, outer stack-view, sidebar stack-view, buttons);
   - `examples/counter-mac/tests/gallery.rs`, plus a step in the macos-appkit workflow: load the bundle, performClick each
     sidebar button natively, check there are no JS errors and that the classes exist, take a screencapture;
   - `coverage.py`: AppKit `"gallery": gallery_kinds("appkit")` instead of None (WP-W may touch the same line);
   - `check-pages.mjs`: the click event `action` for appkit, and the tests file path per toolkit.
4. Optional: transient panels, a virtuallist on NSTableView, a datetime/object binding type (date-value, color, url).
5. Write `plans/milestones/WP-appkit-widgets.md` and run all the gates (see the lane brief).

## How to verify

- Local, with `SVELTERS_SLOT_JOBS=4`:
  - cross-check: `tools/dev/cargo-slot cargo check -p svelters-apple -p svelters-appkit --all-targets --target aarch64-apple-darwin`;
  - type-check the generated tests by copying `examples/counter-mac/tests/generated.rs` to
    `crates/svelters-appkit/examples/zz_tc_generated.rs`, running
    `cargo check -p svelters-appkit --example zz_tc_generated --target aarch64-apple-darwin`, then deleting the copy.
    Watch out: a non-ASCII sample becomes `…` from json.dumps, which Rust rejects.
- Generators, all with `--check`, as in `pnpm test:meta` (root `package.json`).
- CI loop:
  - push `ci/wp-appkit`;
  - the `appkit-metadata` job re-ingests and uploads `appkit-metadata` (copy `metadata/appkit/NS*.json` and `_enums.json`);
  - the `appkit` job uploads `appkit-probe` (the probe plus `appkit-results/generated.jsonl`).
- A new override selector (in methods, setters, slots, or write/count) needs an ingest round before `gen_backend` accepts it.

## Known failures and risks

- Expected failing: js, because the ledger is stale until the import, and macos-appkit, until the new tests are fixed
  and the probe is committed.
- Stringification: `write_through` turns list items into strings (combo-box `object-values` become strings).
- Mutual exclusion: `item-titles` (removeAllItems) and a `menu` child on a pop-up-button exclude each other.
  `remove-all-items` is marked dangerous.
- Panels: a `<panel>` inside a window page is inserted into the window by the renderer. The backend refuses that
  (transient windows are not supported yet), so panels only work at the root for now.
- No layout sizing props: a scroll-view or split-view needs a stack view or a window to size it, and a document view needs
  a `frame`.
