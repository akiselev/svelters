# Oracle harness (NC1)

Correctness of the Rust front end (`crates/svelters-compiler`: `acorn`, `svelte::parse`) is
defined by the output of the real acorn 8.19.0 and `svelte/compiler` 5.57.2 on a fixed corpus.

```sh
tools/oracle/run.sh                 # generate corpora if stale, run both suites (release)
FORCE=1 tools/oracle/run.sh         # regenerate
ORACLE_FILTER=tests-harmony tools/oracle/run.sh   # only ids containing the substring
ORACLE_SHOW=100 ...                 # print more failing cases (default 20)
ORACLE_UPDATE_BASELINE=1 tools/oracle/run.sh      # raise the pass-count ratchet
```

Requirements: `pnpm install --frozen-lockfile` (acorn, @sveltejs/acorn-typescript and svelte are
resolved from `node_modules/.pnpm`), Node 22, upstream checkouts in `$UPSTREAM` (default
`/tmp/claude-0/up`): `acorn` @ 8.19.0, `svelte` @ tag `svelte@5.57.2` (stock; before RT1 the fork base f3de3f2) and `acorn-typescript`
(`git clone https://github.com/sveltejs/acorn-typescript`, tested @ 5e2e704; optional, its
`test/*/input.ts` files are inputs only; the parsers that run are the npm packages).

## Corpus (`target/oracle/`, not checked in, ~37 MB, generated in ~2 s)

- `acorn.jsonl` (~4350 cases, ~29 MB): every `test`/`testFail`/`testAssert` of
  `acorn/test/tests*.js` (loaded in a `vm` with stub globals; options kept, with the driver's
  defaults `{locations: true}` / `ecmaVersion: 5`), plus every `.js` under `svelte/packages/svelte/src`
  and `acorn/acorn/src` (`ecmaVersion: 16, sourceType: "module"`), plus TypeScript: every
  `acorn-typescript/test/*/input.ts` and every `.ts` under `svelte/packages`, parsed by
  `acorn.Parser.extend(tsPlugin())` (@sveltejs/acorn-typescript 1.0.13, Svelte's `TSParser`) with
  `{ecmaVersion: 16, sourceType: "module", typescript: true}` (`typescript` is not an acorn
  option: it selects the plugin, `crate::acorn::Options::typescript` on the Rust side). Records
  `{id, code, options, ast, comments}` or `{id, code, options, error: {message, pos, raisedAt}}`.
- `svelte.jsonl` (~3680 cases, ~14 MB): every `.svelte` in the `parser-modern`, `parser-legacy`,
  `compiler-errors`, `runtime-runes`, `runtime-legacy`, `validator` samples, plus this repo's
  `fixtures/*/src/*.svelte`, parsed with
  `parse(src, {modern: true, loose})` (`loose` for `parser-modern/loose-*`). Records
  `{id, source, options, ast}` or `{id, source, options, error: {code, message, start, end}}`
  (message without Svelte's docs-URL line).

Positions are UTF-16 offsets (upstream's). `loc`/`range` are not written. BigInt `value` -> `null`,
lone surrogates in strings -> U+FFFD; cases whose source has lone surrogates carry `skip`.

## Rust side

`crates/svelters-compiler/tests/oracle_{acorn,svelte}.rs` + `tests/oracle_common/mod.rs` (the
ignore list and comparison rules are documented at the top of that file). Categories: pass, ast
mismatch, error mismatch, unimplemented (`todo!()`), panicked, skipped (unsupported acorn option,
upstream crash). Reports go to stdout and `target/oracle/<suite>-report.txt`. The pass counts may
never drop below `tests/oracle_baseline.json`. Never edit corpus expectations.

Self-tests (`*_selftest_roundtrip`, `utf16_map_non_ascii`, `comparator_rules`,
`estree_contract_smoke`, `svelte_ignore_list`) check the harness itself: every corpus expectation,
converted to byte offsets and fed through the "ours" path, must compare equal.
