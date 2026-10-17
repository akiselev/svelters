# P0 - workspace setup

Done by the coordinating agent before the lanes start, so the lanes don't collide.

- Tag the 2021 head as `v1-legacy`, then delete the old tree (app, gtk, qt, lib, src, the old build scripts, the submodules, and the unrelated docs). The tag preserves it, and `docs/notes/gtk-v1-archaeology.md` points into it with file@sha refs.
- Root `Cargo.toml`: a workspace with edition 2024, resolver 3, and `rust-toolchain.toml` pinned to the installed stable (1.97).
- Root `package.json` + `pnpm-workspace.yaml` with a pinned `packageManager` (pnpm 10.33). Node 22 is installed.
- Empty lane directories with README stubs:

```text
packages/dom/            P1  @svelters/dom shim + JS host-op interface
packages/testhost/       P1  Node op-log TestHost
fixtures/                P1  counter, keyed-list, nested/slots, events; expected op logs
crates/svelters-abi/     P2  C header + Rust types for backend-abi.md
crates/svelters-host/    P2  rquickjs host core
crates/svelters-backend-log/ P2  backend that records ops (test oracle)
crates/svelters-gtk/     P3  GTK4 backend
crates/svelters-qt-build/ P4 leaf-crate build helper (runs the generator, compiles the C++)
tools/metadata/common/   P0  schema v0 module: load/validate/merge overrides (coordinator-owned)
tools/metadata/gtk|qt/   P3/P4 ingestion + generators (Python for now)
metadata/{gtk,qt}/       P3/P4 generated element-kind JSON (checked in)
metadata/overrides/      P3/P4 manual/agent annotations (YAML)
examples/counter/        P5  bin + examples/counter-qt leaf crate
```

- Copy the ABI header draft from `plans/architecture/backend-abi.md` into `crates/svelters-abi/include/svelters_backend.h` so P2–P4 compile against the same file from the start.

- Freeze `svelters-element-kind/v0`: promote the schema sketch from `docs/notes/widget-metadata-ingestion.md` §4 into `plans/architecture/widget-metadata.md#schema`. Write `tools/metadata/common/` (load, validate, override merge, deterministic JSON writer) so both lanes share it. Schema changes during P go through the coordinator.
