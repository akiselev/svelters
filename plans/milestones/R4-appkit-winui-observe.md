# R4: AppKit and WinUI `observe` (plans/research/REACTIVITY.md §3.3, §3.4, §4.5, §5.4-5.6, §10 R4, §13)

Lane: `crates/svelters-{appkit,apple,winui}`, `tools/metadata/{appkit,winui}`, `metadata/rules/{appkit,winui}.yaml`, the
AppKit/WinUI observe facts, inferred layers and ledgers, `.github/workflows/{macos-appkit,windows-winui}.yml`,
`examples/counter-mac`. Branch `r4-appkit-winui`, pushed to `ci/r4`. The ABI is unchanged (v2.6 `observe`,
`prop_changed`, `SVL_M_OBSERVABLE` already existed; both backends left `observe` NULL before).

Nothing here runs on Linux: AppKit is checked for `aarch64-apple-darwin` without an SDK, WinUI for
`x86_64-pc-windows-gnu`; every test result below comes from the `macos-appkit` and `windows-winui` CI jobs.

## Mechanisms

Both backends read the merged observe facts (`metadata/observe/<tk>.json`) through their generators:

| | AppKit | WinUI |
|---|---|---|
| table | `OBSERVE: [ObserveInfo]` in `generated/tables.rs`, one row per fact on the class the fact is written on, looked up along the element's chain (`NSTextField:string-value` wins over `NSControl:string-value`) | `OBSERVE: [Observe]` parallel to `PROPS` (class, proportional) in `generated/names.rs`; `OBSERVE_CONNECT` per prop in `generated/dispatch.rs` |
| `notify` | KVO on the KVC key. One backend observer object (`SveltersObserver`, `observeValueForKeyPath:ofObject:change:context:`); `context` is a boxed (h, prop) record owned by the handle entry, freed after the registration is removed | `RegisterPropertyChangedCallback(<Prop>Property, cb)`; the `(dp, token)` registration is kept per (h, prop) |
| fallbacks | `notification`: the handle's `SveltersTarget` on the default center with the object as sender. `target_action`: the control's target/action. Both are user-counted together with `listen` (a listened `control-text-did-change` and an observed `string-value` share one registration) | `event`: the event on the prop's class, a base, or the nearest derived class (`UIElement.ActualSize` -> `FrameworkElement.SizeChanged`), with its revoker |
| delegate captures | the context record's ids; the backend through its thread-local weak reference | the ids; the backend through its thread-local weak reference (`emit_changed`) |
| threads | KVO on the main thread calls the host synchronously; from another thread the pair is queued (deduplicated) and the run loop woken; the next wake delivers it (`timing: deferred`) | DP callbacks and events run on the UI thread; `observe` checks `DispatcherQueue.HasThreadAccess` and answers -1 elsewhere |
| codes | `none`/`set_only`/no fact/runtime-found prop: `SVL_OBSERVE_UNSUPPORTED`; constant, write-only, dead handle: -1; `off` for an unobserved prop or dead handle: 0 | same |
| `SVL_M_OBSERVABLE` | on the descriptors `describe` returns (scalar and record props) when the fact is exact/partial with a mechanism the backend installs | on every prop descriptor, same rule |
| echo (§5.4) | `set_prop` marks (h, prop) while it writes; a `value_cost: proportional` prop is not reported for that write. Cheap props are reported and the host's compare drops them | same |

## Teardown per lifetime class (§5.5, decision 13.10)

| Class | AppKit | WinUI |
|---|---|---|
| OWNED / TOPLEVEL | `destroy` and the window-close cascade remove every KVO registration (`removeObserver:forKeyPath:context:`) while the entry still holds the object, then the target/action and notification observers; the entry (and our reference) is released afterwards from the graveyard | `destroy` drops the observations (unregister / revoke) before the element reference; native death and `shutdown` move them to the graveyard |
| SHARED | not minted by AppKit (our strong reference would keep it alive while observed) | not minted |
| BORROWED | weak entry. Autonotifying class with NSObject's `observationInfo`: not retained; an identity sentinel (associated object) queues the handle when the object deallocates, the next wake forgets it and reports `SVL_GONE_OWNER`; the runtime removed the KVO registration (macOS ≥ 10.13, minimum 11). Any other class: retained while KVO-observed, so `off`/`destroy` remove from a live object | none (every handle is owned) |

Minimum macOS 11 is recorded in `.github/workflows/macos-appkit.yml` (`MACOSX_DEPLOYMENT_TARGET`), `tools/dev/mac-cross`
and `docs/notes/macos-setup.md`.

## Verification rows (§4.5 at backend level)

One generated row per fact (`observe_<class>_<prop>`), through `observe` -> mechanism -> `host.prop_changed`:

1. native change (a write that is not the host op, or the fact's `verify.trigger`): announced;
2. identical write: logged (`fires` is not in the AppKit/WinUI facts yet);
3. echo: a `set_prop` write is announced for cheap props, not for proportional ones;
4. coalescing input: 10 native changes are announced at least once (more than one per change is logged: AppKit itself changes `views-need-display` around the display cycle); the host coalesces;
5. user path where the runner can synthesize it: AppKit typing through `-[NSWindow sendEvent:]` (text fields),
   `performClick:` (button state), the right-arrow key through `sendEvent:` (slider values);
6. teardown: nested on/off, nothing after `off`, destroy while observed; AppKit also checks `observationInfo` is
   NULL on the object we still hold after `off` and after `destroy` ("KVO observer removed before dealloc").

`none`/`set_only` rows assert `SVL_OBSERVE_UNSUPPORTED` (weak: strength `unsupported`). Facts of props a backend
does not expose (WinUI) or of classes no element derives from have no row and stay inferred.

Hand-written AppKit tests (`examples/counter-mac/tests/backend.rs`): typing into an NSTextField is observed and
`string-value` reads the text (acceptance); KVO from another thread hops and deduplicates; a BORROWED content view's
death is reported through the sentinel; codes and `SVL_M_OBSERVABLE`.

## Completion record

Branch `r4-appkit-winui` (pushed as `ci/r4`), based on master 9231ec3f. Not merged.

Commits: 4156088c (WinUI observe, generator, rows, verify triggers), 3ee417bf (AppKit observe, KVO observer, sentinel,
hop, rows, hand-written tests, minimum macOS 11), 996b40de (first CI round: WinUI writes cycle through the initial
value, `TextBlock.SelectedText` via SelectionChanged, burst counts logged instead of bounded), 3df9b019 (select-all is
a one-shot trigger; the borrowed KVO test covers the retained path), then the ledger import and this record.

### CI (`ci/r4`)

| Workflow | 3ee417bf | 996b40de | 3df9b019 (code head) |
|---|---|---|---|
| macos-appkit | failure 39538249180: `observe_nswindow_views_need_display` (58 announcements for 10 sets, AppKit changes it itself), `observe_borrowed_object_death` | failure 39541027676: `observe_borrowed_object_death` only | **success 39542111803**: generated 570/570 (344 observe rows), backend 17/17 (4 observe tests), counter ok |
| windows-winui | failure 39538249160: 456/495, 39 observe rows (single test value rewritten = no change; echo writes in the getter's format) | failure 39541027728: 494/495 (`observe_text_block_selected_text`: select-all cannot repeat) | **success 39542111789**: generated 495/495 (145 observe rows), lib, selftest, counter ok |
| js | failure (coverage stale) | success 39541027801 | success 39542111853 |
| linux-gtk | success | success 39541027691 | success 39542111835 |
| linux-qt | success | success 39541027688 | success 39542111918 |

The commits after 3df9b019 change only the ledgers, coverage (`tools/metadata/coverage.py` now counts the AppKit/WinUI
observe rows) and docs.

Acceptance (§10 R4), seen in the logs of the green runs: `observe_typing_into_text_field` ok ("typing 3 keys announced
string-value 3 time(s)", and `string-value` reads "abc"); `observe_text_box_selection_start`, `_selection_length`,
`_selected_text` ok (`select(1, 3)` / `select(2, 2)` on a mounted TextBox, SelectionChanged).

### Verified observe facts (ledger import of 39542111803 and 39542111789)

| Toolkit | facts | before | after: verified (native change announced) | after: weak (`unsupported`/`connect`) | no row |
|---|---|---|---|---|---|
| AppKit | 344 | 0 | 154 (exact 149, partial 5) | 190 (none 116, set_only 70, connect 4) | 0 |
| WinUI | 251 | 0 | 130 | 15 (unsupported 7, connect 8) | 106 (props the backend does not expose) |

Ledgers: appkit 570 facts / 570 tests, 0 not passing, 0 parked; winui 540 facts, 0 not passing, 0 pending, 8 parked
(pre-R4). Nothing was parked by R4.

### Facts changed and why

- `Microsoft.UI.Xaml.Controls:TextBlock:SelectedText`: `notify` -> `event: SelectionChanged` (rule `no_notify` in
  `metadata/rules/winui.yaml`). On the runner the SelectedTextProperty change callback did not fire for `SelectAll` on a
  mounted, selectable TextBlock (run 39538249160); SelectionChanged does (39541027728 onwards).
- `verify` blocks (how the generated row triggers the change) from `observe.verify` in both rules files: WinUI
  TextBox/TextBlock selection, ActualWidth/ActualHeight/ActualSize/RenderSize (resize), ActualTheme (theme), IsLoaded
  (mount), Window.Visible (activate); AppKit `NSWindow:frame` (`setFrame:display:`). Record fingerprints changed; no
  verified fact existed for them.
- No class changed otherwise: every row with a mechanism announced its native change.

### Observations recorded, not yet facts

- Identical writes: AppKit KVO re-announces an identical KVC set for 147 of 150 rows (one twice; not: NSView `hidden`,
  `wants-layer`, NSWindow `titlebar-appears-transparent`); WinUI DP callbacks re-announce for 6 of 121 (`content`,
  `is-enabled`, `name`, slider/text-box `header`, `use-layout-rounding`). A `fires` rule could record this; the host's
  compare makes it harmless.
- Bursts: 10 sets announced 19-20 times for a few AppKit props (button `is-enabled`, `border-shape`,
  `tint-prominence`, stack-view `alignment`, view `additional-safe-area-insets`) and 58 times for NSWindow
  `views-need-display`: AppKit changes them itself in between. The host coalesces per checkpoint.
- NSView is not autonotifying with NSObject's KVO state by the backend's test (`+automaticallyNotifiesObserversForKey:`
  / `-observationInfo`), so a borrowed NSView is retained while KVO-observed: the CI test exercised the retained path
  (off removes the registration from the live object, `observationInfo` NULL, then released).

### Gates

| Gate | Result |
|---|---|
| `tools/dev/cargo-slot cargo check --workspace --all-targets` | rc 0 (on the import head; code as 3df9b019) |
| `cargo check -p svelters-apple -p svelters-appkit --all-targets --target aarch64-apple-darwin` (no SDK) | rc 0 |
| `cargo check -p svelters-winui -p counter-win --tests --target x86_64-pc-windows-gnu` | rc 0 |
| `pnpm test` (incl. `test:meta`: pytest 140 passed / 1 skipped, every `--check`, `ledger.py check`) | rc 0 after the import |
| `ledger.py check` | pass: appkit 79 manual + 491 inferred verified; winui 60 + 480, 8 parked |

The generated AppKit tests and `counter-mac/tests/backend.rs` were type-checked locally by compiling a copy as a test
target of `svelters-appkit` for `aarch64-apple-darwin` (counter-mac itself needs the SDK for QuickJS).

### Open items

- **Identity sentinel path not exercised on CI**: every borrowed object of the current kinds (NSView, NSWindow
  subclasses) takes the retained path. The sentinel (death reported as `SVL_GONE_OWNER`) is only reachable for an
  autonotifying class; a test needs a borrowed handle of such a class.
- **Proportional echo suppression** is implemented in both backends but no AppKit/WinUI fact is `value_cost:
  proportional`, so no CI row covers it.
- AppKit `describe` returns descriptors (and so `SVL_M_OBSERVABLE`) for scalar and record props only; enum, object and
  list props are observable through `observe` but carry no flag.
- `fires` per fact (above) is not recorded.
- **WinUI user path through UI Automation is not generated**: the rows change props natively or run a trigger
  (`select`, `activate`); only the ButtonAutomationPeer `Invoke` existed before (selftest). A UIA row for TextBox text
  (ValuePattern) and selection (TextPattern) needs those peers in the bindings filter.
- WinUI: 106 facts have no row because the backend does not expose the prop (brushes, templates, transitions, ...).
- Real Mac / Windows checks the runner cannot do: typing with an input method and undo; slider drag (the row uses the
  arrow key); live window resize/move by the user (the row uses `setFrame:display:`); user selection in a TextBox by
  mouse and keyboard (the row uses `Select`); a KVO change from a
  background thread in a real app (the hop is tested with `setValue:forKey:` on a thread).
