Owned by the P3 lane, see plans/milestones/.

# GTK metadata pipeline

```
Gtk-4.0.gir + live GTK ──> ingest.py ──┐
metadata/probe/gtk/<ver>.json ─┐       │   merge: ingested < inferred < manual
metadata/rules/gtk.yaml ───────┴─> infer.py ──> metadata/inferred/gtk/*.yaml ──┤
metadata/overrides/gtk/*.yaml (manual, hand/agent) ───────────────────────────┘
                                                  └─> metadata/gtk/*.json ──> gen_backend.py ──> tables, slots, verify.rs (a test per fact), facts.json, gtk.d.ts
                                                                                └─> ledger.py refresh: pass = promoted, fail = parked
```

Plan and decisions: `plans/research/ANNOTATION-AUTOMATION.md` (section 11). Notes: `docs/notes/ai1-inference.md`.

| Step | Command | Who runs it |
|---|---|---|
| probe | `xvfb-run -a python3 tools/metadata/gtk/probe.py` (writes `metadata/probe/gtk/<version>.json`, ~6 min) | a maintainer, when the toolkit or the rules change; CI never probes |
| infer | `python3 tools/metadata/gtk/infer.py` (`--check` in `pnpm test:meta`) | anyone; a pure function of ingested records, probe and rules |
| ingest | `python3 tools/metadata/gtk/ingest.py` | anyone |
| generate | `python3 tools/metadata/gtk/gen_backend.py`, `common/gen_bindings.py`, `semantics/gen_semantics.py` | anyone |
| verify | `python3 tools/metadata/ledger.py refresh --toolkit gtk` (runs the generated tests through `tools/dev/cargo-slot`, needs a display) | anyone who changed facts |
| report | `python3 tools/metadata/coverage.py` | anyone |

## Layers

- **ingested**: GIR plus the live type registry (`ingest.py`).
- **inferred** (`metadata/inferred/gtk/<Kind>.yaml`): the same schema as the manual overrides
  (`override.schema.json`), `annotated.origin: inferred`, provenance pointing at the probe observation or the rule.
  `<Kind>.evidence.json` has the confidence and the probe excerpt. Only facts at `promote_at` (0.9) or above are
  written; the rest are counted in `_review.json` and stay unreviewed.
- **manual** (`metadata/overrides/gtk/<Kind>.yaml`): wins field by field, a nested field (a trigger, an op) is
  replaced as a whole. `deny: ["methods:set_*", "bindings:x"]` removes inferred facts of the kind.

## Promotion and parking

The generators emit a test for every annotated fact, inferred or manual. After `ledger.py refresh`:

- a passing inferred fact is **verified (inferred)**: coverage counts it, the typings drop `@unverified` for its property;
- a failing inferred fact is **parked** in `metadata/inferred/gtk/_parked.json` (fact, record fingerprint, test, failure
  message) and left out of the merged metadata, so out of the backend tables, bindings and typings. It is retried when
  its inferred record changes (a new probe or rule) or the toolkit version does (`infer.py` prunes the entry);
- a failing manual fact is still a gate failure (`ledger.py check`).

A signal whose trigger is a synthetic emission (`delivery_only: true`) is reported as `verified: delivery`: delivery,
listen-off and destroy are checked, not native causation.

## Version gates (V1)

The build links GTK 4.14 symbols only. Typed slot calls (gtk4-rs, feature `v4_6`) are inferred only when the setter,
the getter, the property and the kind are not newer than `typed_baseline` in the rules. Everything else is called through
GIRepository; the GIR `version` of a method (`since` in the metadata) makes its generated test skip on an older GTK.
The `gtk-4-14` CI job must stay green.
