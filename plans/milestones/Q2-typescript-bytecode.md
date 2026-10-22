# Q2: TypeScript modules and the on-disk bytecode cache

Design: `plans/research/QUICKJS-RUNTIME.md` §3.2 (rows `.svelte.ts`/`.ts` and Cache) and §6 row Q2,
`plans/research/ASSETS.md` AS-O2 (decided (a): compiled JS ships, QuickJS bytecode is cached on disk at first
launch, keyed by the engine stamp), `plans/research/TRACKS.md` §2.1 LOADER row Q2, and the Q2 items deferred by
`plans/milestones/Q1-module-loader.md`. Lane: Q2, branch `ci/q2`.

## What landed

### TypeScript (`svelters-compiler`, `svelters-modules`)

No new dependency: the loader reuses what `<script lang="ts">` already uses, the ported acorn-typescript parser
and `remove_typescript_nodes` with its source-range strip (`svelte::ts`).

- `remove_typescript_nodes_program(program, src, strip)` (SVELTERS addition) runs the same walk over a whole
  module; `svelte::ts::strip_program` returns the ranges; `svelte::ts::is_ts_module(filename)` (`.ts`, `.mts`).
- `compile::strip_typescript_module(src, filename) -> CompileResult`: a `.ts`/`.mts` module with its types
  removed and the JavaScript left as written (no runes, no runtime import), plus a source map. Removed
  declarations leave their line breaks, so positions stay close to the source.
- `compile_module` parses with TypeScript on and strips it first when the filename is TypeScript
  (`.svelte.ts`); `transform_module` takes the strip ranges (`ts: &TsStrip`). No `ModuleOptions` field: the
  filename already decides, as for components. The `svelters-compile` binary and CX1's check get `.svelte.ts`
  through the same path.
- Errors are the `lang="ts"` ones (`typescript_invalid_feature`: enums, namespaces with values, decorators,
  parameter properties, ...) and syntax errors, rendered as load errors.
- Loader kinds (`source/transform.rs`): `.svelte.ts` → `RunesModule`; `.ts`/`.mts` → new `Kind::Ts`;
  `.d.ts`/`.d.mts` → load error ("no runtime code: use `import type`"); `.tsx`/`.jsx` → load error (no JSX);
  `.cts` joins `.cjs` (no `require`).
- Resolution (`source/mod.rs`, `source/resolve.rs`): `.mts` and `.ts` join the tried extensions (after `.mjs`,
  `.js`, before `.json`, as Vite), and TypeScript's convention holds: `./util.js` that does not exist names
  `./util.ts` (`.mjs` → `.mts`).

### Bytecode cache (`svelters-host`, new `src/bytecode.rs`)

- `BytecodeCache::open(root)`: `<root>/<hash of stamp>/STAMP` + one `<key>.qbc` per module.
- **Engine stamp** (`bytecode::stamp()`): `JS_GetVersion()` (QuickJS-NG 0.16.2 today; its bytecode format
  version moves with it), the `JS_WriteObject` flags, arch/OS, pointer width, endianness, debug/release, and the
  `svelters-host` version. Another stamp reads another directory.
- **Key**: FNV-1a 64 over (stamp, module name, JavaScript text). The name is in the key because QuickJS stores
  it in the bytecode; a source compiled differently (HMR on) is a different text, so a different key.
- **Safe invalidation**: a header (magic, key, name, hash of the JavaScript, payload length and checksum) is
  checked before the bytes reach `JS_ReadObject`, so a truncated, corrupt, empty or misplaced file is a miss; a
  miss compiles from source and rewrites the entry. An entry QuickJS still rejects is deleted and recompiled.
  Writes are a temporary file renamed into place (no half files; concurrent writers both succeed).
- **Pruning** at open: directories of other stamps whose `STAMP` was not rewritten for 7 days
  (`STALE_STAMP`; only 16-hex-digit directories with a `STAMP`, nothing else in the root is touched), entries
  not written for 30 days (`STALE_ENTRY`; an entry in use is rewritten once that long), leftover temp files
  older than an hour.
- **Wiring** (`quickjs.rs` `declare_code`): every `ModuleCode::Source` goes through the cache when one is set:
  hit → `Module::load`; miss → `Module::declare`, `Module::write`, `put`. Source maps are still registered from
  the provider. HMR re-imports (`?v=n`) are never cached (one-off names). `ModuleCode::Cached` from a provider
  is unchanged.
- **Host API**: `Host::set_dx(dx)` opens the cache under `dx.paths.cache()/bytecode` (DX1's `Paths`) unless
  `Host::set_bytecode_cache(Some(dir) | None)` was called (explicit wins, `None` disables). A cache that cannot
  be opened is logged (`tracing::warn`) and skipped. `Host::bytecode_cache()` exposes it; `hits()`/`writes()`
  count since open.
- **Trust**: the checksum detects damage, not tampering. The directory is the app's own cache, written by this
  process, as trusted as its binary (QUICKJS-RUNTIME §3.5). Bytecode shipped with an extension bundle is not this
  cache (ASSETS.md: ignored unless trusted, ARCH-SECURITY's rule). No extension-reachable API was added, so no
  policy hook is needed; whether extension sources may share the app's cache is ARCH-SECURITY's to say.

## Tests

- `svelters-compiler` `tests/q2_typescript.rs` (4): stripping, line layout kept, `$state` untouched in `.ts`,
  enum/namespace errors, source map names the `.ts`, `.svelte.ts` strips and compiles runes, the same text as
  `.svelte.js` is a syntax error.
- `svelters-modules` `source::transform::tests::kinds_by_extension` (updated).
- `svelters-host` `bytecode::tests` (4): stamp, round trip and key separation, damaged entries (truncated,
  flipped byte, garbage, empty, another key's file) are misses, pruning of stale stamps/entries.
- `svelters-host` `tests/modules.rs`: `typescript_modules_components_and_runes_modules_load_from_source` (a
  `lang="ts"` component importing `./util.js` → `util.ts`, an extensionless `.mts`, a `.svelte.ts`),
  `typescript_errors_are_load_errors` (enum, `.tsx`),
  `the_bytecode_cache_is_written_at_first_launch_and_read_after` (cold: every module written; warm: every module
  a hit, nothing written; one edited file: one write; all entries damaged: all recompiled),
  `set_dx_opens_the_bytecode_cache_under_the_apps_cache_dir`. The Q1 test
  `styles_and_typescript_are_reported_not_silently_dropped` became `styles_are_reported_not_silently_dropped`.

## Not done / follow-ups

- Warm-start time not measured (QUICKJS-RUNTIME's probe: 95-120 ms cold → about 13 ms warm for 115 modules).
  The provider still transforms every file on each launch (the Rust compile, ~0.6 ms per component); only the
  QuickJS parse/compile is cached. Q4 measures and may key the cache before the transform.
- TS features `remove_typescript_nodes` rejects (enums, namespaces with values, decorators, parameter
  properties) stay errors, as in `lang="ts"`. No `.tsx`. `tsconfig.json` `paths` aliases are not read.
- `.d.ts` imports are refused rather than dropped; `import type` is the fix.
- The stamp does not see C compiler flags of `rquickjs-sys` beyond the profile (e.g. its `disable-assertions`
  feature); the app can add its own build id later if that matters. Release `app.qbc` and `precompile`: Q4.
