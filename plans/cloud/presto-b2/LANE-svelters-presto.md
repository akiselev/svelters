# Lane SVELTERS-PRESTO: the svelters backend on presto

Owns `crates/svelters-presto/**` and `metadata/semantics/presto.json` (generated). Implement the backend ABI
(`crates/svelters-abi`, `plans/architecture/backend-abi.md`; svelters-egui's `ops.rs` is the template) on
`presto_core::Document`: create/insert/remove/destroy/set_prop (typed, by id)/get_prop/listen (with the
passive flag)/call_sync/observe/request_frame/wake/style/bounds/describe, plus `instantiate`/`destroy_range`
natively (lane COMPILER/RUNTIME define the ops). Generate `presto.json` from presto-widgets' and
presto-core's `KindSpec`s the way svelters-egui's `spec.rs` generates `egui.json` (stable ids with
fingerprints, change classes, passive/cancelable, a11y, capabilities `ids templates native_transitions
native_motion css_precompiled`), with a staleness test; add presto to `gen_props.py`/`gen_utilities.py` and to
the compiler's `SpecBackend::builtin`; replace the hard-coded `"egui"` checks with capability checks
(`svelters-modules/src/source/transform.rs:81`, `packages/native-runtime/vite.js:376,451`, testhost:437,
compiler `backend.rs:59`, `kinds.rs:141`). The embed model (roots, slots, settle, faults) as svelters-egui's
`embed/` has it, minus egui. Event-time delivery: `Document::dispatch` → JS through the host's "dispatch and
drain" entry (lane RUNTIME). A runner binary `svelters-presto-run` that opens a window with a Svelte app
through the `presto` facade (lane APP-TEST). Tests (written, not run): the counter fixture's op log equals
egui's; the svelters examples using Tier 0 kinds.
