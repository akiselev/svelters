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

## Foundation decision (pending)

- oxc and swc are rejected: too heavy to build (clean dev build of the parser alone, 4 cores:
  oxc 14 s, swc 17 s; biome 12 s, boa 9 s).
- rsvelte and svelte-rs (both MIT) are byte-exact DOM ports built on oxc; being evaluated for
  reusable parser/scope code.
- Leading proposal: a verbatim Rust port of Svelte's own parser (`phases/1-parse`, ~4.4k lines
  + generated entities table), acorn (~6.3k lines, typed ESTree structs) and `phases/scope.js`
  (~1.5k lines), with HTML-only rules behind a native-mode flag and an AST-diff test against
  `svelte/compiler`'s `parse()`.

## Environment notes (not committed)

GTK 4.14 lacks four >= 4.16 `*_get_type` symbols that the generated GTK tables reference; local
test runs link a stub archive (`RUSTFLAGS="-L <dir> -l static=gtkshim"`).
