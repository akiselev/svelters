# REACTIVITY: observing native properties from Svelte (GTK, Qt, AppKit, WinUI)

Status: research proposal, 2026-10-18. It answers `plans/research/NON-GUI-TYPES.md` §10 Q2 / §11
item 2: reactive getters, the per-(object, prop) subscription contract in the ABI, observability facts
in the metadata, a compile-time opt-out, and how the Rust compiler stays generic over GUI platforms
and JS runtimes.

**Status 2026-10-19: mechanics only, semantics pending §12.** The ABI v2.6 surface of §5.1 is
implemented: nullable backend `observe(h, prop, on)` (with `SVL_OBSERVE_UNSUPPORTED = -2` for a prop
without a mechanism), nullable host `prop_changed(h, prop)`, `SVL_M_OBSERVABLE`. GTK observes through
`notify::<prop>` on any GObject handle, Qt through the NOTIFY signal (no QBindable, no event filter);
AppKit and WinUI leave the member `NULL`. The host refcounts per (h, prop) and hands each notification
to `globalThis.__svelters_prop_changed(h, prop)` through the ordinary event queue: no dirty set, no
read-back or compare, no batch, no `__svelters_changed`, no set-path read-back, no sync flush. Nothing
in `@svelters/types` or the runtime subscribes yet. All §12 decisions were made on 2026-10-19 (§13). Details:
`plans/architecture/backend-abi.md` "ABI v2.6 additive".

**Status 2026-10-19 (R0, observability facts): done on branch `ci/r0`**, see
`plans/milestones/R0-observability-facts.md`. Every readable property of the GTK, Qt, AppKit and WinUI metadata
(GTK and Qt non-GUI types included) has an `observe` fact per §4.2, inferred by rules O1-O10 from the ingested
facts and two probes that are now `probe.py --section observe` (GTK 4.22.4, Qt 6.11.3: the metadata's versions),
merged into `metadata/observe/<tk>.json`, carried as `ObserveSpec` in the semantic specs, and verified on GTK and
Qt by one generated row per fact through the real `observe` path (AppKit and WinUI stay inferred until R4).
Deviations from §4 are listed in the milestone file; the main one in the measurements: **`GtkRange:fill-level`
does notify** (§3.1, §11 item 2 were a probe artifact: the research sweep's `c + 1` next to a `G_MAXDOUBLE` default
did not change the value). `GtkEntry:text-length` stays a real miss. Gates and CI (all five workflows green on `ci/r0` 0d10a073) are in the milestone's completion record.

- **Prototypes and probes** (`docs/notes/scripts/reactivity/`; the pipeline versions are
  `tools/metadata/gtk/observe_probe.{c,py}` and `tools/metadata/qt/observe_probe.{cpp,py}`):

  | File | Covers |
  |---|---|
  | `gtk_notify_probe.c`, `run_gtk_notify_probe.sh` → `gtk_notify_probe.tsv` | GTK 4.24.1 sweep: 711 props of 145 classes set natively, every `notify` recorded; scenarios (buffer edits, clamping, freeze/thaw) |
  | `qt_observe_probe.cpp` → `qt_observe_probe.tsv` | Qt 6.12.0 sweep: 226 props of 32 classes; NOTIFY, QBindable notifiers and QEvents seen per set; scenarios |
  | `host_bench/` (standalone crate + `js/`) | cost on the real host (QuickJS) + GTK backend: `getProp`, event delivery, `createSubscriber` re-runs per event vs once per burst |

- **Builds on:** `docs/notes/signals-and-slots-semantics.md` §1.3/§1.6 (delivery classes, echo),
  `docs/notes/fork-bindings.md` §6 (binding facts), `docs/notes/ownership-and-lifetimes.md`,
  `plans/architecture/backend-abi.md`, `plans/research/RUNTIME.md` (own reactivity core),
  `plans/research/ANNOTATION-AUTOMATION.md` (rules + probes + ledger).
- **Facts vs proposals.** Sections 1, 3 and 6 are verified facts, each with its source (repo file:line,
  probe output, or official docs as checked on 2026-10-18). Sections 2, 4, 5, 7-10 are proposals.
  Anything inferred rather than verified says so.

## Summary

- **Model: a reactive native read is "invalidate, then read live".** A getter read inside an effect
  subscribes to (handle, prop) through Svelte's `createSubscriber`. The native side only says "this
  (handle, prop) may have changed". The host coalesces those per checkpoint, reads the value natively,
  drops it if it equals the last known value, and hands JS one batch before effects run. Reads always
  come from the native object, so a missed or late notification can make a value stale but never wrong
  at read time.
- **Measured cost decides the delivery shape** (QuickJS release build, GTK 4.24, §6):
  - `getProp` from JS: ~4 µs.
  - Delivering one queued event to JS: ~3 µs; with a read-back: ~8 µs.
  - **One Svelte effect re-run per event: ~170-200 µs.** The host drains jobs after every event today
    (`crates/svelters-host/src/core.rs:657`), so 10k changes in one checkpoint cost 1.7-2.0 s.
    Invalidating all of them before a single drain costs 55-70 ms, which is **~30x less**.
  - So: coalesce per (handle, prop), deliver one batch, drain once.
- **Observability is uneven, and the metadata must say so per property and per platform:**

  | Toolkit | Primary mechanism | What the probes/metadata show | Fallbacks needed |
  |---|---|---|---|
  | GTK | `notify::<prop>` (no value) | 695/696 changed props notify; 237/711 also notify for an *identical* set; 99 cross-notify derived props; `GtkEntry:text-length` and `GtkRange:fill-level` never notify | read-back + host compare; `via` another object (`EntryBuffer:length`) |
  | Qt | `NOTIFY` (98/670 props in our metadata) | 31/33 NOTIFY props fire on change; of 181 no-NOTIFY props that changed, 6 are `QBindable` (notifier works), 21 announce through a `QEvent`, 154 have nothing | `QBindable` notifiers, event filter by `QEvent::Type`, non-NOTIFY signals (`textEdited`, `modificationChanged`); most of the 154 are set-only config |
  | AppKit | KVO | compliant "only if documented"; user edits on text fields go through the field editor | `NSNotification` (`NSControlTextDidChangeNotification`), target-action, KVO for programmatic changes |
  | WinUI | `RegisterPropertyChangedCallback` (no value) | 199/251 props are DPs; `ActualWidth`/`ActualHeight` are DPs that never notify | events (`SizeChanged`, `SelectionChanged`); controls do not implement `INotifyPropertyChanged` |

- **Observability classes** for every readable property, per platform:
  - `exact`: every native change is announced;
  - `partial`: some causes are missed;
  - `set_only`: only our setters change it;
  - `constant`;
  - `none`.

  These classes and a mechanism list (primary plus ordered fallbacks) are inferred by rules, confirmed
  by the probes, and verified by one generated test each into the ledger, exactly like props today.
- **ABI v2.6, additive:**
  - backend `observe(h, prop, on)` (nullable);
  - host `prop_changed(h, prop)` (no value, UI thread only);
  - a member flag `SVL_M_OBSERVABLE`.

  The host refcounts, coalesces, reads back, compares and batches. Echo suppression for observed props
  becomes "compare with the last value written or delivered", so no handler blocking is needed.
- **Opt-out:**
  - `bind:prop|nonreactive` on native elements. Stock Svelte 5.57.2 already parses directive
    modifiers on `bind:` and ignores them (verified, §8.1); attributes cannot carry modifiers.
  - Svelte's own `untrack(() => obj.prop)` for expressions; it needs no compiler support.
  - Rejected: per-import variants and new runes.
  - The compiler warns when a template reads or binds a non-observable property.
- **Compiler genericity:** keep one object-safe platform trait (today's `BackendSemantics`) and give it
  `observability()` plus type-member lookup. Add a small `RuntimeTarget` trait for import specifiers,
  capabilities and dev hooks.

  Today the emitted JS is the same on QuickJS and Node; the engine differences live in the host and
  runtime packages. So the target trait is deliberately thin.
- **Decisions needed:** 11 items, in §12. The main ones:
  - invalidate + batch;
  - values carried in the batch;
  - getters reactive by default;
  - the `|nonreactive` syntax;
  - warning vs error for partial observability.

## 1. Where things stand (verified)

| Area | Today | Source |
|---|---|---|
| NT0 wrapper getters | plain `getProp`, never reactive (`get text() { return getProp(this, 'text'); }`) | `packages/types/gtk/gtk/TextBuffer.js:340`, `packages/types/src/core.js:105` |
| Wrapper listeners | `NativeObject.on(signal, fn)` keeps `{ obj: this }` in a module map while listened, so a listened object stays reachable (no toggle refs) | `packages/types/src/core.js:141`, `:219-225` |
| `core.js` dependencies | none: plain ES2022, no Svelte import | `packages/types/src/core.js:1-12` |
| Element two-way binding | `$n.bind`: a `template_effect` writes the prop; a native signal reads it back (arg or `getProp`) and compares with the bound state | `packages/native-runtime/src/index.js:468-475` |
| One-way prop also edited natively | `$n.track`: passive mirror of the last native value, never listens | `packages/native-runtime/src/index.js:483-487`, emitted by `crates/svelters-compiler/src/lower/mod.rs:779-782` |
| Spec field `notify` | carried in the spec, used only to word the `bind_no_plan` diagnostic | `crates/svelters-compiler/src/semantics/spec.rs:205-208`, `backend.rs:303` |
| Spec coverage of `notify` | GTK 1303/1316 props; Qt 44/952; AppKit 0/331; WinUI 0/48 | `metadata/semantics/*.json` (counted) |
| Bindings declared | GTK 16, Qt 21, AppKit 3, WinUI 3 | same |
| Event delivery | FIFO queue; **one job drain after every event** in a checkpoint | `crates/svelters-host/src/core.rs:657-662` |
| `post_event` | queues and calls `backend.wake()` | `crates/svelters-host/src/lib.rs:51-58` |
| GTK echo suppression | handlers of the signals in a prop's `echo` list are blocked around `set_property` | `crates/svelters-gtk/src/backend.rs:852-871` |
| GTK `notify::<prop>` listening | only the 231 `notify::*` signals declared in the generated tables. Reflected (non-widget) kinds resolve signals with `SignalId::lookup`, which does not parse details, so `on('notify::text')` on an NT0 wrapper fails with `unknown Signal 'notify::value'` (reproduced by the bench) | `crates/svelters-gtk/src/reflect.rs:95-97`, `:128` |
| ABI tail | `call_static` is the last backend member, `frame` the last host member | `crates/svelters-abi/include/svelters_backend.h:319`, `:244` |
| Svelte `createSubscriber` | subscribes only when `effect_tracking()`; `untrack` makes that false; teardown is delayed one microtask | vendored `svelte/src/reactivity/create-subscriber.js`, `internal/client/reactivity/effects.js:191`, `runtime.js:786` |

## 2. The model (proposal)

A native property is **state owned by the toolkit**. Reading it in an effect should behave like
reading `$state`: the effect re-runs when the value changes. Three layers:

1. **JS:** a getter calls `subscribe()` (a `createSubscriber` per (handle, prop)) and then reads live with
   `getProp`.
   - Outside an effect, `subscribe()` is a no-op, so plain scripts pay nothing.
   - Inside `untrack`, nothing subscribes.
2. **Host:**
   - **Refcount.** It keeps a refcount per (handle, prop) and calls `backend.observe` on the 0→1 and
     1→0 transitions only.
   - **Coalescing.** It keeps an ordered dirty set of (handle, prop) that `prop_changed` fills between
     checkpoints.
   - **Read and compare.** At the checkpoint it reads each dirty prop natively (no JS), compares with
     the last known value, and keeps the ones that differ.
   - **One batch.** It calls JS once with all of them, then drains jobs once.
3. **Backend:**
   - installs and removes the native mechanism for (handle, prop): notify handler, NOTIFY connection,
     QBindable notifier, event filter entry, KVO observer, notification observer, DP callback token;
   - calls `prop_changed(h, prop)` from it.

   It never holds JS values and never reads the value itself.

**Why invalidate-and-read-live instead of pushing values into JS state:**
- GTK `notify` and WinUI DP callbacks carry no value (§3.1, §3.4), so someone must read anyway.
- A read at the checkpoint gives "latest value" semantics for free. That is what state wants; event
  semantics (every value) stay with `on*` signals.
- A notification that is late, coarse (`QAction::changed()` covers 14 props) or duplicated (GTK
  re-notifies identical sets) costs at most one extra comparison. It never produces a wrong read.

**Writes from JS** go through the same getter's object:
- the wrapper setter invalidates locally and synchronously, so effects that read it re-run in the
  current flush;
- the host records the read-back value as "last". The native echo of our own write is then dropped
  at the next checkpoint, and a natively normalized value (clamped) is delivered once.

## 3. Per-platform observability survey (verified)

### 3.1 GTK 4 (`notify`)

**Docs** (docs.gtk.org, `GObject::notify`, `GParamFlags`, `g_object_freeze_notify`):
- `notify` is emitted when a property is set through `g_object_set_property()`, and receiving it
  "doesn't itself guarantee that the value of the property has actually changed".
- The handler gets the `GParamSpec`, not the value.
- With `G_PARAM_EXPLICIT_NOTIFY` a set does not notify automatically; the implementation calls
  `g_object_notify` itself, "common practice is ... only when the value has actually changed".
- Freeze/thaw squashes duplicates: at most one notification per property, emitted at thaw.
- Synchronous emission is not stated in the docs. The probe's scenarios count notifies without running
  the main loop, so those notifies were emitted inside the setter or edit call.

**Metadata** (`metadata/gtk/*.json`):
- 1316 props on 298 kinds: 811 `EXPLICIT_NOTIFY` (62%), 505 not, 69 construct-only, 22 write-only.
- The non-GUI type facts (`metadata/gtk/types/*.json`, 635 props) carry **no notify field** yet.

**Probe** (`gtk_notify_probe.c`, GTK 4.24.1):
- 711 scalar props of 145 classes, each set natively to a new value and then to the same value.

| Measure | Result |
|---|---|
| props whose value changed | 696 (15 ignored the write: `GtkAdjustment:value` with the default upper 0, `GtkNotebook:page` with no pages, ...) |
| notified on change | **695/696**; the miss is `GtkRange:fill-level` (R0 correction: an artifact. Its default is `G_MAXDOUBLE`, so the sweep's `c + 1` did not change the value and nothing was due; set to a value that differs, it notifies. The R0 probe, on fresh instances: 775/793 props announced every observed change; the 18 misses are side effects, below) |
| notified more than once per set | 0 |
| non-`EXPLICIT_NOTIFY` props that re-notify an identical set | 201/201 |
| `EXPLICIT_NOTIFY` props that still re-notify an identical set | **36/510** (e.g. `GtkButton:label`, `GtkProgressBar:fraction`, `GtkEntry:placeholder-text`) |
| props whose set also notifies other props | 99 (e.g. `GtkCalendar:year` → `date`; `GtkButton:label` → `icon-name`, `child`, `css-classes`; `GtkImage:icon-name` → `storage-type`) |

**Scenarios** (changes that are not property sets):

| Scenario | notify counts |
|---|---|
| `TextBuffer` insert at cursor | `text` 1, `cursor-position` 1, `can-undo` 1 |
| `TextBuffer` 100 single-char inserts | `text` 100, `cursor-position` 100 (and 105 `changed` signals in total) |
| `TextBuffer` select all | `cursor-position` 1, `has-selection` 1 |
| `Adjustment` set 5, 5 | `value` 1 |
| `Adjustment` set 500 with upper 100 | `value` 1 (reads 100) |
| `Adjustment` 10 sets inside freeze/thaw | `value` 1 |
| `Scale.set_value` | nothing on the widget, `value` 1 on its adjustment (forwarded property) |
| `GtkEntry` insert text | entry `text` 1, **`text-length` 0**; its `EntryBuffer` `length` 1 |
| `Notebook` remove the current page | `page` 1 (changed without a set) |
| `Label.set_markup` | `label` 1, `use-markup` 1 |

The `text-length` miss matches GTK issue 3691, which was reported against GTK 4.0/4.1 and is still
reproducible on 4.24.1.

**What this means for GTK:**
- `notify` is a near-complete *invalidation* signal, but not a change signal.
- `EXPLICIT_NOTIFY` is a prior, not a fact.
- Host-side compare is what makes it exact.
- Bursts are real: typing produces two notifies per keystroke on a `TextBuffer`, and programmatic
  inserts produce one per call.

### 3.2 Qt 6 (NOTIFY, QBindable, events)

**Docs** (doc.qt.io: properties, QWidget, QEvent, QMetaProperty, QUntypedBindable,
QPropertyNotifier, bindable properties):
- **NOTIFY signals.** They take zero or one argument of the property type, and "should only be emitted
  when the property has really been changed".
- **QWidget.** Its core props (`enabled`, `visible`, `geometry`, `size`, `font`, `toolTip`) have no
  NOTIFY. `windowTitle` and `windowIcon` do: their `windowTitleChanged`/`windowIconChanged` NOTIFY
  signals are in our metaobject-derived metadata. Changes are announced by QEvents: `EnabledChange`,
  `WindowTitleChange`, `FontChange`, `PaletteChange`, `ToolTipChange`, `Show`/`Hide`, `Resize`/`Move`,
  `WindowStateChange`, `StyleChange`, `ReadOnlyChange` (5.4+), `CursorChange`, `ModifiedChange`,
  `DynamicPropertyChange`, ...
- **Event filters** "receive all events that are sent to this object", before the target.
- **Bindable properties.**
  - `QMetaProperty::isBindable()` / `bindable(QObject*)` arrived in 6.0, and `addNotifier` /
    `QPropertyNotifier` in 6.2.
  - The notifier is RAII: it stops when destroyed.
  - It fires when the value changes, including for bindable props with no NOTIFY.
  - `Qt::begin/endPropertyUpdateGroup` (6.2) defers notifications to the end of the group.
- **Threading:** bindable properties must stay on their thread.

**Metadata:**
- `metadata/qt` (670 props, 195 classes): 98 NOTIFY, 572 none, 31 without a metaobject record.
- moc metatypes on this machine (`/usr/lib/qt6/metatypes`, Qt 6.12.0):

| Module | props | NOTIFY | CONSTANT | none | BINDABLE |
|---|---:|---:|---:|---:|---:|
| QtCore | 68 | 29 | 0 | 39 | 38 |
| QtGui | 192 | 108 | 29 | 55 | 3 |
| QtWidgets | 625 | 86 | 0 | 539 | 0 |

- **Bindable without NOTIFY** in QtCore:
  - `QTimer`/`QChronoTimer` `interval`, `active`, `singleShot`, `timerType`;
  - `QSortFilterProxyModel` `filterKeyColumn`, `dynamicSortFilter`, `filterRegularExpression`;
  - `QStandardItemModel:sortRole`;
  - the animation classes;
  - `QMovie`.

  Our ingest does not record `bindable` yet.
- **Coarse NOTIFY:** 14 `QAction` props share `changed()`; `QTextEdit` `html`/`markdown` share
  `textChanged()`.

**Probe** (`qt_observe_probe.cpp`, Qt 6.12.0, offscreen):
- 226 writable scalar props of 32 classes, each set to a new value and then to the same value.

| Measure | Result |
|---|---|
| NOTIFY props that changed | 33; **31 fired**. The two that did not (`QAction:iconVisibleInMenu`, `shortcutVisibleInContextMenu`) are worth a test on xcb |
| NOTIFY re-fired on an identical set | 3 (`QTextEdit:html`, `QTextEdit:markdown`, `QPlainTextEdit:plainText`) |
| no-NOTIFY props that changed | 181 |
| … announced by a QBindable notifier | 6 (`QTimer` 3, `QSortFilterProxyModel` 2, `QStandardItemModel:sortRole`) |
| … announced by a QEvent | 21 (e.g. `readOnly` → `ReadOnlyChange`, `enabled` → `EnabledChange`, `toolTip` → `ToolTipChange`, `layoutDirection` → `LayoutDirectionChange`, `maximumSize` → `Resize`, `windowModified` → `ModifiedChange`) |
| … announced by nothing | **154** (e.g. `QLabel:text`, `QSpinBox:minimum`, `QLineEdit:placeholderText`, `QTextDocument:modified`) |

**Scenarios:**

| Scenario | Result |
|---|---|
| `QLineEdit.setText("ab")` twice | `textChanged` 1, `textEdited` 0 |
| typing "xyz" (`QTest::keyClicks`) | `textChanged` 3, `textEdited` 3 |
| `QSlider.setMaximum(50)` with value 80 | `valueChanged` 1 (clamped to 50) |
| parent `setEnabled(false)` | child gets `EnabledChange`, no set on the child |
| hidden widget `resize` | **no event until `show()`** (then `Move`, `Resize`, `Show`, ...) |
| `QTimer.setInterval` 10, 10, 20 | bindable notifier 2 |
| 3 sets inside a property update group | notifier 0 inside, 1 after |
| native cost of a NOTIFY slot that reads the value | 126 ns per `QSlider::setValue` |

**Reading of the 154:**
- **Set-only.** Most are configuration (`QLabel:text`, `QSpinBox:suffix`, `QTabWidget:tabPosition`)
  that nothing but our setter changes. They need no native observation (§4.1 `set_only`).
- **Real gaps with a non-NOTIFY signal fallback.** `QLineEdit:modified` and `QTextDocument:modified`
  have one: `textEdited`, `modificationChanged(bool)`.
- **Out of scope here.** Geometry of hidden widgets is deferred by Qt itself.

### 3.3 AppKit (KVO)

**Docs** (Apple's Key-Value Observing Programming Guide, Foundation release notes, NSControl and
Cocoa Text Architecture references):
- **Compliance.** Framework classes are KVO-compliant "only if they are documented as such".
- **Automatic notification** covers changes made through compliant accessors and KVC.
  `+automaticallyNotifiesObserversForKey:` returning NO means manual `will/didChange`.
- **Options.** `Initial` notifies before `addObserver` returns; `Prior` adds a pre-change
  notification; `Old`/`New` carry values.
- **No strong references.** `addObserver` keeps none to the observer, the observed object or the
  context.
- **macOS 10.13 relaxed unregistration.**
  - Before it, KVO threw when an autonotifying object still had observers after its `-dealloc`
    finished.
  - From 10.13, KVO cleans up remaining observers, but only if the class uses autonotifying KVO and
    does not override the private KVO state accessors.
  - The observer must still unregister before it is itself freed.
  - `NSKeyValueObservation` (block API) invalidates automatically when the observed object is released
    from macOS 11 on. This comes from the Big Sur release notes as quoted by mjtsai.com; the Apple page
    itself was not fetched.
- **Text fields.** While editing, the field editor is first responder and owns the text. "The end of
  editing triggers synchronization" with the cell. Per-change hooks:
  `NSControlTextDidChangeNotification` / `controlTextDidChange:`.
- **Continuous controls.** `isContinuous` decides whether a control sends its action during mouse
  tracking (sliders).
- **Not verified:**
  - "KVO on `stringValue` never fires per keystroke" has no Apple statement found. It is consistent with
    the field-editor sync above and with `docs/notes/apple-gui-backend.md:38`.
  - "KVO notifies synchronously on the changing thread" rests on a forum quote of the guide. It is
    treated as true and must be pinned by a CI test.

**Metadata** (`metadata/appkit`, 10 classes):
- 336 props (218 read-write, 118 read-only), with `memory` (assign/copy/strong/weak) but **no KVO
  facts**.
- The three existing bindings already use the fallbacks (`metadata/semantics/appkit.json`):
  - `NSTextField:string-value` via `control-text-did-change` (value in arg 0);
  - `NSSlider:double-value` via the `action` with read-back;
  - `NSButton:state` via the `action`.

### 3.4 WinUI 3 (DependencyProperty callbacks)

**Docs** (learn.microsoft.com: `DependencyObject.RegisterPropertyChangedCallback`,
`DependencyObject`, custom dependency properties, `TextBox`, `FrameworkElement.ActualWidth`):
- **The callback API.**
  - `RegisterPropertyChangedCallback(dp, cb)` returns an `Int64` token;
    `UnregisterPropertyChangedCallback(dp, token)` removes it.
  - The callback gets `(sender, dp)`, no value.
  - It is invoked "when the value of the specified property changes".
  - For custom-DP metadata callbacks the docs add that strings notify only on effective change, while
    enum/struct values "may be invoked even if the value did not change" because of boxing. That this
    also applies to `RegisterPropertyChangedCallback` is inferred.
- **UI thread only.** DependencyObjects are UI-thread-affine, and every API except `DispatcherQueue`
  throws from another thread, so register and unregister both run on the UI thread.
- **Plain properties need events.** `TextBox.SelectionStart`/`SelectionLength`/`SelectedText` are plain
  properties, covered by `SelectionChanged`. Text changes are also covered by `TextChanging`/
  `TextChanged`.
- **DPs that do not notify.** `ActualWidth`/`ActualHeight` have DP identifiers but "do not raise
  property change notifications"; use `SizeChanged`.
- **INotifyPropertyChanged.** The docs point to it for data sources, not controls. "Controls do not
  implement it" is inferred from its absence on the control pages.

**Metadata** (`metadata/winui`, 17 classes):
- 251 props: **199 DP (79%)**, 52 not.
- The non-DP ones include `TextBox.SelectionStart`/`SelectionLength`/`SelectedText`, `Window.Title`,
  `FrameworkElement.IsLoaded`/`Parent`, `UIElement.ActualSize`/`ActualOffset`/`DesiredSize`, and the
  Composition-backed `Translation`/`Rotation`/`Scale`.
- `ActualWidth`/`ActualHeight` are marked `dependency_property: true`, so the metadata alone would
  wrongly call them observable.

### 3.5 Summary: mechanism families per platform

| Family | GTK | Qt | AppKit | WinUI |
|---|---|---|---|---|
| property notification | `notify::p` (no value) | NOTIFY signal (0-1 arg) | KVO (`New` gives value) | DP callback (no value) |
| framework-level observable | — | `QBindable::addNotifier` | — | — |
| other signal / notification | `changed`, `value-changed`, sub-object (`EntryBuffer:length`) | `textEdited`, `modificationChanged`, `toggled` | `NSNotification`, target-action, delegate | routed/CLR events (`SelectionChanged`, `SizeChanged`, `Loaded`) |
| event filter | (controllers; not needed for props) | `QEvent::*Change`, `Resize`/`Move`, `Show`/`Hide` | — | — |
| coalescing primitive | `freeze/thaw_notify` | property update groups | `willChange`/`didChange` pairs | — |
| identical-set noise | common (237/711) | rare (3/37) | depends on class | boxing for enum/struct |
| thread of delivery | the setter's thread | the emitter's thread (AutoConnection) | the changing thread | UI thread |

## 4. Observability facts (proposal)

### 4.1 Classes

| Class | Meaning | Reactive read | `bind:` |
|---|---|---|---|
| `constant` | never changes after construction (construct-only, Qt `CONSTANT`) | plain read, no subscription | rejected |
| `set_only` | only our setters change it (configuration props) | no native observation; the wrapper setter invalidates locally | two-way is pointless: compile error, use an attribute |
| `exact` | every native change is announced by the listed mechanisms (verified) | subscribe | allowed |
| `partial` | some causes are announced; `missed` lists the others | subscribe; dev warning at first subscription | allowed with a compile warning naming the missed causes |
| `none` | native changes are not announced | snapshot read | compile error (as `bind_no_plan` today) |

Classes are **per platform**. `NSTextField:stringValue` is `exact` only because its mechanism list has
KVO plus the notification. The same element prop can be `exact` on GTK and `partial` on AppKit, and
the portability check (§8.4) reports that.

### 4.2 Metadata schema (in the ingested/inferred/manual layers)

One optional `observe` record per property, in the same layers and with the same strict-merge rules
as the rest of `metadata/<tk>/*.json` and the non-GUI types facts:

```yaml
observe:
  class: exact                    # constant | set_only | exact | partial | none
  via:                            # ordered; the backend installs every entry it supports
    - { mechanism: notify }                                         # GTK notify::<prop>, Qt NOTIFY, KVO key, DP callback
    - { mechanism: bindable }                                       # Qt QUntypedBindable::addNotifier
    - { mechanism: signal, signal: text-edited }                    # another signal of the same object
    - { mechanism: event, events: [EnabledChange] }                 # Qt event filter
    - { mechanism: notification, name: NSControlTextDidChangeNotification }
    - { mechanism: target_action }                                  # AppKit NSControl action (continuous)
    - { mechanism: event, events: [SizeChanged] }                   # WinUI routed/CLR event
    - { mechanism: via, object: buffer, prop: length }              # observe a sub-object's prop (GtkEntry text-length)
  missed: [user_edit]             # partial only: causes not covered (user_edit | layout | parent | wm | async)
  fires: on_change                # on_change | on_set (re-notifies identical sets) | coarse (shared notifier)
  derived_from: [year, month, day]  # props whose set also announces this one (probe cross-notify)
  value_cost: cheap               # cheap | proportional (whole-text props): the host skips read+compare
  timing: sync                    # sync | deferred (Qt Resize on hidden widgets, KVO from a worker thread)
  origin: probe                   # gir | metaobject | moc | winmd | sdk | rule | probe | manual
  provenance: "O1 + gtk_notify_probe 4.24.1: notify on change, re-notify on same"
```

Fact id: `svelters-api:v1:type:<tk>:<Ns>:<Type>:prop:<name>:observe`. The ledger keys results by
this id and fingerprint, as it does for props.

*As built (R0):* the record is validated by `tools/metadata/common/observe.py` and kept out of the kind records:
inferred `metadata/inferred/<tk>/observe/<Ns>.<Type>.yaml` < manual `metadata/overrides/<tk>/observe/...` merged
into `metadata/observe/<tk>.json` (the ingested layer is the raw input of the rules, never an `observe` record), so an
observe fact never changes a verified property fact's fingerprint. `origin` is the per-entry `provenance` of the
other facts; `missed` gains `programmatic` and `cross_prop` (a side effect of another set the mechanisms miss); a
`verify` block (instance type and check) tells the generators how to test the fact; `notify` entries name their
signal (`notify::text`, `textChanged(QString)`) and a shared Qt NOTIFY is `coarse: true`.

### 4.3 What the semantics spec carries

`PropertySpec` (`crates/svelters-compiler/src/semantics/spec.rs`) gains one optional field. Older
specs load unchanged, as with `methods`.

```rust
#[serde(default)]
pub observe: Option<ObserveSpec>,

pub struct ObserveSpec {
    pub class: ObsClass,          // Constant | SetOnly | Exact | Partial | None
    #[serde(default)] pub missed: Vec<String>,
    #[serde(default)] pub coarse: bool,
    #[serde(default)] pub value_cost: ValueCost,   // Cheap | Proportional
    pub mechanism: String,        // "notify", "notify+notification:control-text-did-change", ... (diagnostics/docs only)
}
```

- The compiler needs the class, the misses and the cost. The mechanism string is for messages.
- The mechanism details go to the backend tables, generated from the same metadata by
  `gen_backend.py` / `gen_cpp.py` / the AppKit and WinUI generators, as the `echo` lists do today.
- Non-GUI types get the same field in `metadata/<tk>/types/*.json`. The types generator
  (`tools/metadata/gtk/gen_types.py`) chooses between a reactive getter and a plain one from it (§7).

### 4.4 Inference rules (extend the A-rules of ANNOTATION-AUTOMATION.md §2)

| Id | Rule | Toolkits | Expected yield |
|---|---|---|---|
| **O1** | readable prop with a ParamSpec → `via: [notify]`. `fires: on_set` unless `EXPLICIT_NOTIFY`, then the probe decides. `class: exact` if the probe saw notify on change, `none` if it saw a change with no notify | GTK | 695/696 props of the sweep |
| **O2** | `CONSTRUCT_ONLY` / Qt `CONSTANT` → `constant` | GTK, Qt | GTK 69, QtGui 29 |
| **O3** | Qt `NOTIFY` → `notify`, `coarse` when the signal is the NOTIFY of more than one prop; `BINDABLE` (moc JSON) → `bindable` | Qt | 98 + bindables |
| **O4** | Qt event table: a hand-written map of about 25 entries from QWidget prop to `QEvent::Type`s (`enabled`→`EnabledChange`, `geometry`/`size`/`width`/`height`/`pos`→`Resize`/`Move`, `visible`→`Show`/`Hide`/`ShowToParent`/`HideToParent`, `font`, `palette`, `toolTip`, `windowState`, `readOnly`, `windowModified`, `cursor`, `layoutDirection`, `styleSheet`, `locale`, `acceptDrops`, `mouseTracking`, `tabletTracking`, `isActiveWindow`) | Qt | the 21 the probe saw, plus QWidget geometry |
| **O5** | name-paired signals: a signal `<prop>Changed`, `<prop>Edited`, `modificationChanged` for `modified`, WinUI `<Prop>Changed` events and `SelectionChanged` for `Selection*` → `signal`/`event` | Qt, WinUI | the probe's `modified` cases, TextBox selection |
| **O6** | WinUI DP → `notify` (DP callback), minus a deny list (`ActualWidth`/`ActualHeight` → `event: SizeChanged`; `ActualTheme` → `ActualThemeChanged`) | WinUI | 199 DPs, 2-3 overrides |
| **O7** | AppKit: `notify` (KVO) as `partial` with `missed: [user_edit]` for `NSControl` value props, completed to `exact` by a notification/target-action entry from a short list (`NSTextField`/`NSSecureTextField` string/object value, `NSSlider`/`NSStepper` values, `NSButton` state, window frame via `NSWindowDidResizeNotification`) | AppKit | the 3 bindings that exist |
| **O8** | `set_only`: writable, not `USER` (Qt), no notify fallback found, and the probe found no native trigger (no action, input or parent change moved it). Users can override | all | most of Qt's 154 |
| **O9** | forwarded props (`via` in the GTK tables, `GtkRange:value` → adjustment) observe the target object and re-target when the forwarding prop changes | GTK | existing `F_VIA` props |
| **O10** | `derived_from` from the probe's cross-notify rows; `value_cost: proportional` for text-content props of text models (`TextBuffer:text`, `QTextDocument` contents) | GTK, Qt | 99 GTK props |

Judgement stays manual for:
- the `missed` causes when no synthesizer exists (WM-driven window state, layout);
- `set_only` claims that a probe can't refute.

### 4.5 How facts are verified

The generators emit one test per `observe` fact, through the ledger (`tools/metadata/ledger.py`):

1. **Native set path.** The test observes (h, prop), changes the prop natively (not through our
   `set_prop`), runs a checkpoint, and asserts exactly one delivery with the read-back value.
2. **Identical set.** Set the same value natively: zero deliveries after host compare.
3. **Echo.** Set through the host op: zero deliveries. Set a value the toolkit normalizes: one
   delivery with the normalized value.
4. **Coalescing.** 10 native changes between two checkpoints: one delivery.
5. **User path,** where a synthesizer exists:
   - Qt: `QTest::keyClicks` / mouse;
   - GTK: `gtk_editable_insert_text`, controller emits;
   - WinUI: UIA on the CI runner;
   - AppKit: `-[NSWindow sendEvent:]` on the CI runner.

   The test asserts the fallback mechanism fires.
6. **Teardown:**
   - unobserve, then change: no delivery;
   - destroy the object while observed: no crash, the observation ends;
   - KVO: the observer is removed before the observed object deallocates.
7. **Class claims:**
   - `none` is asserted too: a native change with no delivery. If it starts delivering, the toolkit
     improved and the fact must be updated.
   - `set_only` has no runnable negative test. It is recorded `unverified` unless a probe tried the
     triggers.

Inferred facts that pass are promoted, and failing ones are parked, per ANNOTATION-AUTOMATION §5.3.
The two probes in `docs/notes/scripts/reactivity/` are the seed of the `observe` section of
`tools/metadata/<tk>/probe.py`. For WinUI and AppKit the same harness runs inside the backend's test
binary on the CI runners (ANNOTATION-AUTOMATION §3.3).

### 4.6 Expected coverage

| Toolkit | Exact or constant after rules + probe | Needs judgement | Source of the estimate |
|---|---|---|---|
| GTK | ~1290/1316 widget props; the 635 type props need ingest of `notify` first | a handful (`text-length`, `fill-level`, WM-driven window props) | sweep: 695/696, 2 known misses |
| Qt | NOTIFY 98 + bindable + ~25 event-mapped; most of the rest `set_only` | `set_only` vs `none` on read-only computed props (`sizeHint`, `hasSelectedText`) | sweep: 154 announced by nothing, mostly configuration |
| AppKit | the props behind the O7 list, plus `set_only` configuration | everything else, until the CI probe runs | 3 annotated bindings today |
| WinUI | 199 DPs minus the deny list; `set_only` for configuration non-DPs | non-DP read-only props (`DesiredSize`, `ActualSize`) | metadata counts |

*R0 actual* (`metadata/coverage/observe.json`): GTK 1295 facts (exact 1127, partial 14, none 3, constant 149,
set_only 2), 848 verified by a native change, 309 by `connect`/`refused` only, 138 without a test; of the 190 read-only
props the probe never saw change, 40 are `constant` by review pattern, 9 decided with a trigger the generated test
runs (8 exact, `GtkEventController:widget` none), 141 deliberately left without a fact in 9 named groups (display
server, list views, window/input, ...: no headless trigger); Qt 708 (exact 169, partial 10, none 77, set_only 452), 40 + 56 verified, the rest wait
for the R3 mechanisms or are not exposed by the backend; AppKit 344 and WinUI 251, inferred only (R4). Details in
`plans/milestones/R0-observability-facts.md`.

## 5. ABI v2.6 (proposal; additive on v3, after NT0's v2.5)

### 5.1 Members

```c
/* backend, appended after call_static; NULL = the backend cannot observe (every observe fails) */
int (*observe)(void *self, svl_handle h, svl_id prop, bool on);

/* host, appended after frame; UI thread only */
void (*prop_changed)(void *self, svl_handle h, svl_id prop);

/* member flag on SVL_PROP descriptors (describe): the backend can observe it */
SVL_M_OBSERVABLE = 64
```

- `observe(h, prop, true)` installs every mechanism of the prop's `via` list that the backend
  supports, and `false` removes them. It sees only transitions: the host refcounts.
- It fails (< 0, `last_error`) for a prop that is `none`, `constant` or `set_only`, for a dead handle,
  and on a backend without the member.
- **Reflected kinds:** GTK reflected props are all `notify`-observable. Qt's metaobject fallback uses
  `hasNotifySignal() || isBindable()`.
- `prop_changed(h, prop)` carries ids only, never a value and never JS. A coarse mechanism (Qt
  `QAction::changed()`) calls it once per prop it covers. A backend may call it for props nobody
  observes (cross-notify); the host ignores those.

### 5.2 Host contract

| Rule | Detail |
|---|---|
| refcount | per (h, prop); 0→1 calls `observe(on)`, 1→0 calls `observe(off)`. Several JS layers (wrapper getters, element bindings) share one native connection |
| coalescing | `prop_changed` inserts into an ordered dirty set (insertion order, deduplicated) and calls `backend.wake()` on the first insert of a turn |
| read | at the checkpoint the host calls `get_prop` natively for each dirty (h, prop). `value_cost: proportional` props are not read; they are delivered as invalidation only |
| compare | with `last[(h, prop)]`, the last value delivered **or** read back after our own `set_prop`. Equal → dropped. Handles compare by id, strings by content, records and lists deeply, f64 by bits (NaN equals NaN). `BOXED`/VALUE results always count as changed: each read mints a new handle |
| batch | one JS call per checkpoint, `globalThis.__svelters_changed([h, prop, value, h, prop, value, ...])` (`value` is `undefined` for proportional props), **before** the queued signal events, and with no job drain between the batch and the first event |
| set path | after a successful `set_prop` on an observed (h, prop), the host reads it back once and stores it as `last`. Our echo is then dropped, and a normalized value is delivered once |
| sync handlers | before `call_sync` runs JS, the host flushes the dirty set into an invalidation batch (no drain), so `$derived` values read inside the sync handler are fresh. This extends the rule in signals-and-slots §1.5.4 |
| death | `on_destroyed(h)` drops all observations and `last` entries of `h`; the backend has already removed its mechanisms (§5.5). Dirty entries of a dead handle are dropped |
| scripted mode | `changed #h prop <json>` lines in fixture scripts feed the dirty set, so the TestHost and the log backend replay observation |

**Op log:** `observe #h prop on|off` and `changed #h prop <json>`. **Host ops:**
`host.observe(h, prop, on)`.

### 5.3 Timing relative to Svelte's scheduler

```text
native change ──prop_changed──▶ dirty set ──wake──▶ checkpoint:
   1a. read + compare + build batch         (native only)
   1b. __svelters_changed(batch)            (subscribers' update(): version bumps, no flush yet)
   1c. queued signal events, as today       (handlers see the new state)
   2.  drain: Svelte flushes once           (effects re-run, read live, emit ops)
   3.  ops applied; next wake if work remains
```

- **Latency:** one main-loop turn after the native change, the same as `on*` handlers.
- **JS writes then reads in the same turn:** the getter reads live, so it is consistent.
- **Native changes while JS runs** (a host op emits notify synchronously) are only marked dirty. They
  are delivered at the next checkpoint, never re-entrantly.
- **With the own reactivity core** (RUNTIME.md), 1b becomes an explicit batch. With stock Svelte the
  microtask flush already gives the same effect, because the host does not drain between 1b and the
  first event.

### 5.4 Echo and `bind:`

- **Host compare replaces handler blocking for observed props.** GTK's `echo` blocking
  (`backend.rs:852`) and Qt's suppress flag stay for **signals** (`on*` listeners and signal-based
  bindings).
- **Proportional props** (no compare) use backend-side suppression instead: the backend does not call
  `prop_changed(h, p)` for the prop it is currently setting through `set_prop`. Cross-notified props
  still go through.
- **`bind:` on an `exact` prop can be lowered onto observation** (§8.3): one mechanism instead of two.
  - Signal-based bindings stay where a user-intent signal is the better source (`textEdited`, AppKit
    notifications) or where the value comes in the signal args.

### 5.5 Ownership and teardown

- **No cycles.**
  - The chain is native mechanism → (h, prop) ids → host dirty set → JS table keyed by handle →
    `createSubscriber` `update` closures.
  - The JS table must **not** hold wrappers. This differs from `NativeObject.on`'s `listening` map
    (`core.js:225`), which holds `{ obj }` on purpose.
  - The subscribing effects hold the wrapper. When they are destroyed, `createSubscriber`'s delayed
    teardown calls `host.observe(off)`. When the wrapper is collected, the SHARED handle is released
    and the backend drops whatever is left.
- **Owner per layer:**

| Layer | Owns | Released by |
|---|---|---|
| JS | one subscriber per (h, prop), in the reactive scope of the effects that read it | the last effect's teardown (+1 microtask) |
| host | refcount, dirty set, `last` values | refcount 0, or `on_destroyed` / `destroy(h)` |
| backend | the native connection(s) per (h, prop) | `observe(off)`, `destroy(h)`, or native death |

- **Per toolkit:**
  - **GTK.** `g_signal_connect` on `notify::<prop>` (detailed; needs the `reflect.rs` fix in §11). The
    handler id is kept in the handle's entry next to `signals` and disconnected in `release_entry`
    (`backend.rs:307`).
    - SHARED holds a strong ref, so the object cannot finalize while observed.
    - BORROWED is weak; the weak notify already runs the detach path.
    - `via` props also observe the forwarding prop, to re-target.
  - **Qt.**
    - NOTIFY: `QMetaObject::connect` to the backend's existing relay object, with (h, prop) in the slot
      data.
    - QBindable: a `QPropertyNotifier` in a per-handle map, destroyed on `observe(off)` or `destroyed`.
      Whether destroying a notifier after its object is safe must be pinned by a test before we rely on
      it.
    - Events: one event filter per observed widget, with a `QEvent::Type` → prop ids table, removed
      with the last event-based observation.
    - Connections die with the object.
  - **AppKit.** One backend observer object (`observeValueForKeyPath:ofObject:change:context:`), and
    `context` = a per-registration record (h, prop).
    - **OWNED/TOPLEVEL:** `removeObserver` runs in `destroy`, before the object can deallocate.
    - **SHARED:** our strong ref keeps it alive while observed.
    - **BORROWED:** remove from the identity sentinel's dealloc hook, relying on the 10.13 relaxation
      for autonotifying classes. For classes the metadata marks manual-notifying, retain the object for
      the life of the observation.
    - Notification observers are removed with `removeObserver:name:object:`.
  - **WinUI.**
    - `observe(on)` registers the callback and keeps `(dp, token)` per (h, prop). The delegate captures
      a weak backend reference and the ids.
    - `observe(off)` and `destroy` unregister **before** our reference is released. Both run on the UI
      thread, where all host ops already run (NON-GUI-TYPES §4.1).
    - Event-based fallbacks keep their `event_token`.

### 5.6 Threads

`prop_changed` is UI-thread only. Each backend hops before calling it:
- **GTK:** if the default main context is not owned by this thread, `g_main_context_invoke`.
- **Qt:** connections to the relay in the GUI thread are queued automatically (AutoConnection).
- **AppKit:** `NSThread.isMainThread`, else `dispatch_async` to the main queue.
- **WinUI:** DP callbacks are UI-thread by the DependencyObject rule.

A hop marks the fact `timing: deferred`.

## 6. Cost (measured)

**Setup:**
- `docs/notes/scripts/reactivity/host_bench`: a standalone crate on `svelters-host` + `svelters-gtk`,
  release build, QuickJS-NG via rquickjs 0.14, GTK 4.24.1 under Xvfb on WSL2. Three runs each;
  ranges shown.
- N = 10,000 native changes are queued while the module loads (`gtk_adjustment_set_value` through
  `host.call`). The harness then times one checkpoint that delivers them.
- The scenarios listen to `value-changed`, because `notify::value` cannot be listened to on a
  reflected kind today (§11). The delivery path is identical.

| Measurement | Result |
|---|---|
| `host.getProp(adj, 'value')` from JS | 3.7-6.9 µs (typ. 4.2) |
| `host.call(adj, 'set_value', [i])` (GI call + emission), not listened / listened | 8.4-13.7 / 10-30 µs |
| delivery per event, empty handler | **2.7-3.5 µs** (27-35 ms per 10k) |
| delivery per event + `getProp` read-back | **7.5-9.1 µs** (75-91 ms per 10k) |
| delivery + `createSubscriber` update + render effect re-run (reads `getProp`) **per event** | **172-198 µs** (1.72-1.98 s per 10k; 10,001 effect runs) |
| same 10k events, **one** update for the burst (what coalescing delivers) | **5.5-7.0 µs** per event (55-70 ms per 10k; 2 effect runs) |
| one event in a checkpoint, end to end | 0.1 ms |
| Qt native: NOTIFY slot reading the value | 0.13 µs per set (`qt_observe_probe`) |

**What follows:**
1. **The Svelte flush per event dominates** (≈170 µs vs ≈3-8 µs of host work). Batching invalidations
   before a single drain is worth ~30x on bursts, so it goes into the ABI contract (§5.2), not into a
   later optimisation. RUNTIME.md's measurement of Svelte's flush on QuickJS (64 µs per write+flush,
   77x on idle trees) points the same way.
2. **Host coalescing per (h, prop)** removes the remaining per-event delivery cost (3-9 µs) for bursts:
   - `TextBuffer` inserts: 2 notifies per call (§3.1);
   - echoes of JS writes;
   - Adjustment `configure`.
3. **Typical interaction is cheap anyway.** A slider drag produces about one change per frame, so
   ≈0.2 ms per frame including the effect.
4. **Carrying the value saves the read-back crossing** (≈4 µs) only if the JS getter trusts a cached
   value. That is an optional optimisation (§7), not needed for correctness.
5. **Proportional props are the real risk.** Reading a large `TextBuffer:text` on every keystroke is
   O(n) per checkpoint. The fact `value_cost: proportional` makes the host skip the read, and the docs
   should steer users to `length`-style props.

## 7. JS runtime (proposal)

- **`@svelters/types` core** keeps zero dependencies.
  - It gains `observed(self, prop)`, which calls a tracker installed by the runtime
    (`installTracker(t)`).
  - The tracker is `@svelters/native-runtime`'s, built on `createSubscriber`. Later it is the own core's
    port of it (RUNTIME.md phase 3).
  - Without a tracker (plain scripts, Node tests without Svelte), `observed` is a plain read.
- **Generated getters** follow the class:
  - `exact`/`partial` → `get text() { return observed(this, 'text'); }`;
  - `constant`/`none` → `getProp` as today;
  - `set_only` → `getProp` plus a local `invalidate(this, prop)` in the setter.

  Every setter invalidates locally, so a JS write re-runs readers in the same flush.
- **Subscriber table:**
  - `Map<handle, Map<prop, {subscribe, update}>>`, created on first read;
  - `start` = `host.observe(h, prop, true)`, `stop` = `host.observe(h, prop, false)` + delete;
  - `__svelters_changed(batch)` looks entries up and calls `update()`.
- **Element nodes** (`packages/native-runtime/src/index.js`):
  - `$n.observe(node, prop, fn)` for bindings;
  - `node.prop(name)` on `bind:this` nodes for reactive reads, while `node.getProp(name)` stays a
    snapshot (methods are snapshots, properties of the wrapper are reactive).
- **Optional (R5):** a per-(h, prop) value cache filled from the batch, used by `observed` while the
  prop is subscribed and `exact`. It is invalidated by our own writes. It saves one `getProp` per
  re-run. Not needed for correctness; measure first.

## 8. Compiler: `|nonreactive`, lowering, diagnostics (proposal)

### 8.1 Syntax options

Verified with the vendored `svelte@5.57.2` compiler:
- `<input bind:value|nonreactive={x} />` parses into `BindDirective.modifiers = ["nonreactive"]` and
  compiles with no error and no warning. The modifier is silently ignored.
- `<div onclick|once={...}>` fails with `attribute_invalid_name`.

The Rust port keeps modifiers on every directive (`crates/svelters-compiler/src/svelte/parse/state/element.rs:720-722`).

| Option | Where it applies | Compile-time known | Tooling (prettier, svelte-check, stock compiler) | Verdict |
|---|---|---|---|---|
| `bind:prop\|nonreactive={x}` | element bindings | yes (AST) | parses everywhere; stock Svelte ignores it | **adopt** |
| attribute modifier `prop\|nonreactive={x}` | attributes | — | invalid attribute name in Svelte 5 | reject |
| `untrack(() => obj.prop)` | any expression | yes if wanted (import from `svelte`), but not needed: `createSubscriber` respects it | standard | **adopt** (document) |
| `obj.peek('prop')` / `node.getProp('prop')` | wrapper or node reads | yes (method call) | typed via the generated `.d.ts` | adopt as the idiom for snapshots |
| new rune `$native.peek(expr)` | expressions | yes | `$native` is a store subscription to a variable `native` in stock tooling | reject |
| per-import variant `svelters:gtk/gtk?static` | whole module | yes | two class objects per native class breaks identity (`wrapObject` keeps one wrapper per handle) and `instanceof` | reject |
| per-object option `new TextBuffer(props, { reactive: false })` | one object | no | fine | possible later; not needed |
| component option `<svelte:options>` | whole component | yes | unknown options are an error in stock Svelte | reject |

### 8.2 Semantics of `|nonreactive`

- **On a two-way binding** (`bind:text|nonreactive={x}`):
  - state → native writes as usual;
  - native → state happens **once**: the initial read after creation (so `x` starts in sync);
  - then never: no observation, no change signal connected, no Track mirror.

  The use case is a field the app owns and only reads back at explicit moments, through
  `node.getProp` or a submit handler.
- **On a read-only binding** (`bind:has-selection|nonreactive={sel}`): a one-time snapshot after the
  first layout pass. This is the read-only rule from fork-bindings §6.4.
- **Elsewhere** (`bind:this`, component bindings, the DOM conformance backend): error
  `bind_modifier_invalid`.
- **Other modifiers** on a native `bind:` are an error. Stock Svelte would silently ignore them, and
  we must not.
- **Future siblings** in the same slot, not proposed now: `|lazy` (subscribe to the commit signal:
  `activate`, `editing-done`, focus-out; Vue's `v-model.lazy`). Recorded as a decision (§12).

### 8.3 What the compiler emits

| Source | Today | Proposed |
|---|---|---|
| `bind:text={x}` on an `exact` prop | `$n.bind(node, "text", "changed", -1, get, set)` (signal + read-back) | `$n.bind_prop(node, "text", get, set)`: the write effect plus `$n.observe`; signal plans stay where the spec's binding names a user-intent signal |
| `bind:has-selection={sel}` (read-only, observable) | `bind_no_plan` unless a `bindings` entry exists | `$n.bind_readonly(node, "has-selection", set)`: planned from `observe`, no hand annotation (rule B1 extended to read-only props) |
| `bind:text\|nonreactive={x}` | — (modifier ignored) | `$n.bind_once(node, "text", get, set)` |
| `text={x}` on a prop the native side edits | `$n.set` effect + `$n.track` | unchanged |
| `{buffer.text}` with `buffer` a wrapper | `getProp` (non-reactive) | unchanged code. The getter is reactive at runtime; the compiler only adds diagnostics (§8.4) |

### 8.4 Diagnostics

| Code | Severity | When |
|---|---|---|
| `bind_not_observable` | error | `bind:` on a `none`/`constant` prop (replaces the `notify` wording of `bind_no_plan`, `backend.rs:303`) |
| `bind_set_only` | error | two-way `bind:` on a `set_only` prop: "nothing but your code changes it; use `prop={x}`" |
| `bind_partially_observed` | warning | `bind:` on a `partial` prop; names the `missed` causes per platform |
| `native_read_not_observable` | warning (best effort) | a template expression, `$derived` or `$effect` reads `obj.prop` where `obj` is statically a native type and `prop` is `none`/`partial`. Static typing comes from scope analysis: `const b = new TextBuffer(...)` with `TextBuffer` imported from `svelters:<tk>/<ns>`, `bind:this` targets of known tags, and generated-class parameters. Unknown receivers are silent |
| `bind_modifier_invalid` | error | unknown modifier, or `\|nonreactive` where it has no meaning |
| `nonreactive_redundant` | warning | `\|nonreactive` on a `set_only`/`constant` prop |
| `observability_differs` | warning | `svelters-compile --backend gtk,qt,appkit,winui` (portability mode): the same binding or read is `exact` on one platform and `partial`/`none` on another |

The generated `.d.ts` mirrors the classes:
- `/** @observable partial: user edits on AppKit */` on getters;
- typings mark `none` getters with an `@remarks Snapshot: not reactive` line.

Editors then show the same information svelte-check cannot compute.

## 9. Trait split: platform semantics vs runtime target (proposal)

### 9.1 What exists

- `BackendSemantics` (`crates/svelters-compiler/src/semantics/mod.rs:241`) is already the platform
  trait. It covers elements, lifetime, props, events, bindings, slots, child attributes, styles,
  methods, text nodes and HTML syntax.
- `SpecBackend` implements it from the generated JSON (`semantics/backend.rs`).
- `Rewriter` (`codegen/component.rs:23`) abstracts the script/expression transform over Svelte's
  scope analysis.
- The runtime side is hard-coded: `generate()` writes
  `import * as $ from 'svelte/internal/client'; import * as $n from '@svelters/native-runtime';`
  (`codegen/component.rs:62`), and the `$n.*` names are literals in the emitter.

### 9.2 Proposal

Keep one object-safe platform trait. It is loaded at run time from JSON, and one `svelters-compile`
binary serves every platform, so generics would buy nothing. Group it, extend it, and add a thin
runtime trait.

```rust
/// What a template means on one GUI platform (gtk, qt, appkit, winui; later uikit, android; dom for
/// conformance). Generated specs answer it; hand-written impls may override parts.
pub trait PlatformSemantics {
    fn id(&self) -> &str;
    // elements:  resolve_element, lifetime, is_a, text_content, text_node, html_syntax, row_outlet
    // props:     property, writable_property, coerce_static
    fn observability(&self, el: &ElementRef, prop: &PropertySpec) -> Observability;   // NEW
    // events:    event (phases, sync)
    // bindings:  plan_binding (now consults observability; BindingPlan.source = Signal{..} | Observe)
    fn bind_modifiers(&self) -> &[&'static str] { &["nonreactive"] }                  // NEW
    // slots:     place_child, child_attr, coerce_child_attr
    // styles:    style_property, style_declaration, is_setter_property
    // methods:   method
    fn native_type(&self, module: &str, export: &str) -> Option<TypeRef> { None }      // NEW: svelters:<tk>/<ns> classes
    fn type_property(&self, ty: &TypeRef, prop: &str) -> Option<(&PropertySpec, Observability)> { None } // NEW
}

pub struct Observability { pub class: ObsClass, pub missed: Vec<String>, pub coarse: bool, pub value_cost: ValueCost, pub mechanism: String }

/// What the emitted module runs on: a JS engine plus the host that provides `__svelters_host`.
pub trait RuntimeTarget {
    fn id(&self) -> &str;                                   // "quickjs-host", "node-vite", "node-testhost"
    fn runtime_imports(&self) -> RuntimeImports;            // core (svelte/internal/client today, the own core later), native runtime
    fn specifier<'a>(&self, s: &'a str) -> Cow<'a, str>;    // `svelters:gtk/gio` kept (QuickJS built-in modules) or mapped to a package path
    fn caps(&self) -> Caps;                                 // weak_refs, finalization_registry, symbol_dispose, top_level_await, intl
    fn dev(&self) -> DevHooks;                              // hmr: vite | host_reload | none; source maps: file | inline | in-memory
}

pub struct CompileOptions<'b> { pub platform: &'b dyn PlatformSemantics, pub target: &'b dyn RuntimeTarget, /* filename, dev, css_scope */ }
```

| Concern | Platform (`PlatformSemantics`) | Runtime target (`RuntimeTarget`) |
|---|---|---|
| element tags, kinds, lifetime, slots, child attrs | ✓ | |
| props, coercion, styles (setter/child-attr/css dialect, units) | ✓ | |
| events, phases, sync delivery | ✓ | |
| bindings, echo strategy, **observability classes** | ✓ | |
| non-GUI type members (`svelters:<tk>/<ns>`) for diagnostics | ✓ | |
| import specifiers of the runtime and of `svelters:*` modules | | ✓ |
| engine capabilities that change emitted code (`using`, `Symbol.dispose`, `Intl`) | | ✓ |
| dev hooks: HMR shape, source-map delivery | | ✓ |
| how host ops reach native (QuickJS native global vs N-API vs TestHost) | neither: host + `@svelters/native-runtime` | |
| microtask drain policy around batches | neither: host (§9.5) | |

### 9.3 How observability flows

```text
metadata/<tk>/*.json (+ inferred + overrides) ── observe facts ──┬─▶ gen_semantics.py ─▶ metadata/semantics/<tk>.json ─▶ PlatformSemantics::observability
                                                                 │                                                         ├─▶ plan_binding (Signal | Observe | Once)
                                                                 │                                                         └─▶ diagnostics (§8.4), portability mode
                                                                 ├─▶ gen_backend.py / gen_cpp.py / AppKit, WinUI gens ─▶ backend tables: mechanism per prop ─▶ observe()
                                                                 ├─▶ gen_types.py ─▶ observed() vs getProp() getters, .d.ts remarks
                                                                 └─▶ verification tests ─▶ ledger (fact …:prop:<p>:observe)
```

### 9.4 Specs vs hand-written implementations

- **Default is all data.** `metadata/semantics/rules/<tk>.yaml` is already the hand-written layer, folded
  into the spec at generation time. Observability overrides belong in `metadata/overrides/<tk>`, not in
  Rust.
- **`Overlay<P: PlatformSemantics>`** wraps a spec backend and overrides individual answers in Rust,
  for what cannot be data:
  - a new platform before its metadata pipeline exists (UIKit, Android);
  - a renderer with no retained objects (egui: every prop `set_only`, bindings by polling the frame);
  - conformance doubles in tests.

  It replaces nothing in `SpecBackend`.
- **Portability mode** compiles against several `PlatformSemantics` and intersects their answers for
  diagnostics only. Output is still per platform.

### 9.5 QuickJS vs Node: what actually differs

| Difference | Where it lives |
|---|---|
| module resolution: QuickJS host loader with built-in `svelters:*` modules (QUICKJS-RUNTIME summary item 4) vs Vite/Node package resolution (`packages/types/vite.js` aliases) | `RuntimeTarget::specifier` (compiler) |
| `__svelters_host`: native global (QuickJS) vs JS TestHost vs N-API addon (node-deno-integration.md) | host packages; same JS API |
| microtask drain: the QuickJS host drains when it decides (after the batch, §5.2); Node drains after every native→JS callback, so the batch **must be one callback** (`napi_make_callback` once per checkpoint) | host, not compiler |
| `FinalizationRegistry` timing (release of SHARED/VALUE handles) | runtime + host; not observable in emitted code |
| dev: Vite HMR (`import.meta.hot`) vs in-process host reload (M2b) | `RuntimeTarget::dev` |
| `Intl` absent in QuickJS-NG | `caps()`; the compiler never emits `Intl` today, so no current effect |

The emitted component JS is identical for both targets today. `RuntimeTarget` starts with
specifiers and dev hooks only; capabilities are added when the first one changes codegen.

## 10. Phasing

| Phase | Work | Acceptance |
|---|---|---|
| **R0 facts** (done, `plans/milestones/R0-observability-facts.md`) | `observe` schema in `svmeta`; ingest Qt `BINDABLE`/`CONSTANT` from moc JSON and GTK notify for `metadata/gtk/types`; rules O1-O10; the two probes become `probe.py` sections; `ObserveSpec` in `gen_semantics.py` + `spec.rs`; coverage report rows; fix the GTK detailed-signal lookup (§11) | coverage report shows classes per toolkit; `gen_semantics.py --check` stable |
| **R1 ABI + GTK** | ABI v2.6 (`observe`, `prop_changed`, `SVL_M_OBSERVABLE`); host dirty set, read, compare, batch-before-drain, set-path read-back, sync flush, op-log lines; log backend + TestHost; GTK backend (`notify::<p>`, `via` re-targeting, cleanup in `release_entry`); `observed()` in `@svelters/types` + tracker in native-runtime; `gen_types.py` emits reactive getters | the NON-GUI-TYPES §7 TextBuffer example works without a `$state` mirror; `host_bench` "coalesced" numbers hold with real batches; generated observe tests in the GTK ledger |
| **R2 compiler** | `observability()`, `bind_prop`/`bind_readonly`/`bind_once` lowering, `\|nonreactive`, §8.4 diagnostics, portability mode, `.d.ts` remarks; `RuntimeTarget` with specifiers + dev hooks | fixtures for each lowering on GTK and Qt; svelte-check passes with the modifier |
| **R3 Qt** | NOTIFY relay, `QPropertyNotifier` map, event-filter table, non-NOTIFY signal fallbacks; Qt types (N2) get reactive getters | Qt observe facts in the ledger; `QTimer.interval` and `enabled` (event) observed |
| **R4 WinUI, AppKit** | DP callback tokens + event fallbacks; KVO observer + notification/target-action fallbacks; probe harness inside the backend test binaries on CI | CI green with observe tests; `NSTextField` typing and `TextBox` selection observed |
| R5 optional | JS value cache; `poll` mechanism for `none` props if users need it; `\|lazy` | measured before adopting |

## 11. Side findings (verified, worth fixing regardless)

1. **GTK: `notify::<prop>` can't be listened to on non-widget kinds.** `reflect.rs:128` resolves signal
   names with `SignalId::lookup`, which does not parse the `::detail`. So
   `new TextBuffer().on('notify::text', ...)` fails with `unknown Signal`. The bench hit it with
   `Gtk.Adjustment`.
   - The fix: parse the name with `g_signal_parse_name`, and key the reflected id by (signal id,
     detail quark), not by signal id alone. Otherwise two details would share one connection.
   - Widget kinds are unaffected for the 231 `notify::*` signals the tables declare.
2. **GTK: `GtkEntry:text-length` never notifies** (GTK 4.24.1, probe scenario), and
   **`GtkRange:fill-level` doesn't notify on set**.
   - *R0:* `text-length` is the fact `class: none` (verified: an insert changes it, nothing is announced).
     `fill-level` was wrong: a probe artifact (see §3.1); it is `exact`. The R0 probe found other unannounced side
     effects instead: `GtkWidget:css-classes` (internal class changes of GtkImage/GtkLevelBar), the GtkEntry icon
     tooltip text/markup pair, GtkTextTag's `*-set` and font props (`partial, missed: [cross_prop]`).
     The read-only review added one: **`GtkEventController:widget` is not notified** when the controller is added
     to or removed from a widget (GTK 4.22.4; `class: none`, verified by a silent row).
3. **GTK `EXPLICIT_NOTIFY` is not a reliable "no notify on identical set" flag:** 36/510 props
   re-notify anyway. The echo design in fork-bindings §6 already assumes compare-before-set; this
   confirms it is needed for observation too.
   - *R0:* recorded per fact as `fires: on_set | on_change` from the probe (GTK 4.22.4: 39/557 EXPLICIT_NOTIFY
     props re-notify an identical set, 206/206 others), and checked by the generated row.
4. **WinUI metadata marks `ActualWidth`/`ActualHeight` as DPs,** but they never notify. Rule B1
   ("WinUI: 774/784 rw props are DP-backed" → bindable) needs the O6 deny list before any read-only
   binding is generated from it.
   - *R0:* the deny list (`no_notify` in `metadata/rules/winui.yaml`, already used by AI1's B1) makes them
     `exact` through `event: [SizeChanged]` (and `ActualTheme` through `ActualThemeChanged`), never `notify`.

## 12. Decisions needed

1. **Delivery model.** Invalidate-and-read-live, coalesced per (handle, prop) per checkpoint, one batch
   delivered before signal events with a single job drain (§2, §5.2-5.3).
   *Recommend: yes. The measured ~30x on bursts (§6) makes this an ABI property, not an optimisation.*
2. **Values in the batch.** The host reads natively, compares with the last value and carries the value
   to JS. Proportional props are invalidation-only.
   *Recommend: yes for compare and convergence; the JS value cache stays optional (R5).*
3. **Default reactivity of generated getters.** Reactive for `exact`/`partial` props, which costs
   nothing outside effects; or opt-in per read.
   *Recommend: reactive by default.*
4. **Opt-out syntax.** `bind:prop|nonreactive` (initial read, then write-only) plus `untrack(...)` /
   `obj.peek(prop)` for expressions; reject per-import variants, new runes and component options (§8.1).
   *Recommend: as stated.*
5. **Partial observability policy.** Compile warning plus a dev runtime warning; error only in a
   `--strict-native` mode.
   *Recommend: warning by default.*
6. **`set_only` props.** No native observation; wrapper setters invalidate locally; two-way `bind:` on
   them is a compile error.
   *Recommend: yes.*
7. **Echo.** For observed props, host compare-with-last (written or delivered) replaces handler
   blocking; backend-side suppression only for proportional props; signal-based echo rules unchanged.
   *Recommend: yes.*
8. **`bind:` migration.** Lower `bind:` on `exact` props onto observation (R2) and keep signal plans
   where a user-intent signal is better; or keep all bindings signal-based.
   *Recommend: migrate gradually, one mechanism per prop.*
9. **Polling for `none` props** (frame-clock read for layout/WM-driven values).
   *Recommend: not in v1. Report `none` with a warning and revisit with real cases.*
10. **AppKit BORROWED + KVO.** Rely on the macOS 10.13 relaxed unregistration for autonotifying classes
    and remove from the identity sentinel; retain the object while observed for manual-notifying
    classes.
    *Recommend: yes. It needs the project's minimum macOS (≥ 10.13) to be confirmed.*
11. **Compiler traits.** Keep `BackendSemantics` (renamed `PlatformSemantics`) as the single object-safe
    platform trait with `observability` and type-member lookup; add a thin `RuntimeTarget` (specifiers,
    dev hooks); hand-written overrides through `Overlay`, not a parallel trait hierarchy (§9).
    *Recommend: yes. Also decide whether `|lazy` joins `|nonreactive` in R2 or waits.*

## 13. Decisions (user, 2026-10-19)

All recommendations in §12 are adopted, except #8:

1. **Delivery model:** yes. Coalesce per (handle, prop) per checkpoint and deliver one batch before signal events, with a single job drain.
2. **Values in the batch:** yes. The host reads natively, compares with the last value and carries the value. Proportional props are invalidation-only. The JS value cache stays optional (R5).
3. **Generated getters are reactive by default** for `exact`/`partial` props.
4. **Opt-out:** `bind:prop|nonreactive` plus `untrack(...)` / `obj.peek(prop)`. The rejected forms stay rejected (§8.1).
5. **Partial observability:** compile warning plus a dev runtime warning; an error only in `--strict-native`.
6. **`set_only` props:** no native observation; setters invalidate locally; two-way `bind:` is a compile error.
7. **Echo:** for observed props, host compare-with-last replaces handler blocking. Backend-side suppression applies only to proportional props. Signal-based echo rules are unchanged.
8. **`bind:` migration: all at once (not gradually).** In R2 every `bind:` on an `exact` prop is lowered onto observation (`bind_prop` / `bind_readonly`). Signal-based binding plans remain only for props that are not `exact` on that platform (e.g. AppKit user edits through the field editor, which KVO misses).
9. **Polling for `none` props:** not in v1. Report `none` with a warning.
10. **AppKit BORROWED + KVO:** as recommended. **The minimum macOS is 11**, which covers the 10.13 relaxed-unregistration requirement.
11. **Compiler traits:** one object-safe `PlatformSemantics` (renamed from `BackendSemantics`) with `observability` and type-member lookup, a thin `RuntimeTarget`, and overrides through `Overlay`. **`|lazy` waits** for R5.

