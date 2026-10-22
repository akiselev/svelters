# LD2: `<style>` on the live loader path

Spec: coordinator assignment (2026-10-22). The module loader (Q1, `SourceProvider`) compiled `.svelte` without its
`<style>` and only warned "not applied when the app is loaded from source", so apps loaded from source, HMR from
source included, ran unstyled. LD2 compiles `<style>` with Q3w's `scoped_css` on the live path and sends the sheet
**as text**, as the Vite plugin does for egui (`registerStyles(id, { css })`, installed by the style engine).
Decided (coordinator): text, not a pre-parsed sheet (parsing costs 0.23 ms at startup and 17 µs per swap, and scoped
`:state()` resolution needs the host's state registry at install time). Lane LD2, branch `ci/ld2`.

## What landed

| File | Content |
|---|---|
| `crates/svelters-modules/src/source/transform.rs` | `Compiler::styles()`: the backend takes the scoped sheet as text (egui). Components are compiled with `CompileOptions::scoped_css` there; `Output::css` carries the `ScopedCss` (none when empty or `css="injected"`, as the Vite plugin skips them). The "not applied" warning is gone; the other backends get "applied from source on egui only" (their sheets need `@svelters/style/build`'s JS translator, which only the Vite build runs). |
| `crates/svelters-modules/src/source/mod.rs` | `svelters-style:<component id>`: a component's sheet module, `registerStyles(<id>, { css: <text> })` from `@svelters/native-runtime/style` plus `import.meta.hot.accept()` (HMR.md §3.3 item 6, H2's self-accepting style module). A component with a sheet ends with `import "svelters-style:<its id>"`. Only that component may import it (resolve refuses any other importer). `Inner::compiled` (the cache, now with the sheet) serves both modules. `invalidate` compiles a changed component and records whether its code and/or its sheet changed; `modules_at` then returns the sheet module for a sheet change and the component for a code change (or for no change, as before). Policy hook: a confined (extension) provider does not install a sheet with `:global` rules, with a warning (ARCH-SECURITY §6, Q2/Q3 row: an extension's global rules must stay within its own roots, which the engine cannot enforce yet). |
| `crates/svelters-modules/src/source/build.rs` (Q4, one hunk) | `Logical::id`: `svelters-style:<abs path>` becomes `svelters-style:app/<rel path>` in a release bundle (Q4 kept non-absolute ids verbatim, which would have put a machine path in the bundle). |
| `crates/svelters-egui/tests/live_styles.rs` (new) | `an_app_loaded_from_source_is_styled`: `TestApp::from_source`, the label's computed `color` is its `<style>` rule's, no `<style>` warning. `a_style_edit_restyles_without_remounting`: embed root from source with `dev`+`hmr`; after a click (state 1) the rule's colour is edited: the reload is in place, only `svelters-style:` modules re-ran, the label is the same handle, `count 1` is kept, the new colour applies, no `Mounted`/`Faulted` event. A markup edit afterwards is H1's component swap (no sheet module re-runs), the state is kept and the swapped component is still styled. |
| `crates/svelters-egui/tests/counter.rs` | `counter_from_source_runs_under_kittest` now asserts the opposite of before: no `<style>` warning, and CS5e's `.row { flex-direction: row }` lays the text edit and the label out in a row, as with the Vite bundle. |
| `crates/svelters-host/tests/modules.rs` | `styles_are_reported_not_silently_dropped` (gtk): the new "egui only" warning. `a_style_only_edit_reruns_only_the_sheet_module` (provider level, LogBackend): the component ends with the sheet import; the sheet module registers the scoped text and self-accepts; another module cannot import it; a colour edit selects only the sheet module, a markup edit only the component; an egui `.svb` build (Q4) names the sheet module `svelters-style:app/src/App.svelte`. |
| `plans/milestones/Q1-module-loader.md` | The `<style>`-from-source line points here. |

## Deviations, and why

- **egui only.** The other backends' sheets are translated to GTK CSS / QSS / decl rules by `@svelters/style/build`
  in the Vite plugin (Q3w); the live loader does not run that JS translator. They keep a warning. AppKit, WinUI and
  Qt are maintenance-only (CLAUDE.md), GTK is not a cadatomic target.
- **Scope class.** Vite's egui path scopes with a hash of the path relative to the repository (`scopeOf`); the live
  path uses Svelte's default `cssHash` of the absolute filename (Q3w's default when no `css_scope` is given). Both
  are self-consistent (the elements get the class the sheet uses); the class differs between the two paths.
- **Release bundles (Q4) are not yet machine-independent for styled egui components.** The module ids are logical,
  but the scope class (Svelte's default hash of the absolute filename) and the `registerStyles` key (the absolute
  path) inside the compiled code still derive from the build machine's path. Functionally fine; a follow-up can pass
  a `css_scope` and sheet key derived from the logical id.
- **Sheet id.** `registerStyles` is keyed by the component's module id (its absolute path), not the
  repository-relative path the Vite plugin uses: two extensions can both have `src/App.svelte`.
- **Style-only detection compiles in `invalidate`.** To pick the sheet module alone, the changed component is compiled
  when the change is reported (the result is cached, so the load that follows is a cache hit). A `<style>` edit whose
  JS output changes too (a setter declaration such as `spacing`, which lowers into the component's code, or a
  `<style>` placed before the markup in dev mode, which shifts its locations) is a component swap plus a sheet swap;
  H3 keeps the state.
- A removed `<style>` changes the component's code (no scope class), so it is a component swap; the old sheet stays
  registered but matches nothing.

## Tests run locally

- `cargo nextest run -p svelters-host --test modules` filtered to `styles_are_reported`, `a_style_only_edit`,
  `release_bundle` (3 pass, after merging origin/master with Q4).
- `cargo nextest run -p svelters-modules --lib` (28 pass).
- `cargo nextest run -p svelters-egui` on `live_styles`, `counter_from_source`, `hmr`, `hmr_state` (7 pass).
- Not run locally: the full workspace nextest, the gates, conformance (CI).
