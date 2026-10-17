# P0 - workspace setup

Done by the coordinating agent before the lanes start, so the lanes don't collide.

- Tag the current 2021 head as `v1-legacy`. Move `app/ gtk/ qt/ lib/ src/ build*.js svelte.config.js tsconfig.json package.json yarn.lock Cargo.toml .eslintrc.yml shell.nix` into `legacy/`, and drop the submodules from the active tree. They're preserved by the tag.
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
crates/svelters-qt-build/ P4 C++ backend generator/compiler (hand-written C++ in P)
examples/counter/        P5  bin + examples/counter-qt leaf crate
```

- Copy the ABI header draft from `plans/architecture/backend-abi.md` into `crates/svelters-abi/include/svelters_backend.h` so P2–P4 compile against the same file from the start.
