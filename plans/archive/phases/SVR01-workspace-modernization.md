# SVR01 - workspace and dependency modernization

References: `plans/architecture/runtime-stack.md`.

## Goal

Create the clean v2 workspace/tooling shell without yet implementing the new renderer.

## Work

- Move active Rust crates to edition 2024 and workspace resolver 3.
- Add a pinned stable `rust-toolchain.toml`; centralize workspace dependencies/features.
- Establish `crates/`, `packages/`, `tools/`, `fixtures/`, and `plans/` conventions.
- Replace Yarn v1 with a pnpm workspace and pinned `packageManager` field.
- Make JS packages ESM-only (`type: module`).
- Add current Node LTS CI/dev requirement satisfying Vite 8.
- Add base CI for Rust fmt/clippy/test/nextest and JS format/lint/test/typecheck.
- Adopt Prettier 3 and current Svelte-aware linting. Prefer a small ruleset; do not spend the phase recreating the old large ESLint configuration.
- Add `cargo-deny` policy and dependency/license inventory.
- Add representative build-size benchmark harness placeholders (`hello`, `counter`, `editor-shell`).

## Old code treatment

Keep old implementation accessible through the legacy tag and selected fixtures. Remove git-submodule participation from the active v2 workspace. Do not delete the submodule declarations in this phase unless SVR00's preservation artifacts are merged and verified.

## Non-goals

No Svelte 5 runtime implementation, no QuickJS migration, no GTK/Qt port.

## Acceptance

A fresh clone can bootstrap with documented Rust + Node + pnpm commands. Empty/new v2 crates and JS packages build/test without requiring the old Svelte/rquickjs submodules or GTK/Qt. CI has no dependence on Node 12/Yarn 1/Rust 2018 behavior.
