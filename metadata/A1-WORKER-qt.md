# A1 worker brief: Qt annotation batch

You annotate one batch of QtWidgets classes for svelters (Svelte 5 apps in embedded QuickJS that
render native GTK4/Qt6 widgets through a fixed C ABI). Your batch is appended at the end of this
brief. Work only on it. Every fact you annotate must be backed by a passing generated test; a
fact you cannot verify is left out and listed in your batch's status line.

## 0. Rules

- Builds: always `export CARGO_BUILD_JOBS=2 CARGO_BUILD_BUILD_DIR=target/build`, nextest
  `--test-threads 2`. Never omit `CARGO_BUILD_BUILD_DIR` (the global cargo config shares a build
  dir across worktrees and corrupts artifacts). Run only your batch's tests until the final gate run.
- Edit only: `metadata/overrides/qt/<Class>.yaml` for your batch's classes, your gallery page
  `fixtures/gallery-qt/src/pages/NN-<family>.svelte`, your `gallery_page!` line(s) in
  `examples/counter/tests/gallery.rs`, your batch's `Status:` line in `metadata/A1-TODO.md`, and the
  generated outputs the commands below rewrite (commit them). Qt C++ is changed only through the
  generator, and you do not change the generator or its templates. If the framework is missing
  something, leave the fact out and name the gap from `metadata/A1-GAPS.md` (or describe a new gap
  in your final report).
- Never expose blocking APIs: `exec()`, `QDialog::exec`, `QMessageBox::information/warning/...`,
  `QFileDialog::getOpenFileName` and friends, `QMenu::exec`, nested event loops. `open()`/`show()` are fine.
- Don't push. Don't touch master. Commit on your own branch; messages end with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01Lc9bA3PFUNUTFSuo1BzjWS
  ```
- Notes: append findings (surprises, ownership traps, failed facts with the failure) to
  `docs/notes/a1-<batch-id>.md` as you go. Keep it short.

## 1. Setup (once)

```sh
git merge --ff-only worktree-agent-af26bb86d3e5158d3 || git reset --hard worktree-agent-af26bb86d3e5158d3   # the A1 branch
pnpm install && pnpm -r build          # fixture bundles, needed by the gallery tests
```

When the lead asks you to sync later: `git merge worktree-agent-af26bb86d3e5158d3`; on conflicts
in generated files take either side and rerun the commands of section 3.

## 2. Where things are

- `metadata/qt/<Class>.json`: ingested classes (qdoc + QMetaObject + PySide typesystem) with typed
  property defaults/ranges and a lifetime block; the enum table is `metadata/qt/_enums/Qt.json`.
  Read them for property names/types/accessors, signal signatures (`metaobject_signatures`),
  method overloads and ownership facts (`ownership`). Never edit them.
- `metadata/overrides/qt/<Class>.yaml`: your annotations; the file name is the class name. Schema:
  `tools/metadata/qt/override.schema.json` (unknown keys are errors). Examples: `QLineEdit.yaml`
  (props, binding, signal, synthetic key-press), `QPushButton.yaml`, `QMainWindow.yaml` (named
  slots with ownership effects), `QWidget.yaml` (layout-managed `box`), `QScrollArea.yaml`,
  `QSpinBox.yaml`/`QComboBox.yaml` (typed props from the ABI v3 value model).
- Generator: `tools/metadata/qt/gen_cpp.py` (run by `examples/counter-qt/build.rs`, so a cargo
  build regenerates the C++). Typings: `packages/renderer/types/qt.d.ts`.
- Ledger: `metadata/verified/qt.json`. Coverage: `metadata/coverage/qt.md`.
- References: `docs/notes/signals-and-slots-semantics.md` 2.5 (slot op descriptors),
  `docs/notes/fork-bindings.md` 6 (bindings), `docs/notes/ownership-and-lifetimes.md` 2.4 (Qt traps),
  `docs/notes/s1-handoff.md` (value model: which types convert), `docs/notes/x2-sync-frame.md`
  (sync signals, `synthetic: key_press`), `metadata/A1-GAPS.md`.

How Qt exposure works: only annotated facts exist in the generated backend. An element is a tag
plus a class to construct; its props, signals, methods and slots are listed explicitly.
`inherit:` blocks on a base class (`QAbstractButton.yaml`, `QAbstractSlider.yaml`, ...) give their
facts to every element whose class derives from it (unless the element declares the same JS name
or lists it in `exclude:`); each inherited fact is tested on every element that gets it.

Tag names: use the GTK tag when the widget is the same concept and shared props mean the same
(`button`, `entry`, `label`, `box`, `window`, `scrolledwindow`), otherwise the lower-case class
name without Q (`checkbox`, `slider`, `spinbox`, `tabwidget`). Tags shared with GTK must agree on
bindings (`gen_bindings.py` checks).

## 3. Annotation format and the loop

Every record needs `provenance` (string, or list of `{origin, ref}` with origin
`qdoc|metaobject|typesystem|uic|probe|manual|docs|source`).

```yaml
# metadata/overrides/qt/QAbstractButton.yaml  (an inherit-only file: no elements)
inherit:
  props:
    - { js: checked, native: checked, suppress_signals: true, provenance: "QAbstractButton::checked (qdoc)" }
  signals:
    - js: toggled
      native: toggled
      connect: "toggled(bool)"                 # must be one of the metaobject signatures
      args: [{ name: checked, type: bool }]    # names are qdoc parameter names
      trigger: "$w->setCheckable(true); $w->setChecked(($i % 2) == 0)"   # C++ making Qt emit it; $w = widget, $i = 0,1,2
      expect_args: ["true"]                    # first emission's args as strings (bool -> true/false, ints, %g floats)
      provenance: "QAbstractButton::toggled(bool) (qdoc)"
  methods:
    - { js: toggle, native: toggle, expect: "$w->isChecked() || !$w->isCheckable()", provenance: "QAbstractButton::toggle() slot (qdoc)" }
```

```yaml
# metadata/overrides/qt/QCheckBox.yaml
elements:
  - tag: checkbox
    construct: { class: QCheckBox }
    props:
      - { js: tristate, native: tristate, provenance: "QCheckBox::tristate (qdoc)" }
    bindings:
      - name: checked
        direction: two_way
        property: checked                      # a JS prop name of this element (inherited ones count)
        type: { ref: bool, nullable: false }
        change: { signal: toggled, role: any_change, value: arg0 }
        write: { natively_normalizes: false }
        echo: { strategy: host_suppressed, compare_before_set: true, read_back: false, sync_signals_on_set: ["toggled(bool)"] }
        lifetime: { on_native_death: end_binding, on_stale_update: warn_noop, on_destroy: idempotent_disconnect }
        provenance: "setChecked emits toggled only on change (qdoc)"
```

- Props: `{js, native}`; type, getter and setter come from the metadata. Scalars, strings, enums
  (as names, from the enum table), flags, records, dates and string-form boxed types convert (ABI
  v3). `sample:` overrides the test value. Props without a setter are read-only (tested as such).
  `target: layout` addresses the element's composite layout.
- Echo suppression: `suppress_signals: true` on the prop; the backend drops this handle's posts
  while JS writes it (a per-handle flag, never QSignalBlocker). Every two-way binding with
  `strategy: host_suppressed` needs it, and the generated test checks that a programmatic set posts
  nothing while a native change (the element's first signal trigger) still does.
- Signals need a `trigger`. Overloaded signals: pick one `connect` signature and list the others in
  `collapses` so they are not delivered twice. `synthetic: close_request | key_press` are backend
  event filters (no trigger).
- Methods: `params` lists neutral type refs of the overload (`[i32, string]`); the test calls it with
  `args` (default 10, 11, ... / true / 0.5 / "x") and checks `expect` if given. Methods whose
  params are not scalar get a resolution test only; prefer adding an `expect`able scalar overload.
- Slots (`children`): copy the shape from `QMainWindow.yaml` (one) or `QWidget.yaml` (many). Many
  slots need `insert` with a `before != null` variant (`args: [index, child]`), `remove`, `index_of`,
  `list: {native: count, item: ...}`; `remove` may use `index` (e.g. `QTabWidget::removeTab(int)`,
  args `[index]`) and `then:` steps (`{native: "QWidget::setParent(QWidget*)", receiver: child, args: [null]}`)
  so the removed child ends unparented. Constant args: `"const:QString()"`. `effects` must state
  what Qt really does (`old: deleted|unparented`, `child: deleted|unparented`): probe it, the
  generated ownership tests check it with QPointer.

The loop:

```sh
python3 tools/metadata/qt/gen_cpp.py --config tools/metadata/qt/gen.dev.toml --out /tmp/$USER-qtgen   # fast check: schema, resolution, testability
xvfb-run -a cargo nextest run -p counter-qt --test-threads 2 --no-capture 2>&1 | grep -E "FAIL|tests,"  # builds and runs all generated Qt tests
python3 tools/metadata/common/gen_bindings.py            # after adding bindings
```

A failing test means the annotation is wrong or Qt behaves differently: fix the fact, or drop it
and record it in your status line. Never weaken a generated test.

## 4. Gallery page

- File `fixtures/gallery-qt/src/pages/NN-<family>.svelte` (NN from your batch). First line:
  `<!-- kinds: QClass1 QClass2 ... -->` (read by the coverage report).
- Show each element of the batch with its annotated props and signals; keep it compact.
- Only annotated props exist on Qt; numbers and booleans as expressions (`value={5}`); events are
  `on<js>` (`ontoggled`, `onchanged`). Text children only work on `label` and `button`.
- Dialogs: never open one in the initial page state (the screenshot is of the main window).
- Each two-way binding gets an `echo <name>: {count}` label counting change events, and the app
  writes the bound state once after mount (`$effect`). The screenshot test fails if any `echo ...`
  label is not 0. See `00-overview.svelte`.
- Add `gallery_page!(qt_NN_family, "qt", "NN-family", ["QFoo", "QBar"]);` to
  `examples/counter/tests/gallery.rs`.
- Check: `pnpm --filter @svelters-fixtures/gallery-qt test` and
  `xvfb-run -a cargo nextest run -p counter --test gallery --test-threads 2 -E 'test(/qt_NN/)'`;
  look at `target/gallery/qt-NN-family.png` (Read tool) and fix layout problems.

## 5. Finish

```sh
SVELTERS_TEST_THREADS=2 python3 tools/metadata/ledger.py refresh --toolkit qt
python3 tools/metadata/coverage.py
pnpm test && pnpm docs:test
xvfb-run -a cargo nextest run --workspace --test-threads 2        # full gate, once at the end
```

Update your batch's `Status:` line in `metadata/A1-TODO.md` to `done (<n> facts verified;
dropped: ...; blocked: gap:Gx for ...)`. Commit everything (overrides, generated typings, ledger,
coverage, page, test line, notes).

Final report (your last message, short): branch name and HEAD, facts verified (count by class),
facts dropped with the reason, facts blocked by which gap, new gaps you found, gate results,
screenshot path.
