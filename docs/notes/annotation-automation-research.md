# Annotation automation research (working notes)

Question: how far can per-kind override annotation (slots, props, signals, methods, bindings,
ownership, signal results, child attrs, text props) be automated? Final proposal:
`plans/research/ANNOTATION-AUTOMATION.md`. Scripts: `docs/notes/scripts/annot/`.

## 2026-10-17: inventory

- Overrides today: GTK 108 files / 2854 lines, Qt 81 / 2430, AppKit 10 / 346, WinUI 16 / 191.
- `git log -- metadata/overrides`: 118 commits.
- Override field census (counts of records carrying the field):
  - GTK: 273 property records (13 virtual, 33 with `sample`, 8 with `setup`), 74 signal records
    (64 with `trigger`: 13 property, 4 native, 47 rust), 61 methods (all with `expect`, 25 with
    `setup`), 16 bindings, 34 slots in 23 `children` blocks, 2 `text_prop`, 36 `unexposed`.
  - Qt: 285+36 inherited props (22 `suppress_signals`, 13 `sample`, 11 `setup`), 85+23 signals
    (80 `trigger`, 52 `expect_args`, 17 `synthetic`), 98+13 methods (23 `expect`, 37 `params`),
    21 bindings, 18 `children` blocks, 23 `unexposed`.
- First observation: most records restate the metadata. A GTK property record is usually only
  `name` + `provenance` (`"GtkButton:label (Gtk 4 docs)"`); a Qt prop is `{js: kebab(native),
  native}`. The provenance strings cite docs, but the fact itself is the presence of the record.
- The merged JSON already carries what a rule needs: GTK props have `notify`, `accessors`,
  `runtime.flags`, typed `default`/`range`; signals have `returns`, `flags`, `phase`; GTK
  `children.candidates` already lists `child_ops` and `widget_props`. Qt props carry
  `metaobject.{notify,user,writable,resettable}`, signals carry `metaobject_signatures` and
  `associated_property`.
- GTK rust triggers (47): about half are `o.emit_by_name::<R>("sig", &[sampled args])`, which is
  mechanical from the signal's param types. The rest set a related property/state through a
  typed downcast (`SpinButton.set_value`, `TextBuffer.set_text`), or reach a sub-object.
- Qt triggers (80): setter of the prop whose NOTIFY is the signal (`setValue(($i+1)*3)`),
  `Q_EMIT $w->sig(args)`, `click()`, and synthesized QMouseEvents.

## 2026-10-17: classification of existing overrides (`annot/classify_overrides.py`)

Each override fact is classified field by field: `rule` (a deterministic rule over the ingested
metadata gives the same value), `probe` (an observation a runtime prober can make), `judge`
(curation). A fact takes its worst field. Result (all 1336 hand-written facts):

| toolkit | facts | rule | probe | judge | rule% | rule+probe% |
| --- | --- | --- | --- | --- | --- | --- |
| GTK | 523 | 365 | 76 | 82 | 70 | 84 |
| Qt | 677 | 445 | 105 | 127 | 66 | 81 |
| WinUI | 73 | 52 | 12 | 9 | 71 | 88 |
| AppKit | 63 | 35 | 10 | 18 | 56 | 71 |

Per fact type (GTK / Qt rule+probe %): props 94/95, signals 88/74, bindings 100/57 (Qt: 9 of 21
picked a non-USER prop such as `currentIndex`; the change signal itself is the NOTIFY in 20/21),
slots 100/87 (ops are rule, `effects` are probe), child attrs 100/100, text_prop 100/100,
unexposed 94/87 (base-class families: item views, layouts, GL/RHI, deprecated), methods 15/41.

Where the judgement sits:
- **Methods** are the only large judgement class: GTK 52 of 61 method facts are a choice of
  *which* non-accessor method to test plus a hand-written `expect`/`setup`; Qt 66 of 111 likewise.
  The `expect` is usually "a property changed to X" (probe: state diff), rarely a custom Rust/C++
  expression.
- **Signal triggers**: GTK 42/64 rule (property set, Editable `set_text`, `emit_by_name` with
  sampled args), 13 probe (set a related prop through a typed downcast), 9 judge (reach a row or
  sub-object, add an offset). Qt 60/91 rule (NOTIFY-prop setter or `Q_EMIT`), 20 probe
  (`click()`, `setRange`), 11 judge (QMouseEvent synthesis, `findChild`, dialog accept/reject).
- **Qt `expect_args`** (56) are pure observations: the prober records them.
- **Setups**: populate-before-test (`addItems`, `addTab`, Notebook pages) and range-before-value
  (`set_range(0,100)`) recur; they are a handful of reusable *precondition recipes*, not per-kind
  judgement.
- **Renames/tags/synthetic events**: small and toolkit-wide (17 Qt `synthetic` signals are the
  QWidget event families, written once).

Git history: 118 commits touched `metadata/overrides` (+6609/-667 lines), 114 of them on
2026-10-17. 23 batch status lines list dropped facts; most drops were facts whose generated test
failed (clamped values, ignored-unless-checkable), i.e. exactly what a prober would have reported
before an agent wrote them.

## 2026-10-17: static rules over all kinds (`annot/infer_static.py`)

Method classes over the coverage denominators (GTK 1672 = own methods of 113 widget kinds; Qt
3021 = own method names of 80 widget classes):

| class | GTK | Qt |
| --- | --- | --- |
| prop accessor (get/set/is of a prop) | 891 (53%) | 994 (33%) |
| pure getter (zero-arg, non-void) | 59 | 308 |
| zero-arg action | 56 | 133 |
| scalar-arg call | 103 | 366 |
| child op (takes a widget) | 92 | 83 |
| object-arg call | 54 | 317 |
| deprecated | 371 (22%) | 11 |
| protected/virtual (Qt event handlers) | - | 731 (24%) |
| unsupported (callbacks, non-introspectable) | 16 | - |
| dangerous (denylist: run/exec/allocate/snapshot/static dialogs) | 30 | 42 + 36 static |

So 950/1285 non-deprecated, supported GTK methods (74%) and 1302/2243 public, non-static,
non-deprecated Qt methods (58%) are accessors or getters whose test is mechanical (call; compare with the property).

Recall of rules against the hand-written facts:
- GTK slots: 32/34 recalled by S1 (writable widget-typed prop with accessors, excluding reference
  names) + S2 (insert/remove families). Missed: `ActionBar.center` (no prop), `Notebook.pages`.
- GTK bindings: 12/16 with own props only; the 4 misses are interface props (`Editable:text`) and
  virtual props (`Range.value`, `TextView.text`). With interface props: 14/16.
- Qt bindings: 19/21 have a writable NOTIFY prop; 12/21 are the class's USER prop.
- text_prop: 2/2 GTK, 2/2 Qt.
- Signal results: `handler_result`/`sync_required` match `bool -> bool_stop + sync`, `void -> none`
  on all 74 GTK signal facts.

## 2026-10-17: GTK prober prototype (`annot/gtk_prober.py`, `annot/summarize_probe.py`)

PyGObject, GTK 4.22.4, Xvfb, `GSK_RENDERER=cairo GDK_DISABLE=gl,vulkan`. 74 concrete,
non-deprecated widget kinds; a full run takes **~10 s wall** (4 worker processes).

Design points that worked:
- Worker subprocesses print `#op kind:section:name` before every operation; on a segfault or
  timeout the driver blames the last announced op, adds it to a skip list and reruns the kind.
  Found 3 crashing ops: `Text`/`TextView` action signal `insert-emoji` emitted on an unrooted
  widget, `DragIcon:child` (set outside a drag). These become `unsafe: crash` facts.
- GLib log handlers on Gtk/GLib-GObject/Gdk turn criticals into `error` outcomes with the
  message as evidence (`gtk_button_set_icon_name: assertion 'icon_name != NULL'` = the prop
  cannot be reset to its NULL default; write-only `group`, `markup`).
- Every signal of the instance is recorded during each operation, so one prop set yields:
  the round trip, whether `notify::<prop>` fired, and **which other signals fired**
  (`CheckButton:active` -> `toggled`; `SpinButton:value` -> `value-changed`, `changed`,
  `output`). That is the trigger, the echo-suppress list and the binding change signal at once.
- Precondition recipes, tried only when the plain round trip is not exact: `range`
  (`set_range(0,100)` / an Adjustment), `populate` (3 labels through the insert op), `text`,
  `realized` (in a presented window). SpinButton `value` needed `range`, Notebook `page`
  needed `populate` (matches the hand-written setups).
- Slot vs reference: a widget-typed prop is a **slot** if the set child ends up parented under
  the host, a **reference** otherwise (`default-widget`, `focus-widget`, `transient-for`,
  `mnemonic-widget`, `key-capture-widget`, `stack`). Expander `child` is only parented when
  expanded: a reveal recipe (`expanded`, `reveal-child`, `search-mode-enabled`) fixes it.
  Replace semantics (old child finalized once the host's ref goes), `set(None)` unparenting,
  insert order (`append` vs `prepend`) and wrapping (Notebook pages are `ancestor`, not
  `direct`) come out of the same probe.

Results (own facts of the 74 kinds):

| fact | probe result |
| --- | --- |
| writable props | 339 exact round trip (all with notify), 44 identity-only (object/boxed), 5 ignored, 3 error |
| annotated props (211 probed) | 200 exact + 11 identity; 0 disagreements with the ledger |
| accessor getters | 378 exact (equal to the property) |
| accessor setters | 287 exact, 3 differ, 70 no sample (object types without a factory) |
| pure getters | 39 called ok |
| zero-arg actions | 33 ok: 8 with an observable effect (`Spinner.start` -> `spinning`, `EditableLabel.start_editing` -> `editing`), 25 no visible effect |
| signals (126 own) | result/sync from the return type for all; trigger found for 38 (27 by emitting an action signal, 11 by a prop set) |
| one-slots | 30 slots, 10 references; all 30 slots: old child finalized on replace, `set(None)` unparents |
| many-slots | 20 insert/remove families exercised (append/prepend/pack/put/attach/add_overlay/append_page) |
| hand-written slots recalled | 33/34 (missing: `ActionBar.center`, a set/get pair without a property) |

Method facts with probe evidence: 378 + 287 + 39 + 8 = **712** of the 1672 GTK methods
(today 52), before scalar-arg calls (103) and object-arg calls with factories.

## 2026-10-17: Qt prober prototype (`annot/qt_prober.py`)

PySide6 6.11.0 against Qt 6.11.0, `QT_QPA_PLATFORM=offscreen`, no Xvfb. 71 concrete QtWidgets
classes from the metadata; **~13 s wall** with 4 workers. Same per-op crash isolation as GTK.

- Every signal is spied with `QSignalSpy(obj, QMetaMethod)`, so a prop write records the NOTIFY
  emission **and the emitted args** (`QSpinBox.value=1` -> `valueChanged(int) [[1]]`,
  `textChanged(QString) [['1']]`). Those are the hand-written `trigger` + `expect_args` pairs.
- PySide has no converter for some namespaced enums in `QObject::property()`; the prober falls
  back to the generated getter/setter (`horizontalScrollBarPolicy`, `orientation`, `timeSpec`).
- Ownership: after each replace/remove, `QCoreApplication::sendPostedEvents(DeferredDelete)` and
  `shiboken6.isValid` tell deleted from unparented from still-parented.

Results:

| fact | probe result |
| --- | --- |
| writable own props | 345 exact, 3 normalized, 10 ignored, 5 error, 30 no sample (QIcon/QFont/QRect/QPoint/QPixmap: no factory yet), 2 crash |
| NOTIFY confirmation | 28 props fired their NOTIFY on write; 51 exact writes also fired other signals with args |
| accessors | 423 getters exact (27 differ), 304 setters exact (14 differ, 55 no sample) |
| pure getters | 179 ok |
| zero-arg actions | 85 ok: 15 observable effect, 70 no visible effect |
| own signals (195) | trigger found by a prop write for 50 |
| one-slots `setX(QWidget*)` | 15 slots (11 old deleted, 3 old still parented, 1 unparented), 11 references (`setBuddy`, `setFocusProxy`, `setMenu`, `setCurrentWidget`) |
| many-slots | 11 add/insert families; `QTabWidget::removeTab` leaves the page parented (8 such) |
| crashes | `QColumnView` `previewColumnVisible`, `QDockWidget` `dockLocation` (segfault, isolated) |

Agreement with the hand-written Qt props: 267 of 288 probed annotated props round-trip exactly.
The other 21 are the cases where the human wrote a `sample`/`setup`: missing factories (icon,
font, geometry, pixmap), populate recipes not yet covering `QTabBar::addTab(QString)` and
`QToolBox::addItem`, `cursorPosition` needing text, `maximumWidth` sampled past 16777215. Each is a
sampler/recipe fix, not per-class judgement.

The probe rediscovered, without being told, several facts A1 workers found by failing tests:
`QDockWidget::setWidget` keeps the old widget parented (G15), `QTextEdit.html` normalizes to a
full document (dropped in A1), `QMenuBar.nativeMenuBar` is ignored on xcb/offscreen (dropped),
`QTabWidget::removeTab` does not unparent (the `then:` steps), `QKeySequenceEdit.maximumSequenceLength`
is limited to 1..4 (no range in the metadata).

## 2026-10-17: crash isolation, refined

The first crash heuristic (blame the last announced op, skip it, rerun) misblames heap
corruption: `Adw.Carousel` aborted with `malloc(): unaligned tcache chunk` several ops after the
culprit, and the skip loop walked backwards through 12 innocent methods. Fix: a crashed kind is
rerun in **isolated mode**, one op per process (`SVL_PROBE_ONLY=i`), and each op's result is taken
from its own process. Before every op the worker also runs `gc.collect()` and drains the main
context, so deferred finalizer crashes land on the op that created them.
- `Adw.AlertDialog` / `Adw.MessageDialog` `default-response` set: segfault, confirmed in
  isolation (real `unsafe` fact).
- `Adw.LayoutSlot`: aborts on construction without its construct-only `id` (a "required
  construct property" fact).
- `Adw.Carousel`: aborts only when many ops share a process; every op passes alone. Such kinds
  need a sanitizer run (ASan/valgrind) rather than blame; the prober reports them as
  `cross-op corruption` and drafts nothing that is not verified by the generated tests.
- Cost: isolated mode is ~0.5-1 s per op (~4 min for three crashy Adw kinds). It runs only for
  kinds that crashed in batch mode.

## 2026-10-17: a pack for free: libadwaita (`gtk_prober.py --ns Adw`)

No ingested metadata, kinds discovered from GI (63 concrete widget classes). One run, ~60 s:
218 props round-trip exactly (217 with notify; one prop has no notify), 17 identity-only, 8
ignored; 294 accessor getters and 197 setters verified; 31 one-slots (all old child finalized on
replace) and 19 references told apart; 10 many-slot families. That is the evidence an agent would
otherwise write by hand for every Adw kind; what is left is the same residual as GTK (tags,
which Adw widgets to expose, non-accessor methods).

## 2026-10-17: WinUI static rules (`annot/winui_static.py`)

`winmd-tool dump` of 78 WinUI 3 control names (Windows App SDK 2.5.1 winmds, fetched into
`target/winsdk`) gives 87 classes in `Microsoft.UI.Xaml.Controls` with their bases' members:
- 915 props, 784 read-write; **774 of the 784 are backed by a DependencyProperty** (the
  `<Name>Property` static), so `RegisterPropertyChangedCallback` gives a uniform change
  notification: every DP prop is a two-way binding candidate by rule. 443 are scalar or enum.
- 180 events; 37 have args with a settable `Handled`/`Cancel` (sync-result candidates by rule).
- 25 classes carry `ContentProperty` (the default slot by rule); `Panel.Children` /
  `ItemsControl.Items` are the many-slots; 45 static `GetX`/`SetX(element, v)` pairs are attached
  properties (= child attributes: `Grid.Row`, `Canvas.Left`, ...).
- No runtime here (Windows only). A WinUI prober would run inside the WinUI backend's test
  binary on the Windows CI runner: set every DP through `SetValue`, read with `GetValue`, observe
  `RegisterPropertyChangedCallback` and the class's events; same evidence format.

## 2026-10-17: drafts in the existing override format (`annot/draft_gtk.py`)

Probe + metadata -> `<Kind>.yaml` in the current GTK override schema plus
`<Kind>.evidence.json` (confidence + raw evidence). 69 kinds drafted, **0 schema errors**.
- Drafted: 346 props (305 at confidence >= 0.9), 100 signals (24 >= 0.9: trigger found by probe;
  76 at 0.6: delivery-only), 604 methods (601 >= 0.9: accessors with the probe's sample and an
  `expect` on the property), 35 one-slots (23 >= 0.9), 10 references.
- Against the hand-written overrides of the same kinds: props 199/203 reproduced, signals 36/36,
  one-slots 20/22 by name (slot names come from stripping `-child`/`-widget`, which matches the
  human names `start`, `end`, `label`, `title`, `center`), methods 15/45 (the other 30 are the
  hand-picked non-accessor methods).
- Validation in a scratch worktree (not this checkout): merging the >= 0.9 drafts into the
  overrides added 76 props, 13 signals, 486 methods, 5 slots. `ingest.py` rejected every draft
  because `svmeta.validate` does not accept `origin: probe`, although
  `tools/metadata/gtk/override.schema.json` lists it (schema/validator mismatch; replaced by
  `runtime` in the scratch copy). `gen_backend.py` then refused 3 drafts (echo_suppress on an
  object-typed `adjustment`: drafting rule fixed to skip identity-only props).
- First validation build of the scratch worktree failed to compile: two drafted typed slots,
  `CheckButton.child` (`gtk_check_button_set_child`, GIR `version="4.8"`) and
  `GraphicsOffload.child` (kind since 4.14), are newer than `svelters-gtk`'s gtk4-rs feature
  baseline `v4_6`. New rule V1: gate typed facts on the *method's* GIR `version` (the prop's
  `since` is wrong for CenterBox: props 4.10, setters 4.0). The ingested method records do not
  carry `since`; `draft_gtk.py` reads the GIR directly for now.

## 2026-10-17: verification run (drafts through the real generated tests)

In a scratch git worktree (since removed), the confidence >= 0.9 drafts were merged under the
manual overrides (76 props, 13 signals, 486 methods, 3 one-slots after V1). Then:
`ingest.py`, `gen_backend.py`, `gen_bindings.py`, and
`tools/dev/cargo-slot xvfb-run -a cargo nextest run -p svelters-gtk --no-fail-fast`.

Result: **1164 tests, 1111 passed (8 flaky), 53 failed, all of them drafted method facts.**
Every drafted prop, signal and slot test passed. So 434/486 drafted methods (89%) and 100% of
the other drafted facts pass without any human edit. The failures come from three draft-generator
bugs, not from wrong facts:
- enum samples written as integers, where the harness compares nicks (`wrap-mode`,
  `transition-type`, `selection-mode`, `input-purpose`, `ellipsize`, ...): fix the sampler output;
- object-typed setter args that the dynamic call path cannot pass from a test literal
  (`set_adjustment`, `set_menu_model`, `set_extra_menu`, `set_popover`, `set_dialog`,
  `set_buffer`): do not draft those as `dispatch: dynamic` with literal args;
- getters of object/strv props compared by handle (`get_pages`, `get_authors`) and gunichar
  returns (`get_invisible_char`, unsupported by the GI layer): skip or use round-trip expects.

In the pipeline these 53 would be parked automatically with their failure messages. Run time:
~28 min for the whole crate at the slot's 6 threads. Each GTK test spins up a process (~3 s),
so the test cost per fact, not the inference, is what scales.
