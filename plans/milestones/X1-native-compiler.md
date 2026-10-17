# X1: experimental Rust Svelte compiler with native semantics

Status: IN PROGRESS (audit + foundation choice). Runs side by side with the existing JS pipeline;
nothing in the working Vite/@svelters/renderer path changes.

Goal: a Rust Svelte compiler whose output is JavaScript executed by rquickjs through the existing
host/ABI, with compilation driven by per-framework semantic specifications (elements, props,
events, bindings, slots, lifetime, styling) generated from `metadata/` plus hand-written rules.

## Done so far

- Audit of master @ 417093a: JS suite and the full Rust workspace suite (incl. GTK + Qt integration
  under Xvfb) pass in this environment (GTK 4.14 / Qt 6.4, older than the repo's 4.22 / 6.11).
- Portability fixes found by that run:
  - `QMainWindow.yaml` statusbar `get` used the options-only `findChild` overload (Qt >= 6.7);
    now the two-argument form.
  - The Qt verify generator had no bool case (`== 7` for every non-string prop); bool props are
    now checked with true/false.
- Qt annotations for the styling slice: `button.minimumHeight`, `button.default`,
  `entry.minimumHeight` (all 47 generated Qt verification tests pass).
- Hand-written semantic rules drafted: `metadata/semantics/rules/{gtk,qt}.yaml`.
- Compiler crate skeleton `crates/svelters-compiler` (diagnostics + a subset template parser),
  no JS parser dependency yet.

## Foundation decision (2026-10-17)

**Port Svelte's front end verbatim to Rust; no JS parser crates.**

- Rejected: oxc, swc (user direction: build cost; clean dev build of the parser alone on 4 cores:
  oxc 14 s, swc 17 s; biome 12 s, boa 9 s). rsvelte and svelte-rs (MIT) are byte-exact DOM ports
  built on oxc, so their code is not reused directly; they remain references.
- Ported (verbatim structure, typed output): Svelte `phases/1-parse` (minus `read/style.js`),
  acorn 8.19.0 (minus the TypeScript plugin), Svelte `phases/scope.js`.
- Not ported: Svelte's CSS parser. Native styling only needs a small subset (user direction), so
  `style.rs` parses selectors/declarations we can lower to native properties.
- Correctness oracle: real acorn and `svelte/compiler` output over their own test corpora,
  Svelte's runtime samples and our fixtures, diffed as JSON with a no-regression ratchet.
- Contracts: `docs/notes/native-compiler/modules.md`, `runtime-contract.md`.

## Work plan

Phase 1 (parallel agents, one worktree each): A acorn, B acorn regexp, C native style subset,
D Svelte template parser, E scope analysis, F oracle harness, G semantic specs + `BackendSemantics`,
H native JS runtime + hand-written compiled fixtures. Phase 2 (coordinator): analysis of runes,
native lowering to a typed IR, JS generation with source maps, Vite plugin, end-to-end fixtures on
the log backend, GTK and Qt; styling + named-slot extension; report.

## Environment notes (not committed)

GTK 4.14 lacks four >= 4.16 `*_get_type` symbols that the generated GTK tables reference; local
test runs link a stub archive (`RUSTFLAGS="-L <dir> -l static=gtkshim"`).
