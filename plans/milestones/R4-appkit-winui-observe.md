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

(filled in below)
