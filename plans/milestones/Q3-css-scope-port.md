# Q3: Svelte's CSS analyze/prune/scope ported to Rust (`crates/svelters-compiler/src/svelte/css`)

Spec: `plans/research/TRACKS.md` (LOADER track, Q3 row), `plans/research/CSS.md` §2.6 and §5 ("Q3 (re-sized)"):
`<style>` in a component compiles with no Node, and the scoped CSS (selectors rewritten with the `svelte-<hash>`
class, `:global()`, unused-selector pruning and its `css_unused_selector` warnings, keyframes scoping, the hash) is
byte-identical to svelte@5.57.2 on the fixture corpus. Lane Q3, branch `ci/q3`, stacked on `ci/cs0` (STYLE CS0).

## What landed

| File | Content |
|---|---|
| `svelte/css/parse.rs` | Port of `phases/1-parse/read/style.js`: Svelte's own character-level CSS parser (rules, at-rules, blocks, declarations with raw values, selector lists, the nine simple selector types, combinators, `<!-- -->` and comments, identifier escapes, the `REGEX_NTH_OF` / percentage / matcher regexes as hand-written matchers). Byte spans. |
| `svelte/css/ast.rs` | Svelte's `AST.CSS` (span-preserving). Rules, complex and relative selectors carry an `id` into the metadata tables. |
| `svelte/css/analyze.rs` | Port of `css-analyze.js`: `parent_rule`, `is_global(_like)`, `is_global_block`, `has_local/global_selectors`, `used` for global selectors, keyframes, `hasGlobal`, and all 14 `css_*` validation errors. |
| `svelte/css/prune.rs` | Port of `css-prune.js`: backward/forward selector matching against the template (ancestors, descendants, `+`/`~` siblings through `{#if}`/`{#each}`/`{#await}`/`{#key}`/slots/components/snippets/render tags, `<select>`/`<option>`/`<selectedcontent>`), attribute/class/id matching with the possible values of expressions (`clsx` arrays/objects, `&&`, ternaries, the 20-value bail-out), `:is/:where/:has/:not/:root/:host/:global`, nesting. Marks `used` and `scoped`. |
| `svelte/css/warn.rs` | Port of `css-warn.js`: `css_unused_selector`, honouring a `<!-- svelte-ignore css_unused_selector -->` before `<style>` (runes and legacy syntax of `extract_svelte_ignore`). |
| `svelte/css/transform.rs` | Port of `3-transform/css/index.js` `render_stylesheet`: scoping class / `:where(.h)` specificity bumps, `:global` removal, `/* (unused) */` and `/* (empty) */` comments (with `*/` escaping), selector-list pruning, keyframes and `animation(-name)` renaming, `-global-` keyframes, `:global {...}` blocks, dev mode, and the minified form for injected styles. |
| `svelte/css/magic_string.rs` | The subset of magic-string 0.30.21 the transform uses, with its chunk semantics (insert attachment, `overwrite` vs `update`, `remove` clearing inserts). |
| `svelte/css/template.rs` | The template metadata upstream's analysis visitors write and the pruner reads: `analysis.elements`, zimmerframe paths, `RenderTag`/`Component` `metadata.snippets` (resolved through the existing scope port), `SnippetBlock.metadata.sites`. |
| `svelte/css/mod.rs` | `compile_css(src, &root, &analysis, &CssOptions) -> Result<Option<CssOutput>, CompileError>` and `compile_component_css(src, native, runes, &opts)` (parse + TS strip + scopes + CSS). `CssOptions { hash: CssHash::{Default{filename}, Fixed}, injected, dev }`. `CssOutput { code, has_global, hash, injected, warnings, scoped_elements }`. `hash()` / `css_hash()`: Svelte's djb2 hash and default `cssHash`. Exported-snippet `hasGlobal`. |
| `svelte/parse/read/style.rs` | In HTML syntax (`!native`) the `<style>` content is now read by the ported CSS parser, as upstream's parser does: CSS syntax errors are parse errors and the block ends where upstream's ends. Native syntax keeps the raw capture (the native subset parser `crate::style` reads it). |
| `tools/oracle/gen-css.mjs` | Corpus generator (Node, real svelte 5.57.2): every `.svelte` with a `<style>` in Svelte's test samples (the 184 css samples with their `compileOptions`, plus runtime-*, snapshot, validator, compiler-errors, preprocess, migrate, ...) and in this repo's `fixtures/` and `examples/` (native `bind:` blanked as the Vite plugin does when Svelte rejects them). |
| `tests/css_scope/corpus.json` | 368 cases (45 CSS errors), checked in (~0.5 MB; Svelte's samples are MIT). Per case: `css: 'external'` code, `hasGlobal`, `css_*` warnings with positions; `css: 'injected'` minified code; CSS error code/message/span; the `cssHash` arguments and Svelte's default hash. |
| `tests/css_scope.rs` | The comparison: byte-identical code (external and injected), `hasGlobal`, warnings (code, message, UTF-16 span), errors (code, message, span) and default hashes. No Node. **368 / 368 pass.** |

## Deviations, and why

- **Not on cssparser / `svelters_style`.** Svelte's grammar is its own (raw `prelude`/`value` strings, `<!-- -->`,
  its identifier rules, its error positions), and the transform rewrites source text by span, so byte identity needs
  the exact character-level parser. cssparser tokens would give different spans and accept/reject different input.
  Nothing is needed from CS0's crate for Q3. The engine sees the scoped output as an ordinary sheet (CSS.md §2.6).
- **No source maps** (`css.map`) and no `sourceMappingURL` comment (upstream adds one only in dev with injected
  styles). The generator skips the injected form in dev.
- **Metadata in side tables** (`Meta`, by node id, `Cell`s) instead of on the nodes. Upstream's pruner makes modified
  copies of relative selectors that share the original's `metadata` object; here a `Rel` view carries the original's
  id (or none for the synthetic `&` / `*`, whose upstream metadata is a never-read module-level object).
- **`content.comment`** is found again from the root fragment (the Rust parser does not keep it); the
  `svelte-ignore` check covers `css_unused_selector` only, and the unknown/legacy-code warnings of
  `extract_svelte_ignore` are not emitted here.
- **Exported snippets**: upstream finds top-level snippets in the module scope; the Rust scope port keeps them in the
  template scope, so a top-level `{#snippet}` of the exported name also sets `hasGlobal`.
- **`String.fromCodePoint` > U+10FFFF** in an identifier escape throws a RangeError upstream (not a CompileError);
  here it is `css_expected_identifier`.
- **Not wired into `compile()` or the CLI.** CX1 and CS5e are editing `compile.rs`, `diag.rs` and the bin; the
  loader (LD1/H1) and CS5 call `svelte::css::compile_component_css` / `compile_css` directly. The Vite plugin still
  scopes with stock Svelte (`nativeSheets`); switching it is a one-call change once the CLI exposes the CSS.
- **`scoped_elements`** (the elements upstream gives the hash class, by start offset) is exposed but the codegen
  still adds `--css-scope` to every native element (unchanged; harmless, the class only adds matches the pruner
  already allowed).

## Notes for the next lanes

- A native component (`native: true`) can be compiled with `compile_component_css(src, true, ...)`: element names
  are the native tags, matched case-insensitively like HTML names. Native-only CSS (`@native gtk {}` blocks, setter
  properties) parses (at-rules and declarations are opaque to Svelte's parser).
- Regenerating the corpus: `UPSTREAM=/path SVELTE_PKG=/path/to/svelte node tools/oracle/gen-css.mjs` with a
  checkout of sveltejs/svelte @ `svelte@5.57.2` in `$UPSTREAM/svelte`. On a svelte pin bump, regenerate and port the
  diff of the five upstream files.

## Verification (local)

- `cargo test -p svelters-compiler --test css_scope`: 368 / 368.
- `cargo test -p svelters-compiler --lib svelte::` (parse, scope and css unit tests): 65 passed.
- `cargo check -p svelters-compiler`; clippy on the crate (style lints only, not gated).
- **Not run locally:** the full workspace nextest, the other compiler integration tests (fixtures compile in native
  syntax, unaffected by the `read_style` change), the conformance samples (the DOM backend parses in HTML syntax, so
  a `<style>` Svelte rejects is now rejected at parse time like upstream; every runtime sample with a `<style>` is in
  the corpus and parses as upstream does), the oracle suites (`oracle_svelte` needs the uncommitted
  `target/oracle` corpus; it skips in CI), conformance. GitHub CI is the gate.
- The local shell rule (`zsh -lic`) could not be followed: this agent's sandbox refuses `zsh -lic` wrappers, so
  cargo-slot and Node 22 (`~/.nvm/versions/node/v22.23.4/bin/node`) were called directly.
