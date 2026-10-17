# svelters

Svelte 5 components running in embedded QuickJS and rendered with native toolkits: GTK4 and QtWidgets now, egui later.

The project is being rebuilt (v2).
- Plan: [PLAN.md](PLAN.md)
- Status: [plans/STATUS.md](plans/STATUS.md)
- Research notes: [docs/notes/](docs/notes/)

The 2021 prototype is preserved at the `v1-legacy` tag.

## Layout

| Path | What |
| --- | --- |
| `packages/dom`, `packages/testhost`, `fixtures/` | JS: DOM-style shim, Node op-log TestHost, fixture apps |
| `crates/svelters-abi` | backend C ABI (`include/svelters_backend.h`) + Rust mirror |
| `crates/svelters-host` | QuickJS-NG host core |
| `crates/svelters-backend-log` | op-recording backend, used as a test oracle |
| `crates/svelters-gtk` | GTK4 backend, tables generated from `metadata/gtk` |
| `crates/svelters-qt-build` | generates and compiles the C++ Qt backend from a leaf crate's `build.rs` |
| `tools/metadata/` | metadata ingestion and generators (Python for now) |
| `metadata/` | generated element-kind JSON + `overrides/` annotations |
| `examples/` | counter app and its Qt leaf crate |
