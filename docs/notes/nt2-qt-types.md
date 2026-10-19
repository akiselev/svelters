# NT2: Qt non-GUI types, working notes

Plan: `plans/research/NON-GUI-TYPES.md` (N2). Milestone and completion record: `plans/milestones/NT2-qt-types.md`.

## Re-ingesting (only when the Qt version or the source set changes)

The checked-in ingest (`metadata/qt/types/ingested/`) is what everything else reads; re-ingest only to move Qt.

```sh
python3 -m venv /tmp/aqt && /tmp/aqt/bin/pip install aqtinstall
/tmp/aqt/bin/aqt install-doc linux 6.11.0 -O ~/.cache/claude-svelters/nt2/qtdoc
/tmp/aqt/bin/aqt install-qt linux desktop 6.11.3 linux_gcc_64 -O ~/.cache/claude-svelters/nt2/qt --archives qtbase icu
# PySide6 6.11.0: the venv of the AI1 probe (~/.cache/claude-svelters/venv-qt)
python3 tools/metadata/qt/ingest_types.py          # defaults point at the paths above
python3 tools/metadata/qt/types_qt_classify.py --write
python3 tools/metadata/qt/gen_types_js.py --write
```

The system Qt of this machine is 6.12 (Arch), CI uses 6.11.3: the generated C++ is compiled against both
(`g++ -fsyntax-only` with the aqt headers) before pushing. qdoc 6.11.0's `qtwidgets.index` is byte-identical to the
one the widget metadata was ingested from; `qtcore.index` and `qtgui.index` differ (a distro build) and are recorded
by hash in the ingest.

## Traps

- `pnpm` scripts call bare `pnpm`; with only corepack installed put `~/.cache/claude-svelters/bin` (a `corepack pnpm`
  shim) first on `PATH` for `pnpm -r build` / `pnpm test`.
- The member table of the generated layer is binary-searched in C++: it must be sorted by the numeric
  `svl_name_kind`, not by its spelling (a first version sorted `SVL_METHOD` before `SVL_PROP`).
- The host pads omitted optional arguments with null (`op_call`), so every thunk trims trailing nulls beyond the
  required count before dispatch.
- Element apps probe getter names and accept "unknown" errors (`fixtures/borrowed-walk-qt`): the metaobject
  fallback reports a missing member as `unknown method ...`.
- qdoc lists protected reimplementations only for some classes: abstractness misses private overrides
  (QAbstractListModel), so `concrete:` in the overrides lists the exceptions.
