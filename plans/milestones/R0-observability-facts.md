# R0: observability facts (plans/research/REACTIVITY.md §4, §10 R0, decisions §13)

Lane: `tools/metadata/{common,gtk,qt,appkit,winui}`, `metadata/{observe,probe,inferred,rules}`, the GTK and Qt
verification harnesses, `crates/svelters-compiler/src/semantics/spec.rs`. Branch `ci/r0` (worktree branch
`worktree-agent-a4d83a1de5dd9e1bf`). Out of scope: host batching/compare/read-back and reactive getters (R1), compiler
lowering and diagnostics (R2), the Qt bindable/event/signal mechanisms (R3), AppKit/WinUI `observe` (R4).

## Scope

Every readable property of the four toolkits' metadata, widget kinds and non-GUI types (GTK `metadata/gtk/types`,
Qt `metadata/qt/types`), gets one `observe` fact per platform: class, mechanisms, misses, timing, cost, provenance.
The facts are inferred by rules from the ingested metadata and two probes, can be overridden by hand, are merged
into `metadata/observe/<tk>.json`, reach the compiler as `ObserveSpec`, and on GTK and Qt are verified by one
generated test row each through the real ABI v2.6 `observe` path into the ledger.

```text
GIR + live flags (probe) ─┐                                         metadata/overrides/<tk>/observe/*.yaml (manual)
Qt metaobject + moc JSON ─┤                                                         │
WinUI winmd (DP) ─────────┤─ rules O1-O10 (<tk>/observe_rules.py, run by infer.py) ─┤
AppKit probe (KVO) ───────┤        metadata/inferred/<tk>/observe/*.yaml ───────────┴─ common/observe.py ─▶ metadata/observe/<tk>.json
observe probes ───────────┘        (+ _evidence.json, _review.json)                                          │
  metadata/probe/gtk/observe/4.22.4.json                                       ┌──────────────────────────────┤
  metadata/probe/qt/observe/6.11.3.json                gen_semantics.py ─▶ ObserveSpec on every spec prop   │
                                                       gtk/gen_backend.py ─▶ observe_<owner> table tests ───┤
                                                       qt/gen_cpp.py ─▶ kObserveRows ───────────────────────┤
                                                       ledger.py refresh (rows; parks failing inferred) ◀────┘
                                                       coverage.py ─▶ "Observability" section + metadata/coverage/observe.json
```

## Schema

`tools/metadata/common/observe.py` (validation, layers, merge, `spec()`); documented in
`plans/architecture/widget-metadata.md` "Observability facts".

```yaml
prop: text
class: exact            # constant | set_only | exact | partial | none
via:                    # notify {signal, coarse} | bindable | signal {signal} | event {events} | notification {name}
  - {mechanism: notify, signal: "notify::text"}   # | target_action | via {object, prop}
missed: [cross_prop]    # partial only: user_edit | programmatic | cross_prop | parent | layout | wm | async
fires: on_change        # on_change | on_set | coarse
derived_from: [label]   # props whose set also announced this one (probe)
value_cost: cheap       # cheap | proportional
timing: sync            # sync | deferred
provenance: [{origin: probe, ref: "observe probe GTK 4.22.4: notified on every observed change (...)"}]
verify: {on: "Gtk:Entry", check: change}   # change | cross | via | silent | connect | refused | unsupported
```

Fact id `svelters-api:v1:type:<tk>:<Ns>:<Type>:prop:<prop>:observe`; owner keys `<Ns>:<Type>` (`Gtk:Entry`,
`Gio:ListStore`, `QtWidgets:QLineEdit`, `QtCore:QTimer`, `Microsoft.UI.Xaml.Controls:TextBox`, `AppKit:NSTextField`).
The compiler's view (`spec.rs`): `ObserveSpec { class: ObsClass, missed, coarse, value_cost, timing, mechanism }`,
`PropertySpec.observe: Option<ObserveSpec>` (absent = unknown; older specs load unchanged).

## Layers and ingest

| Layer | GTK | Qt | WinUI | AppKit |
|---|---|---|---|---|
| ingested (rule inputs) | `runtime.flags` + `notify` of `metadata/gtk/*.json`; the non-GUI types' flags (`EXPLICIT_NOTIFY`, `CONSTRUCT_ONLY`) come from the observe probe's flag dump (GIR has no ParamSpec flags; the live-registry ingest needs PyGObject, not on this machine) | metaobject `notify`/`constant`/`user`; **moc JSON** extract `metadata/qt/moc/properties.json` (`tools/metadata/qt/ingest_moc.py`, Qt 6.11.3 metatypes: 406 classes, 926 Q_PROPERTYs, 41 BINDABLE) for BINDABLE, which the PySide6 dump lacks, and for the NT2 types | `dependency_property` | the probe's `kvo` column |
| inferred | `metadata/inferred/gtk/observe/` | `metadata/inferred/qt/observe/` | `metadata/inferred/winui/observe/` | `metadata/inferred/appkit/observe/` |
| manual | `metadata/overrides/<tk>/observe/<Ns>.<Type>.yaml` (none written in R0) ||||
| merged | `metadata/observe/<tk>.json` (`observe.py`, `--check` in `pnpm test:meta`) ||||

Why a separate layer and not a field merged into `metadata/<tk>/*.json`: GTK's `ingest.py` (which merges the other
layers) needs PyGObject, and a property fact's fingerprint covers its whole record, so merging `observe` there would
have invalidated every verified property fact. Ledger parking works the same (`section: observe`).

## Probes (`probe.py --section observe`)

Both research probes became pipeline sections that need no PySide6/PyGObject, only a compiler and the toolkit SDK:

- **GTK** (`tools/metadata/gtk/observe_probe.{c,py}`): driven by the metadata (378 GObject types: the 298 widget-kind
  records and the non-GUI types), run against **GTK 4.22.4** (`--libdir` with the 4.22.4 package's `libgtk-4.so.1`; the
  metadata's version, not the machine's 4.24.1). Per type: the flags of every own pspec (`F`); a bare instance (`I`);
  every own (or abstract ancestor's/interface's) writable scalar property set to a new value **on a fresh instance**
  and then to the read-back value, with a diff of every readable property around the set (`S`, `E`); scenarios for
  changes that are not sets (TextBuffer edits/undo, Adjustment clamping, Entry insert, Scale, Notebook page removal,
  Label markup, ListStore/StringList append, SingleSelection, window present/maximize). ~20 s, crash-isolating
  reruns, deterministic (two runs byte-identical). 780 props swept; 13 types crash on a bare instance.
- **Qt** (`tools/metadata/qt/observe_probe.{cpp,py}`): `PROBE(QClass)` for every QObject class of the metadata
  (`std::is_default_constructible` decides which get an instance), built and run against the **Qt 6.11.3 SDK** (the CI
  and moc version) offscreen. Per class: metaobject flags (readable, writable, constant, bindable, user, NOTIFY); per
  writable scalar/enum property on a fresh instance: NOTIFY count (QSignalSpy), QBindable notifier calls, QEvents
  (event filter), the other signals emitted, and the diff of all readable properties. Scenarios: QLineEdit /
  QPlainTextEdit / QTextEdit / QSpinBox typing (QTest), QCheckBox click, parent disable/hide, hidden resize, QTabWidget
  tab removal, QTimer start/stop, QComboBox items. 158 classes, 700 props, 427 swept, 3 classes crash (QColumnView,
  QDockWidget, as in AI1).
- **AppKit / WinUI**: nothing new runs locally. AppKit's facts use the KVO column the AI1 probe already records on the
  macos-appkit job; WinUI has no probe (the R4 harness runs inside the backend's test binary on the windows-winui job,
  as AI1 did for its facts).

Fresh instances per property were a correction made during R0: on one shared instance a property's change could
depend on an earlier set (QGroupBox `checked` after `checkable`, GtkTextTag `font` after `family-set`), which the
generated tests then could not reproduce.

## Rules, with their measured precision against the probes

Confidence bands as the A-rules: 0.95 rule and probe agree; 0.9 rule alone, the generated test (or R4) is the check
left; 0.8 and below go to `_review.json` (not written).

| Rule | What | Measured |
|---|---|---|
| O1 (GTK) | readable ParamSpec -> `notify::<prop>`; probe decides exact / partial (`missed: cross_prop` or the scenario's cause) / none; `fires` from the identical-set re-notify | rule alone ("every change notifies") right for **775/793** props with an observed change (97.7%); the 18 misses are side effects: `GtkWidget:css-classes`, the GtkEntry icon tooltip text/markup pair and `progress-pulse-step`, `GtkEntry:text-length`, `GtkShortcutsShortcut:icon`, GtkTextTag `font`, `size-points` and `*-set`. `EXPLICIT_NOTIFY` as "no re-notify on an identical set": wrong for **39/557**; non-explicit props re-notify **206/206** |
| O2 (GTK, Qt) | CONSTRUCT_ONLY / CONSTANT -> constant | GTK 109 facts; the backend refuses them (-1) in every generated row. Qt: 0 (no CONSTANT among the metadata's QtWidgets props) |
| O3 (Qt) | NOTIFY -> `notify` (`coarse` when shared), BINDABLE -> `bindable` | NOTIFY announced every observed change for **63/71** props; misses: `QAction:iconVisibleInMenu`, `shortcutVisibleInContextMenu` (shared `changed()` not emitted), `QAbstractSlider:sliderPosition` (`sliderMoved` is user-only), `QDateTimeEdit:time`, `QGroupBox:checked`, `QProgressBar:value` (range changes), `QTextEdit:html` (a markdown set), `QWidget:windowTitle`. BINDABLE without NOTIFY: notifier fired on **9/9** swept changes |
| O4 (Qt) | QWidget event table (25 entries + 4 `readOnly`) | the mapped QEvent arrived on **12/12** swept changes; geometry is `timing: deferred` (a hidden widget gets Resize/Move at `show()`, probe scenario) |
| O5 (Qt, WinUI) | name-paired signals / events, explicit pairs in the rules | Qt: **6/8** signal-paired props announced every observed change (`QAbstractButton:down`, `QLineEdit:modified` miss programmatic sets -> partial). WinUI: no probe |
| O6 (WinUI) | DependencyProperty -> DP callback, minus `no_notify` (ActualWidth/ActualHeight -> SizeChanged, ActualTheme -> ActualThemeChanged) | not measurable on Linux (R4) |
| O7 (AppKit) | KVO when the probe saw it fire; NSControl values `partial, missed: [user_edit]` unless the class has a fallback (NSTextField + NSControlTextDidChangeNotification, NSSlider + action, NSButton state + action, NSWindow frame + DidResize/DidMove) | KVO fired on 173/175 probed programmatic sets (AI1 probe, macOS 26.1) |
| O8 (all) | no mechanism: writable, not USER, no native change seen -> set_only; otherwise none | Qt: 452 set_only, 296 of them set natively by the probe and never seen changed by anything else (by construction: a property the probe saw change unannounced is `none`, 56 such) |
| O9 (GTK) | forwarded props (`virtual.op: via_object`) -> `via: {object, prop}`, class of the target | 13 facts (GtkRange value/bounds, GtkSpinButton bounds, GtkTextView text) |
| O10 (GTK, Qt) | `derived_from` from cross-notify; `value_cost: proportional` for the whole-text props in the rules | GtkTextBuffer/TextView `text`, QTextEdit `html`/`markdown`/`plainText`, QPlainTextEdit `plainText` |

@@RESULTS@@
