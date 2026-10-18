# NT0: non-GUI types pilot (GTK), working notes

Plan: plans/research/NON-GUI-TYPES.md (N0).

## Progress
- ABI v2.5 header + Rust mirror + safe wrapper (construct, call_static, SHARED, SVL_T_TYPE, STATIC/CTOR). Qt template gets explicit nullptr.
- Host: ops construct/callStatic/typeChain (core/statics.rs); VALUE handles remember their kind. `cargo check -p svelters-host` passes.
- `describe(SVL_KIND, 0, kind)` answers the qualified name and the nearest bound ancestor kind (documented in the header).
- Q9 (coordinator, user): named ES exports, one file per class, sideEffects:false, no registration side effects, manifest + signature closure. Recorded in the plan (3.5, 10). Consequence: generated classes reference each other by import (the signature closure), a returned object is wrapped as its declared class, there is no global kind registry.
