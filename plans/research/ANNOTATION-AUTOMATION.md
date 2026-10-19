# Widget annotation automation: rules, probes, and a small review queue

Status: research proposal, 2026-10-17; decisions in section 11. Implementation (lane AI1): the GTK slice is in
`tools/metadata/gtk/{probe,infer}.py` and `metadata/{probe,rules,inferred}/gtk`, see `tools/metadata/gtk/README.md` and
`docs/notes/ai1-inference.md` for what was built and the resulting counts.

- **Working notes and raw numbers:** `docs/notes/annotation-automation-research.md`.
- **Prototypes** are in `docs/notes/scripts/annot/`:

  | Script | Does |
  |---|---|
  | `classify_overrides.py` | classifies every hand-written fact as rule, probe or judgement |
  | `infer_static.py` | static rules over the ingested metadata; recall against the overrides |
  | `gtk_prober.py` | the GTK runtime prober (PyGObject); `--ns Adw` probes a pack with no metadata |
  | `qt_prober.py` | the Qt runtime prober (PySide6, offscreen) |
  | `summarize_probe.py` | summarizes a probe and scores it against the overrides and ledger |
  | `draft_gtk.py` | turns a probe into draft overrides with confidence and evidence |
  | `winui_static.py` | WinUI rule counts over a `winmd-tool dump` |

- **Builds on:**
  - `plans/milestones/A1-api-surface.md` (overrides, generated tests, ledger, coverage);
  - `plans/architecture/widget-metadata.md`;
  - `plans/research/CUSTOM-WIDGETS.md` (packs);
  - `plans/research/NON-GUI-TYPES.md` (that lane shares the facts schema and the ledger; see section 9).

## Summary

- **About 80% of what A1 wrote by hand could have been derived mechanically.** Per fact, by
  what could have produced it:

  | Toolkit | Rule | Probe | Judgement |
  |---|---|---|---|
  | GTK | 70% | 14% | 16% |
  | Qt | 66% | 15% | 19% |

  Rule means the ingested metadata gives the fact. Probe means a runtime observation gives it.
  Judgement is concentrated in two places:
  - *which* non-accessor methods to expose, and how to test them;
  - a few triggers that need event synthesis or a sub-object.
- **Runtime probers work and are cheap.** The probers set every property, watch every signal,
  try child operations and isolate crashes per operation. A full pass takes:
  - GTK, 74 kinds: about 10 s;
  - Qt, 71 classes: about 13 s;
  - libadwaita, 63 kinds with no metadata at all: about 60 s.

  The probes reproduce the hand-written facts:
  - **GTK:** 199/203 properties, 36/36 signals and 20/22 one-slots, with 0 schema errors in the
    drafts.
  - **Qt:** 267/288 properties round-trip. The probe also rediscovered, unprompted, the traps
    that A1 workers found through failing tests.
- **Probe evidence is not truth: the generated tests stay the oracle.** The pipeline proposed here
  works as follows:
  - rules and probes write an *inferred* layer, with provenance and confidence;
  - the existing generators emit a test for every inferred fact;
  - the ledger records each result;
  - inferred facts that pass are promoted automatically;
  - inferred facts that fail are parked with their failure;
  - manual overrides keep precedence.
- **Projected verified coverage**, against today:

  | Measure | GTK today | GTK projected | Qt today | Qt projected |
  |---|---|---|---|---|
  | Methods | 52/1672 | ~900 | 90/3021 | ~950 |
  | Props | ~270 annotated | ~470/543 | 268/522 | ~480/522 |

  The full table is in section 7. What remains for agents is roughly 1,000 review items per
  toolkit, batched by pattern. That is a few cheap batches, not 22.
- **First step:** promote the GTK prober into `tools/metadata/gtk/probe.py`, with a checked-in
  probe file. Then add an `infer.py` pass that writes `metadata/inferred/gtk/*.yaml` in the
  existing override schema, and let `ingest.py` merge it under the manual overrides. GTK
  accessor methods and properties alone add about 850 verified facts.

## 1. What A1 annotated, and what could have produced it

`classify_overrides.py` walks all 1,336 hand-written facts and classifies each field. A fact gets
the worst class among its fields.

| Toolkit | Facts | Rule | Probe | Judge | Rule+probe |
|---|---|---|---|---|---|
| GTK | 523 | 365 | 76 | 82 | 84% |
| Qt | 677 | 445 | 105 | 127 | 81% |
| WinUI | 73 | 52 | 12 | 9 | 88% |
| AppKit | 63 | 35 | 10 | 18 | 71% |

The rule+probe share by fact type:

| Fact type | GTK | Qt | How it is derived, and where judgement remains |
|---|---|---|---|
| props | 94% | 95% | The fact is mostly the existence of the record (`name` + a docs citation, or `{js: kebab(native), native}`). The human added value only through `sample`/`setup`, which is probe work. |
| signals | 88% | 74% | `handler_result`/`sync_required` follow the return type in 74/74 GTK cases. Triggers are mostly a NOTIFY-prop setter, `set_text`, or a synthetic emit with sampled args. Qt `expect_args` (56) are pure observations. |
| bindings | 100% | 57% | The change signal is the notify or NOTIFY signal (20/21 on Qt). 9 Qt bindings chose a non-USER prop (`currentIndex`), which is the judgement. |
| slots | 100% | 87% | Ops are set/get pairs and add/remove families. Effects are probe observations. |
| child attrs | 100% | 100% | They are GtkLayoutChild/page-object props, Qt `(index, value)` setter/getter pairs and WinUI attached properties. |
| text_prop | 100% | 100% | The first writable string prop named label/text/title/content. |
| unexposed | 94% | 87% | Base-class families: item views, layouts, GL/RHI, deprecated. |
| methods | 15% | 41% | Choosing *which* non-accessor method, plus a hand-written `expect`/`setup`. |

Git history agrees. 118 commits touched the overrides (+6,609/−667 lines), 114 of them on one
day. The 23 batch status lines that list dropped facts are mostly facts whose test failed:
clamped values, a prop ignored unless `checkable`, normalized HTML. A prober reports exactly
these before anyone writes the fact.

## 2. Rule catalogue

The rules are deterministic functions of the ingested metadata. Each rule names the fields it
fills. A probe or a manual override can raise or lower a rule's result, but a rule never
overrides either of them.

| Id | Rule | Toolkits | Measured |
|---|---|---|---|
| **P1** | Every readable prop is exposed. Writable + not construct-only gives a round-trip test; read-only gives a read test. | all | GTK props are already reflected; Qt needs only `{js, native}` |
| **P2** | js name = kebab(native); tag = kebab(class without prefix). Renames are manual. | all | Qt 313/313 props, 52/55 tags |
| **P3** | Sample by type, range and name: `uri`→URL, `file`→an existing PNG, `icon-name`→`edit-copy`, `*mask`→`AAA`, `*format`→`yyyy-MM-dd`, index props→1. An object factory table covers Adjustment, MenuModel, ListModel, GFile, QIcon, QFont and so on. | all | the residual Qt prop failures are all missing factories |
| **R1** | Signal result: bool → `bool_stop` + sync; other non-void → `value` + sync; void → `none`. | GTK, Qt, WinUI (`Handled`/`Cancel` args) | 74/74 GTK; WinUI: 37 of 180 events |
| **R2** | Trigger: the setter of the prop whose notify/NOTIFY is the signal. Otherwise a parameterless action emit. Otherwise a synthetic emit with sampled args, which is marked *delivery-only*. | GTK, Qt | 42/64 GTK and 60/91 Qt hand-written triggers |
| **R3** | Qt overloads: connect the longest signature and collapse the rest. | Qt | 91/91 |
| **B1** | Bindable: readable + writable + scalar + has a notify (GTK notify, Qt NOTIFY, WinUI DependencyProperty). The change signal is the notify. | all | GTK 14/16, Qt 19/21. WinUI: 774/784 rw props are DP-backed |
| **B2** | Primary binding (`bind:value`): the Qt `USER` prop; GTK stems `active`/`value`/`text`/`selected`/`page`; Qt `currentIndex`. | GTK, Qt | Qt 12/21 by USER alone, 21/21 with `currentIndex` |
| **S1** | One-slot: a writable widget-typed prop with get/set accessors, minus the reference-name list (`default`, `focus`, `mnemonic`, `transient`, `relative`, `buddy`, `key-capture`). Slot name = prop minus `-child`/`-widget`. Qt: `setX(QWidget*)` + `x()`. | GTK, Qt | GTK 32/34 recalled |
| **S2** | Many-slot families: `append`/`prepend`/`insert_child_after`/`pack_start|end`/`add_overlay`/`put`/`attach`/`append_page` with `remove*`. Qt: `add|insert(Widget|Tab|Item)` with `remove*|take*`. WinUI: `Panel.Children`, `ItemsControl.Items`. | all | GTK 20 families probed |
| **S3** | Default slot: `child`/`content` > the only slot > the WinUI `ContentProperty`. | all | 25 WinUI classes |
| **A1** | Child attributes: the props of the container's layout-child or page type (`GtkGridLayoutChild`, `GtkNotebookPage`, `GtkStackPage`, `GtkOverlayLayoutChild`, ...). Qt `setX(int index, T)` + `x(int index)`. WinUI static `GetX`/`SetX(element, v)`. | all | GTK: every X3 attr is one of these props. Qt: 74 pairs. WinUI: 45 |
| **T1** | text_prop: the first writable string prop named label/text/title/content. | all | 2/2 GTK, 2/2 Qt |
| **M1** | Method classes: accessor (associated prop / get_X, set_X, is_X of a prop), pure getter (zero-arg, non-void, `get_`/`is_`/`has_`/`n_`, Qt `const`), action (zero-arg void), scalar call, child op, object call. | GTK, Qt | GTK accessors 891/1672 (53%); Qt 994/3021 |
| **M2** | Denylist: blocking (`run`, `exec`, `getOpenFileName`-style static dialogs, `QMessageBox::information`), internal vfunc-context calls (`measure`, `allocate`, `snapshot*`, `queue_*`, `realize`), raw pointers, callbacks without destroy-notify, protected/virtual Qt event handlers (731), deprecated (GTK 371), and everything a probe crashed on. | GTK, Qt | removes ~30% of the method denominators |
| **U1** | Unexposed by base-class family or deprecation: item views → `virtuallist`, layouts → box/frame, GL/RHI surfaces, deprecated dialogs. | GTK, Qt | 34/36 GTK, 20/23 Qt reasons |
| **V1** | Version gate: a fact whose native member is newer than the typed baseline (GTK `v4_6` in `crates/svelters-gtk`) goes through the runtime-dlsym path, or is not drafted as a typed slot. The method's GIR `version` attribute decides. The prop's `since` is not enough: CenterBox `start-widget` is 4.10, but `set_start_widget` is 4.0. Ingest does not copy method `version` into the JSON yet. | GTK (Qt `since`, WinUI contract versions alike) | found by the validation build: CheckButton `child` (4.8) and GraphicsOffload (4.14) failed to compile as typed slots |
| **L1** | Lifetime from the base class (toplevel, owned, borrowed by `get_*` of a child). This already happens in ingest (`origin: derived`); the probe confirms it. | all | already derived |

The rules that matter most:
1. **M1 accessors.** They turn about 1,700 methods into "same as the property" tests.
2. **P1/P3** samples by type and name.
3. **B1/R2:** the notify signal is the binding change signal and the trigger.
4. **S1/S2/A1:** slots and child attrs.
5. **M2:** the denylist.

## 3. The runtime prober

### 3.1 Design (as prototyped)

**One worker process per batch of kinds.** The worker announces each operation
(`#op kind:section:name`) before running it. On a crash or timeout, the driver reruns the crashed
kind in **isolated mode**, one operation per process (`SVL_PROBE_ONLY=i`). It merges the results
and records the operation that crashed alone as an `unsafe` fact. Before each operation the
worker runs `gc.collect()` and drains the main loop, so a deferred finalizer crash is blamed on
the operation that caused it.

GLib log handlers, or `qInstallMessageHandler`, turn criticals and warnings during an operation
into an `error` outcome, with the message kept as evidence.

The prober runs these operations on every kind:

| Operation | What it does | Facts it gives |
|---|---|---|
| props | Set a typed sample, read it back, record `exact`, `normalized`, `ignored` or `error`. Every signal on the instance is recorded during the set: GObject `notify` and all signals, or `QSignalSpy(obj, QMetaMethod)` for each signal with its args. | round trip; notify; trigger; echo-suppress list; binding change signal; Qt `expect_args` |
| recipes | Retried only when the plain round trip is not exact: `range`, `populate`, `text`, `checkable`, `realized`, and reveal (`expanded`, `reveal-child`). | `setup`; the recipe that worked is recorded |
| methods | Accessors are compared with the property. Pure getters are called. Zero-arg actions are called and diffed (props + signals). Denylisted methods are skipped. | method facts with `args`/`expect` |
| slots | For each widget-typed prop or `setX(QWidget*)`: is the child parented here (a slot) or not (a reference)? Is it read back? Replace it, and check whether the old child is finalized/deleted, unparented, or still parented. Does clear unparent it? For many-slot families: insert two, check order and wrapping (direct vs ancestor), remove one, check its parent and liveness. | slot ops, effects, order, `accepts`, references |
| signals | Return type, flags and run phase. Parameterless action signals are emitted. | result/sync; action triggers |

The output is one JSON document per toolkit version, with keys sorted. Evidence values are
normalized: enum nicks, rounded floats, `<Type>` for handles.

### 3.2 Results

| | GTK 4.22.4 (74 kinds) | Qt 6.11 (71 classes) | libadwaita (63 kinds, no metadata) |
|---|---|---|---|
| wall time | ~10 s | ~13 s | ~60 s, plus ~4 min isolated mode for 3 crashy kinds |
| writable props round trip exact | 339/391 (+44 identity-only, 5 ignored, 3 error) | 345/395 (30 without a factory, 10 ignored, 5 error) | 218 (+17 identity, 8 ignored) |
| accessor getters / setters verified | 378 / 287 | 423 / 304 | 294 / 197 |
| pure getters, observable actions | 39, 8 | 179, 15 | 1, 0 |
| one-slots / references told apart | 30 / 10 | 15 / 11 | 31 / 19 |
| many-slot families | 20 | 11 | 10 |
| crashing ops isolated | `insert-emoji` on unrooted Text/TextView; `DragIcon:child` | `QColumnView.previewColumnVisible`; `QDockWidget.dockLocation` | `AlertDialog`/`MessageDialog` `default-response`; `LayoutSlot` construct |

Agreement with what humans wrote:
- **GTK.** All 211 annotated props that were probed pass (200 exact, 11 identity). Of the 34
  hand-written slots, 33 are recalled, with the same effects.
- **Qt.** 267 of 288 annotated props round-trip. The other 21 are exactly where a human wrote
  `sample`/`setup`: icon/font/geometry factories, populate for `QTabBar`/`QToolBox`, and text
  before `cursorPosition`.

The Qt probe found these on its own:
- `QDockWidget::setWidget` keeps the old widget parented (A1 gap G15);
- `QTabWidget::removeTab` does not unparent the page (A1's `then:` steps);
- `QTextEdit.html` normalizes to a full document, and `QMenuBar.nativeMenuBar` is ignored on
  xcb (both dropped by A1 workers after failing tests);
- `QKeySequenceEdit.maximumSequenceLength` is limited to 1..4, a range the metadata lacks.

**Drafts.** `draft_gtk.py` turns the GTK probe into override YAML for 69 kinds, with 0 schema
errors:

| Fact | Drafted | Confidence ≥ 0.9 |
|---|---|---|
| props | 346 | 305 |
| methods | 604 | 601 |
| signals | 100 | 24 |
| one-slots | 35 | 23 |

The signals below 0.9 are delivery-only, because no native trigger was found.

**Validation through the real generated tests.** The ≥ 0.9 drafts were merged into a scratch
worktree (76 props, 13 signals, 486 methods, 3 slots), and ingest, `gen_backend` and
`cargo nextest -p svelters-gtk` were run:
- **1164 tests: 1111 passed, 53 failed.**
- All 53 failures are drafted methods: 434/486 pass. Every drafted prop, signal and slot passes.
- The failures are three draft-generator bugs, not wrong facts: enum samples as ints instead of
  nicks; object-typed setter args that test literals cannot express; handle comparisons for
  object/strv getters.
- In the pipeline these would simply be parked.

Pipeline frictions found on the way:
- `svmeta.validate` rejects `origin: probe`, although the GTK override schema allows it.
- The generator refuses `echo_suppress` on object-typed props, so the draft rule now skips them.
- Two typed slots newer than the gtk4-rs `v4_6` baseline did not compile. That led to rule V1.
- Test cost is the bottleneck: about 3 s per GTK test process, 28 min for the crate. Inference
  takes seconds. Open question 3 applies to GTK too.

### 3.3 What the prober does not do (yet)

- It does not discover **which of several signals is the user-facing change** when a native
  interaction is needed (a click, a drag, typing). A prop set covers most cases. Mouse and key
  synthesis are one template per event type (Qt `QMouseEvent`/`QKeyEvent`, GTK `emit_by_name`
  on controllers), written once per toolkit and not per class.
- It does not decide **exposure policy**: tags, renames, and which non-accessor methods are worth
  having.
- Its **samplers and factories** limit it. Each failure class is a one-line factory or name rule,
  not a per-class fix.
- It does not run on **WinUI/AppKit** locally. The same harness belongs inside the backend's test
  binary on the Windows/macOS CI runners: `SetValue`/`GetValue` +
  `RegisterPropertyChangedCallback` for WinUI, and KVC/KVO + the ObjC runtime for AppKit.
- It does not test **cross-op heap corruption** (`Adw.Carousel` aborts only when ops share a
  process). Such kinds get a sanitizer run and no drafts.

## 4. The residual and the agent loop

### 4.1 What needs judgement after rules and probes

| Item | GTK | Qt | Notes |
|---|---|---|---|
| non-accessor methods to accept or deny (scalar/object calls, actions with no visible effect) | ~180 | ~800 | Mostly decided **by pattern** (a whole family at once). |
| props needing a factory, recipe or range | ~15 | ~50 | Fix the sampler once; the facts follow. |
| signals with no native trigger (delivery-only today) | ~90 | ~140 | An event-synthesis template per family. |
| many-slot op args (prev_sibling/index/wrapper), sub-object slots | ~20 | ~10 | ListBox rows, Notebook pages, Stack pages. |
| tags, renames, primary binding names, unexposed reasons | ~110 kinds | ~80 classes | One decision per kind; drafts propose a default. |
| crash/corruption kinds | ~3 | ~2 | Sanitizer run, then a denylist entry. |

### 4.2 Making agents about 10× faster on it

1. **Batch by pattern, not by class.** A review item is a pattern plus all its instances. Examples:
   - "`set_X(bool)` accessor, probe exact, 214 instances": accept all.
   - "Qt `scrollTo*` / `ensure*Visible`, 23 instances": deny as layout-dependent.
   - "action with no visible effect, 25 instances": deny, or keep as a callable-only fact.

   A1 batches were per family and touched the same patterns many times over.
2. **Agents review diffs, not YAML.** The inference pass writes drafts. The reviewer sees a table
   of `pattern | instances | confidence | evidence | proposed decision`. They answer
   accept/deny/edit per row, and per instance only for outliers. Decisions are written to
   `metadata/rules/<tk>.yaml` (patterns) or the manual overrides (single facts). They are never
   copied into the drafts.
3. **Route by confidence.**
   - **≥ 0.9:** auto-promote once the generated test passes. No reviewer.
   - **0.6–0.9:** Haiku reviews in large batches (≈300 rows per call). It checks that the evidence
     supports the fact, for example "notify fired", "parent direct" or "old child finalized".
   - **< 0.6 and conflicts:** Sonnet. Conflicts are cases where rule and probe disagree, or a
     probe outcome is `ignored`/`error`. Sonnet gets the evidence and the docs snippet.
   - **Opus:** only exposure policy and new toolkit families.
4. **No compile loop for reviewers.** The tests run once per wave in CI, through `cargo-slot`.
   Failures come back as rows ("draft failed: `<test>`: `<message>`") in the next review table,
   not as an agent debugging session.

### 4.3 Estimate

A1 spent 22 batches with up to 8 Sonnet workers to produce 1,336 facts: about 60 facts per batch,
each batch a full session with builds.

With the pipeline:
- **Auto-promoted, no agent:** GTK ~1,400 facts, Qt ~1,300.
- **Review queue:**
  - GTK: ~400 rows, about 40 patterns, plus ~110 kind defaults;
  - Qt: ~1,000 rows, about 80 patterns, plus ~80 class defaults.
- **Cost:** roughly 2–3 Haiku calls and 3–4 Sonnet sessions per toolkit. That is about one
  working day of agent time per toolkit, against A1's multi-wave effort, and the token use is a
  fraction because nobody rebuilds per fact.
- **New toolkit version:** a re-probe plus a review of the diff only, typically tens of rows.

## 5. Pipeline

```
ingest (GIR/qdoc/metaobject/winmd/headers) ─┐
probe (offline, per toolkit version) ───────┼─> infer (rules + probe) ─> metadata/inferred/<tk>/*.yaml
                                            │                            + *.evidence.json
metadata/rules/<tk>.yaml (patterns, deny) ──┘
metadata/overrides/<tk>/*.yaml (manual) ────────> merge: ingested < inferred < manual
                                                  └─> generators (unchanged): backend + one test per fact
                                                       └─> ledger refresh: pass -> promoted, fail -> parked
```

### 5.1 Layout in `tools/metadata`

| Path | Contents |
|---|---|
| `tools/metadata/<tk>/probe.py` | the prober (Python, offline, like ingestion) |
| `metadata/probe/<tk>/<version>.json` | the probe output, checked in |
| `tools/metadata/common/infer.py` + `tools/metadata/<tk>/rules.py` | the rule catalogue as code |
| `metadata/rules/<tk>.yaml` | data the rules read: the denylist, reference names, sample name rules, object factories, recipes, and pattern decisions from reviews |
| `metadata/inferred/<tk>/<Kind>.yaml` | output, in the **same schema as the manual overrides**, with `origin: inferred` and provenance pointing at the rule id and the probe key |
| `metadata/inferred/<tk>/<Kind>.evidence.json` | confidence and the probe excerpt, for review tables and docs |

### 5.2 Precedence

`svmeta.merge` gains one layer:

1. ingested;
2. inferred;
3. manual.

Records are matched by fact id. Manual records win field by field. A manual `deny:` list removes
inferred facts by id or by pattern. Strict matching stays: an inferred record must match an
ingested one, unless a rule is explicitly marked `virtual`.

### 5.3 Promotion

The generators already refuse facts without a test, and the ledger keys every result by fact id
and fingerprint. The proposed policy:
- **Inferred facts are generated and tested** like manual ones.
- **`ledger.py check` stays strict for manual facts.** For inferred facts, a failure is not a
  gate failure. The fact is written to `metadata/inferred/<tk>/_parked.json` with the failure
  (fact id, fingerprint, test, message) and excluded from the generated backend and typings.
- **A parked fact is retried** only when its fingerprint changes, meaning a new probe, a new
  rule or a new toolkit version.
- **Coverage counts as verified** only facts with a passing ledger entry, manual or inferred.
- **Qt C++:** only verified facts reach `gen_cpp.py`'s backend output; tests are generated for
  all candidates. Generating tests for candidates may need a separate test crate or table-driven
  tests (one test per kind iterating a fact table), so that 3,000 facts don't mean 3,000
  functions. See open question 3.

### 5.4 Determinism and review

- The probe output, rules data, inferred YAML and ledger are all checked in.
- `infer.py` is a pure function of `(ingested JSON, probe JSON, rules YAML)`. It writes
  byte-identical output on rerun, which is the same contract as ingest.
- A probe rerun produces a diff of the probe file. That diff, through `infer.py`, becomes the
  review table.
- Nothing nondeterministic runs at build time. The probe is an offline maintainer step, like
  ingestion.

### 5.5 Coverage report

`coverage.py` adds columns. Each fact is in one of these states:
- verified (manual);
- verified (inferred);
- parked (failed);
- pending (no test run yet);
- not covered.

It also adds a per-pattern table: denied, accepted, and open review rows. The A1 TODO becomes a
generated review-queue file instead of a hand-maintained batch list.

### 5.6 New toolkit versions and packs

- **Toolkit upgrade:**
  1. ingest;
  2. probe;
  3. infer;
  4. ledger refresh.

  Changed probe outcomes are exactly the facts whose behaviour changed, and they show up as
  review rows.
- **Packs** (`plans/research/CUSTOM-WIDGETS.md`):
  - **GObject packs** (libadwaita, app-authored widgets) and **Qt reflective packs** (QMetaObject)
    run the same prober against the pack's namespace or plugin. `gtk_prober.py --ns Adw` already
    works with no metadata.
  - The pack ships its `probe/`, `inferred/` and `ledger` fragments; its manual overrides are
    tags and policy.
  - **ADS** (a code pack) needs the Qt prober run against the pack's built library.
  - **An app's own widgets** can be probed in the app's dev loop, which annotates them for free.

### 5.7 The Rust port (M6)

Inference is merged before the generators run. The Rust generators read one merged kind table,
with `origin` and ledger state, just as they read manual overrides now. Probe, infer and merge
stay Python offline tools, under the same policy as ingestion. The only generator-side change is
"emit only verified inferred facts", a filter on the ledger the generators already read.

## 6. Where judgement still belongs (and stays manual)

- **Exposure policy and naming:**
  - tags and renames (Qt `present` for `show`);
  - primary binding names;
  - which non-accessor methods a Svelte author should see.
- **Event synthesis templates:** QMouseEvent/QKeyEvent, GTK controller emits, and dialog
  accept/reject.
- **Virtual and forwarded facts:** for example `Range.value` through its adjustment, and
  `TextView.text` through its buffer.
- **Semantics a probe cannot see:** "this prop is a reference to a sibling" across hosts, and a
  slot that replaces a lazily created default (`QMainWindow::menuBar()` would create one).

## 7. Estimates: projected verified coverage

The estimates assume:
- the rules above;
- the probers with the factory/recipe fixes listed in section 3.3;
- abstract bases and interfaces verified through a concrete subject. That is a rule: the first
  concrete subclass in the metadata, which today's `verify_subject` already supports.

| Fact type | GTK today | GTK projected | Qt today | Qt projected | WinUI (rules only, then prober) | AppKit |
|---|---|---|---|---|---|---|
| props (non-deprecated kinds) | ~270 annotated | ~470/543 (probe exact/identity 383/391 writable non-widget props on concrete kinds, + 56 abstract via subject, + read-only by read test; widget-typed props count as slots/references) | 268/522 | ~480/522 (345 exact today + factories) | 784 rw, 774 bindable by rule; ~80% verified with a Windows prober | KVC-settable ~60–70% with a macOS prober |
| signals: result/sync | 222 (reflection) | 222 | 72/219 | 219 (R1/R3) | 180 events, 37 sync by rule | target-action/delegate: manual |
| signals: native trigger verified | ~64 | ~120 (prop-set + action emit + event templates); ~190 delivery-only | ~80 | ~150 | prober | manual |
| methods | 52/1672 | ~900 (accessors ~850, getters/actions ~80) | 90/3021 | ~950 (727 accessors + 179 getters today, + factories) | ~134 own methods: judgement | judgement |
| one-slots | 34 | ~35 (30 probed on concrete kinds + bases) | ~18 | ~26 | 25 ContentProperty | NSView subviews: manual |
| many-slots | in the 34 | ~20 families (args need the family table) | ~5 | ~11 | Children/Items | manual |
| child attrs | 20 | ~25 (all layout-child/page props) | 13 | ~40 (74 index pairs, minus non-widget items) | 45 attached pairs | n/a |
| bindings | 17 | 17 primary + every notify prop bindable (~470) if generic `bind:<prop>` lands | 21 | 21 primary + NOTIFY props | all DP props | KVO props |
| text_prop | 2 | ~18 | 2 | ~12 | 3 | 1 |

Denominators exclude deprecated members and those M2 denies, where the table says so. The method
projections are below the full count because about 30% of methods are deprecated, protected or
denylisted.

## 8. Phasing

1. **GTK inferred layer.** This is the biggest win for the least code.
   - Move `gtk_prober.py` to `tools/metadata/gtk/probe.py` and check in `metadata/probe/gtk/4.22.4.json`.
   - Write `infer.py` with P1–P3, R1, R2, M1, M2 and S1, writing `metadata/inferred/gtk/`.
   - Add the merge layer in `svmeta`.
   - Fix the `origin: probe` validator mismatch.
   - Add parked facts to `ledger.py`.
   - Expected gain: about 850 accessor/prop facts, auto-promoted, with no agent work.
2. **Qt inferred layer.** Same steps with `qt_prober.py`, plus the object factories (QIcon, QFont,
   QRect/QPoint/QPixmap). The generated C++ tests need table-driven verification first, so the
   test binary does not grow by thousands of functions.
3. **Review tables.** `infer.py --review` emits pattern tables. `metadata/rules/<tk>.yaml` holds
   the decisions, with Haiku/Sonnet routing by confidence. Use it on the residual methods.
4. **Event templates and generic bindings.** Qt mouse/key synthesis and GTK controller emits
   upgrade delivery-only signals. Generic `bind:<prop>` for every notify prop needs a small
   renderer/ABI decision (open question 1).
5. **Packs.** Run the prober for libadwaita as the CUSTOM-WIDGETS pilot pack, then app-authored
   widgets in the dev loop.
6. **WinUI and AppKit probers** in the backends' test binaries on CI runners. Before that, the
   rules alone (DP, ContentProperty, attached, events) pre-fill the overrides.

## 9. Where this meets the non-GUI types lane

`plans/research/NON-GUI-TYPES.md` owns value and object types outside the widget tree. The two
lanes share:
- **The facts schema and the ledger.** Inferred facts on non-widget objects (TextBuffer,
  Adjustment, QTextDocument, which A1 reached as borrowed objects in gap G12) use the same
  layer.
- **Object samples and factories.** The prober's factory table (GFile, Adjustment, ListModel,
  QIcon, QFont, QDate) should come from that lane's value-type classification, not from a second
  list.
- **Method classification.** M1/M2 here is the widget-side instance of their auto-classifiers
  (`types_gir_classify.py`, `types_qt_classify.py`). One implementation should serve both.

This document does not cover non-widget kinds beyond using them as samples.

## 10. Open questions

1. **Generic bindings.** Should every prop with a notify be `bind:`-able through one generic,
   generated echo test per prop? Today bindings are a curated list. The notify mechanism is
   uniform on GTK, Qt and WinUI, so curation would only pick the primary name.
2. **Delivery-only signal facts.** Is a synthetic emission test (it checks delivery, listen-off
   and destroy, but not native causation) enough to call a signal verified? The proposal is to
   mark these `verified: delivery` in coverage and keep them distinct.
3. **Test volume.** Qt tests are generated C++ functions; 3,000 more facts would add compile
   time. Table-driven tests (one function per kind, iterating a fact table) or a separate test
   binary are needed. GTK tests are Rust and already cheap per fact.
4. **Parked facts in typings.** If parked facts are excluded from typings, an app sees fewer
   props than the toolkit offers, even though GTK reflection would reach them. Should typings
   show reflection-reachable but unverified props as `@unverified`?
5. **Probe in CI.** Should CI re-probe on the pinned toolkit and fail when the probe file
   changes, or should only maintainers run it? The proposal: maintainers only, like ingestion,
   with a CI check that the checked-in probe matches the toolkit version in the ledger.
6. **Exposure policy defaults.** Should non-accessor methods be exposed by default once their
   test passes (deny by pattern), or hidden by default (accept by pattern)? GTK already reaches
   everything through GI. Qt only has what is generated. Deny-by-pattern maximizes coverage;
   accept-by-pattern keeps the generated Qt backend small.
7. **Sanitizer runs.** Kinds with cross-op corruption (`Adw.Carousel`) need ASan or valgrind
   builds of the toolkit, or at least `MALLOC_CHECK_=3`. Is that worth setting up, or are such
   kinds simply left manual?

## 11. Decisions (user, 2026-10-17)

All seven open questions follow the recommendations:

1. **Generic bindings:** every prop with a notify is `bind:`-able, through one generated echo test per prop. Curation only picks the primary name.
2. **Delivery-only signals** count as verified, reported separately as `verified: delivery`.
3. **Qt test volume:** table-driven tests (one function per kind iterating a fact table), or a separate test binary.
4. **Parked/unverified props** appear in typings marked `@unverified`.
5. **Probe in CI:** maintainers run the probe, like ingestion. CI checks that the checked-in probe matches the toolkit version in the ledger.
6. **Exposure defaults:** deny-by-pattern on GTK (GI reaches everything anyway), accept-by-pattern on Qt (keeps the generated backend small).
7. **Sanitizer runs:** none for now. Kinds with cross-op corruption stay manual.
