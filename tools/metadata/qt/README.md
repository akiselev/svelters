Owned by the P4 lane, see plans/milestones/.

# Qt metadata pipeline

```
qdoc + PySide6 metaobject ──> ingest.py ──> metadata/qt/*.json
metadata/probe/qt/<qt>.json ─┐
metadata/rules/qt.yaml ──────┴─> infer.py ──> metadata/inferred/qt/*.yaml ─┐  qt_layers.py: ingested < inferred < manual
metadata/overrides/qt/*.yaml (manual: elements, tags, facts) ──────────────┴─> gen_cpp.Model ──> backend, svl_qt_verify.cpp, qt.d.ts
                                                                               └─> ledger.py refresh --toolkit qt: pass = promoted, fail = parked
```

| Step | Command | Who runs it |
|---|---|---|
| probe | `python3 tools/metadata/qt/probe.py` with PySide6 of the metadata's Qt version (`pip install PySide6==6.11.0`), offscreen, ~30 s | a maintainer, when Qt or the rules change; CI never probes |
| infer | `python3 tools/metadata/qt/infer.py` (`--check` in `pnpm test:meta`) | anyone; a pure function of metadata, probe, rules and the element list |
| generate | the `counter-qt` build runs `gen_cpp.py`; `common/gen_bindings.py`, `semantics/gen_semantics.py` | anyone |
| verify | `python3 tools/metadata/ledger.py refresh --toolkit qt` | anyone who changed facts |

- **Probe** (`probe.py`): one worker process per class. A probe op that kills the worker is recorded under
  `crashes` and skipped on the rerun, so a crashing class never kills the run. Deterministic (empty Qt settings
  home, fixed cwd, paths and addresses scrubbed; two runs give identical output).
- **Accept-by-pattern** (`metadata/rules/qt.yaml` `accept:`): only classes a manual override exposes as an element
  get facts, and only of the listed patterns (writable scalar/enum props, READ accessors of read-only props, signals
  with a property trigger or delivery-only emission, generic `bind:<prop>` through the NOTIFY). Everything else is a
  review row in `metadata/inferred/qt/_review.json`.
- **Merge** (`qt_layers.py`, used by `gen_cpp.Model`, `gen_bindings.py`, `coverage.py`, the docs frontend): an inferred
  record is dropped when the element has a manual or inherited record of that JS name, when the manual element
  `deny:`s it (`<section>:<glob>`), or when it is parked (`_parked.json`, matched by record fingerprint).
- **Tests**: inferred facts are table-driven (verify.cpp.in, section "table-driven tests"): one generic body per
  section (`tt_prop`, `tt_method`, `tt_signal`, `tt_emit`, `tt_binding`) and one row per fact. An inferred fact the
  generator cannot resolve or tabulate is `untestable` in the fact manifest and parked by the ledger, not an error.
- Scoped enums (`enum class`) are not inferred: the ingested enum table names their members unscoped.

## Observability facts (milestone R0)

| Step | Command |
|---|---|
| moc extract | `python3 tools/metadata/qt/ingest_moc.py --metatypes <qt>/gcc_64/metatypes` -> `metadata/qt/moc/properties.json` (BINDABLE, NOTIFY, CONSTANT, USER per Q_PROPERTY; the PySide6 dump has no BINDABLE) |
| observe probe | `python3 tools/metadata/qt/probe.py --section observe [--qtdir <qt>/gcc_64]` (C++ against a Qt SDK, offscreen, ~30 s; no PySide6) -> `metadata/probe/qt/observe/<qt>.json` |
| infer | `infer.py` writes `metadata/inferred/qt/observe/` (rules O2-O5, O8, O10 in `observe_rules.py`; data in `metadata/rules/qt.yaml` `observe:`) |

`tools/metadata/common/observe.py` merges the inferred and manual (`metadata/overrides/qt/observe/`) layers into
`metadata/observe/qt.json`. `gen_cpp.py` adds one `kObserveRows` row per fact the R1a backend can check (NOTIFY:
`change`/`silent`, CONSTANT: `refused`, no mechanism: `unsupported`) on an element that exposes the property or a
constructible non-GUI type; bindable, event and signal mechanisms wait for the R3 backend.
