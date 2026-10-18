# NT0: non-GUI types pilot (GTK), working notes

Plan: plans/research/NON-GUI-TYPES.md (N0).

## Progress
- ABI v2.5 header + Rust mirror + safe wrapper (construct, call_static, SHARED, SVL_T_TYPE, STATIC/CTOR). Qt template gets explicit nullptr.
- Host: ops construct/callStatic/typeChain (core/statics.rs); VALUE handles remember their kind. `cargo check -p svelters-host` passes.
- `describe(SVL_KIND, 0, kind)` answers the qualified name and the nearest bound ancestor kind (documented in the header).
- Q9 (coordinator, user): named ES exports, one file per class, sideEffects:false, no registration side effects, manifest + signature closure. Recorded in the plan (3.5, 10). Consequence: generated classes reference each other by import (the signature closure), a returned object is wrapped as its declared class, there is no global kind registry.

## NT0b (finish pass)

- origin/master (5714bf7a) was already contained in the branch: merge was a no-op; AI1 had not landed on master.
- 4.14 guard: `since_guard()` in `tools/metadata/gtk/gen_types_extra.py` wraps every property/method/signal that carries a `since` in a run-time check (`Harness::has_prop`, `resolve_method`, `has_signal`); absent members print `SKIP` and pass. Members without `since` stay strict.
- `ingest.py --namespace`: not done. `tools/metadata/gtk/ingest.py` is the element-kind (widget) ingester for Gtk-4.0 only (kind files, enum nicks, widget-specific rules); a namespace mode would need a second record schema and none of the classifier's closure/mutator logic would move into it. The classifier keeps reading the GIR via `gir.py`; the checked-in `metadata/gtk/types/*.json` are the product.
