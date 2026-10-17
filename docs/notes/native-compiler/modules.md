# NC1 compiler: module map, ownership and contracts

Crate `crates/svelters-compiler` (experimental, see `plans/milestones/NC1-native-compiler.md`).
Front end = verbatim ports of Svelte 5.57.2 @ f3de3f2 and acorn 8.19.0. Upstream sources for the
port: `git clone https://github.com/sveltejs/svelte` @ f3de3f236bd586c764fd48bab04f41c54c5ff80d and
`https://github.com/acornjs/acorn` tag 8.19.0. No JS parser crates (oxc/swc/biome/boa are ruled
out: build cost). Dependencies stay minimal (`serde`, `serde_json`, `sourcemap`).

| Module | Ports | Owner | Public contract |
| --- | --- | --- | --- |
| `estree` | ESTree as acorn emits it | coordinator | typed nodes, `Spanned`, serde = acorn JSON |
| `acorn` | `acorn/src/*.js` except regexp | agent A | `parse`, `parse_expression_at`, `parse_statement_at`, `is_identifier_start/char` |
| `acorn::regexp` | `acorn/src/regexp.js`, `unicode-property-data.js` | agent B | `validate_flags`, `validate_pattern` |
| `acorn::typescript`, `estree::ts` | `@sveltejs/acorn-typescript` 1.0.13 `index.js` | agent TS | `AcornOptions::typescript` / `Options::typescript` (Svelte's `TSParser`); TS nodes + the `ts` field of ESTree nodes |
| `svelte::ts`, `svelte::parse::remove_typescript_nodes` | `phases/1-parse/remove_typescript_nodes.js` | agent TS | `strip(&mut Root, src) -> Result<TsStrip, CompileError>`; `TsStrip::apply(&mut Edits, span)` |
| `svelte::ast` | `types/template.d.ts` (AST namespace) | coordinator | typed template nodes, serde = Svelte modern JSON |
| `svelte::parse`, `svelte::errors` | `phases/1-parse/**` (not `read/style.js`), parse errors of `errors.js` | agent D | `parse(src, &ParseOptions) -> Result<Root, CompileError>` |
| `svelte::scope` | `phases/scope.js` | agent E | scope tree + bindings over `Root` |
| `style` | none: native style subset | agent C | `parse_style` -> rules/declarations + diagnostics |
| `semantics` | none: `metadata/semantics/*.json` model | agent G | `BackendSemantics` trait + spec loader |
| `tests/oracle*`, `tools/oracle/` | harness | agent F | corpus generation with real acorn/svelte, JSON diff |
| `packages/native-runtime` | none | agent H | runtime API in `runtime-contract.md` |

## Porting rules (A, B, D, E)

1. **Verbatim**: keep upstream function names (snake_case), structure and order, one Rust file
   per upstream file where practical, with a header naming the upstream file and commit. Comments
   that explain upstream behaviour are kept. Do not "improve" logic; a deliberate deviation gets a
   `// SVELTERS:` comment and an entry in `docs/notes/native-compiler/deviations.md`.
2. **Typed output**: acorn builds untyped nodes and mutates them (`toAssignable` turns an
   expression into a pattern). The port converts between the typed enums at those points instead.
   Node shapes must still serialize exactly like upstream.
3. **Positions** are byte offsets. Anything upstream exposes as a UTF-16 index (error positions,
   `start`/`end`) is converted only at the oracle boundary. Columns in error messages follow
   acorn/Svelte (UTF-16) so messages match exactly.
4. **Errors**: upstream throws; the port returns `Result`. Error message text must match
   upstream byte for byte (the oracle compares it).
5. **No `unsafe`**, no new dependencies without the coordinator.
6. **Oracle first**: correctness is defined by agent F's corpus (real acorn / `svelte/compiler`
   output), not by hand-written expectations. Never edit corpus expectations to make a test pass.

## Merge order

estree/ast contracts (done) -> A, B, C, F, G, H in parallel -> D (needs A for expressions) and
E (needs D's ASTs) integrate against the oracle as soon as A lands.
