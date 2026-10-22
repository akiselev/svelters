# Q3w: Q3's CSS port wired into `compile()`, the CLI and the Vite plugin

Spec: coordinator decision (2026-10-22): non-egui targets scope `<style>` with the Rust port of Svelte's CSS pipeline
(Q3, `plans/milestones/Q3-css-scope-port.md`) instead of stock Svelte in Vite. Constraints: `compile()` entry point
and one CLI flag only, `diag.rs` untouched (CX1 is editing it and `compile.rs`), byte identity with svelte@5.57.2
stays the gate, egui's CS5e path (`"engine": "backend"`, stock Svelte's scoped CSS sent to the engine) unchanged.
Lane Q3w, branch `ci/q3w`.

## What landed

| File | Content |
|---|---|
| `src/scoped_css.rs` (new) | `ScopedCss { code, hash, has_global, injected, scoped_elements }` (serialized camelCase) and `scoped_css(...)`: runs `svelte::css::compile_css` over the component already parsed and analysed by `compile()`. Hash: `CssHash::Fixed(css_scope)` when given, else Svelte's default `cssHash` of the filename. `css_unused_selector` warnings become `Diagnostics::warning`, a `css_*` error becomes `Diagnostics::error` (existing API; `diag.rs` untouched; CX1 already registers the `css_*` codes). `blank_native_blocks`: svelters' `@native <toolkit> { ... }` blocks are replaced by spaces (same byte length, newlines kept) before the port sees the sheet, found as `@svelters/style`'s `extractNative` finds them. |
| `src/compile.rs` (small) | `CompileOptions::scoped_css: bool` (default false). With it, the CSS step runs after scope analysis, before lowering; `CompileResult::css: Option<ScopedCss>` (skipped in JSON when `None`). When `css_scope` is not given, the port's hash also becomes the elements' scope class, so a caller such as the loader gets a self-consistent output. Without the option nothing changes. |
| `src/bin/svelters-compile.rs` | `--css`: sets `scoped_css`; the `--json` output gets `css: {code, hash, hasGlobal, injected, scopedElements}`. |
| `packages/native-runtime/vite.js` | Non-egui backends: the plugin no longer compiles the component with stock `svelte/compiler`. It runs `svelters-compile --css --css-scope <scope>`, translates `res.css.code` with the same `translateSheet` call (now `translatedSheets`, shared with `nativeSheets`) plus the `@native` blocks from `extractNative`, and registers the sheets as before. When nothing remains for the toolkit (a setter-only `<style>`, an injected one), it compiles once more without `--css`/`--css-scope` and takes that JS, so the elements get no scope class, as before. egui keeps `nativeSheets` (stock Svelte, CS5e). `nativeSheets` is unchanged in behaviour. |
| `tests/q3w_css_wiring.rs` (new) | Through `compile()`: seven Svelte css samples (DOM spec, HTML syntax) and five native fixtures for their backend (gtk, qt, appkit, egui) give the corpus's `css.code`, `hasGlobal`, hash and `css_*` warnings byte for byte; the native fixtures' JS carries the scope class. `@native` blocks are neither scoped nor pruned (`fixtures/styles`). Without the option, no `css` and no `css_*` diagnostics; the default hash equals the corpus's `default_hash_filename` and scopes the elements. All 45 corpus CSS errors are compile errors with Svelte's code, message and span. `svelters-compile --css --json` on a gtk fixture gives the corpus CSS. Plus a unit test of `blank_native_blocks`. |
| `docs/notes/native-compiler/styling-syntax.md` | One sentence: who scopes the CSS now. |

## Deviations, and why

- **egui still uses stock Svelte in Vite.** As decided (non-egui targets); the switch is the same call (`--css`, then
  `{ css: res.css.code }`), left for the egui owners so CS5e's path does not move under them.
- **Two compiler runs for a setter-only `<style>`.** The scope class goes on the elements only when the translated
  sheets are non-empty, which only the JS translator knows after scoping. The common case (sheets to register) is
  one run; a `<style>` that translates to nothing is compiled again without `--css-scope`, keeping the op logs (e.g.
  `fixtures/style-precedence`) unchanged. The first run's diagnostics are kept.
- **CSS errors are compile errors in Vite now.** Before, when stock Svelte rejected the component for any reason, the
  plugin warned and skipped the CSS. Now a `css_*` error fails the module (as stock Svelte would on a DOM target),
  and `css_unused_selector` shows as a Vite warning. Svelte's non-CSS rejections of native syntax no longer matter.
- **`@native` blocks are blanked, not removed.** The port rewrites the source by span, so the blocks are replaced by
  spaces to keep every offset (template, diagnostics). The scoped CSS then has blank runs where they were; the
  translator re-serializes, so the registered sheets are the same. A sheet without `@native` is byte-identical to
  svelte@5.57.2.
- **`--dev` reaches the CSS step** (Svelte's `dev` CSS output), where the plugin used to scope with `dev: false`.
  The translator ignores the comments dev mode adds.
- **`scoped_elements` is not used by the codegen.** The codegen still puts the scope class on every native element.
  Using the port's list would remove the class from elements no selector can match, which changes the op logs of
  the styled fixtures (`fixtures/styles`, `styles-qt`, `style-precedence`, the galleries), i.e. it is observable. It
  is exposed in `CompileResult::css` / `scopedElements` for a later lane that regenerates those logs.
- **No separate timing** for the CSS step; it counts in `lower_us` (keeps the `Timings` struct, which CX1 extends,
  untouched).

## Deferred

- egui on `--css` in Vite (above), and the loader (LD1/H1) setting `scoped_css` instead of calling
  `compile_component_css` itself (it then parses once).
- Codegen scoping only `scoped_elements` (needs op-log regeneration).
- On CX1's merge: `compile.rs` conflicts are expected in `run_component` (the `done` closure and the final
  return); keep `css` on `CompileResult` and the CSS step before lowering in both `compile` and `check_component`.

## Verification (local)

- `cargo nextest run -p svelters-compiler --test q3w_css_wiring` (6 passed) and the `scoped_css` unit test.
- `cargo clippy -p svelters-compiler --all-targets`: nothing in the touched files.
- Vite: a scratch script compared the old plugin path (stock Svelte, `nativeSheets`) with the new one on every
  non-egui styled fixture (`style-precedence`, `styles`, `styles-qt`, both galleries' `StyledDemo` and styling
  pages, `todo-mac`): the emitted JS and registered sheets are identical, with no new warnings.
- **Not run locally:** `tests/css_scope.rs` (the port is unchanged; CI runs it), the rest of the compiler tests, the
  fixture op-log suites and JS package tests (CI), conformance (the runner does not pass `--css` and ignores CSS;
  no sample is affected).
