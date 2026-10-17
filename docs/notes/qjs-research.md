# QuickJS runtime research (qjs-research lane)

Working notes for `plans/research/QUICKJS-RUNTIME.md`. Written incrementally; newest at the bottom
of each section. Clones live in the session scratchpad (`.../scratchpad/qjs/`), probes in
`docs/notes/scripts/qjs-*`.

## 1. Where we are (2026-10-17)

- Workspace: crates.io `rquickjs = "0.14.0"`, `default-features = false, features = ["std", "loader"]`
  (`Cargo.toml:35`); `rquickjs-sys` forced to opt-level 2 in dev.
- rquickjs 0.14.0 was released 2026-09-17/18 and pins QuickJS-NG `0fdea21` = v0.16.2+26.
  Upstream master (`cb3bf65`, 2026-10-16) pins QuickJS-NG v0.17.0 (`6d46d07`). So the workspace
  is on the **latest release**; master is ~1 month ahead.
- Host loader today (`crates/svelters-host/src/quickjs.rs:801-868`): `BundleResolver` accepts only
  `./`/`../` specifiers and refuses to climb above the bundle dir; `BundleLoader` reads the file
  and `Module::declare`s it. Vite produces one ES module (`dist/app.js`) + `.map`, so the loader
  is effectively a single-file loader.
- Engine setup: memory limit, max stack size, interrupt handler (turn budget + external flag),
  host promise rejection tracker. Raw-ctx re-entry trick for nested host ops (`RunningCtx`).
- Source maps: `sourcemap.rs` rewrites `module.js:line:col` in messages/stacks using the
  `<module>.map` files next to the bundle. Pure Rust, no `Error.prepareStackTrace`.
- Hot reload: `reload.rs` watches the bundle file (notify), debounces 120 ms, builds a fresh
  runtime, loads, and swaps windows (full reload, all JS state lost).
- GC: `prelude.js` and `native-runtime/src/index.js` already use `WeakRef` +
  `FinalizationRegistry` for handle release.

## 2. The rquickjs fork(s)

### `akiselev/rquickjs` (2026)

- Created 2026-01-14 as a GitHub fork of DelSkayn/rquickjs; only branch `master`.
- `git log up/master..origin/master` is **empty**: zero own commits. Its head is upstream
  `5e34dae` (2026-01-12, "Merge PR #599 dependabot sys/quickjs-97d23e5"), between v0.11.0
  (2025-12-24) and v0.12.0 (2026-05-27), QuickJS-NG v0.11.0+56.
- Behind upstream master by **283 commits** (251 excluding dependabot).
- No PRs by akiselev upstream. Conclusion: the 2026 fork is a pristine snapshot, probably made
  alongside `mquickjs-rs` (same week). Nothing to keep; nothing to rebase.

### `akiselev/rquickjs-old-fork` (2021, the v1 submodule `lib/rquickjs`)

The real fork. 10 own commits on top of katyo/rquickjs @ `f35df72` (2021-10-23), head `e2b31ef`
(2021-11-17), submodule pointer of the v1 tree. Changes (+441/-52):

| Change | Why (v1) | Upstream today |
|---|---|---|
| `Ctx.ctx` / `Ctx::from_ptr` public, raw function-call helpers public | callbacks from Qt/GTK threads re-creating a `Ctx` | `Ctx::as_raw`, `Ctx::from_raw`, `qjs` re-export: covered |
| `eval_global`, `eval_global_file`, `eval_global_module` (`JS_EvalThis` with global this) | loading the Svelte compiler's UMD + polyfills into a context | `Ctx::eval_with_options(EvalOptions{global,strict,..})`, `eval_file_with_options`: covered |
| `typedmap` feature: `Ctx::insert_typed/get_typed/...` on the context opaque | per-context Rust state (e.g. swc SourceMap per file) | `Ctx::store_userdata` / `userdata::<T>()` (`JsLifetime`): covered |
| `backtrace` feature: Rust backtraces inside `rquickjs::Error`, `Error: Clone`, `Error::new(ctx,msg)` | debugging | `Exception::from_message`, `Error::new_*`; Rust backtrace: drop |
| `is_null` / `is_undefined` | convenience | covered |
| reentrant mutex in runtime + "memcheck" ifdef | calling back into JS from nested native calls | we solved re-entry in the host (`RunningCtx` over the raw ctx); upstream `parallel` uses a plain mutex. Keep as host concern, not fork |
| QuickJS submodule -> `akiselev/quickjs` (fork of **koush/quickjs**, Bellard QuickJS + VSCode debugger protocol), `CONFIG_VERSION` removed for Zig | VSCode debugging of the app JS | QuickJS-NG has no debugger; PR quickjs-ng#1421 (see §5) is the modern equivalent |

So of the old fork, only the **debugger** has no upstream equivalent.

### `akiselev/quickjs` (2021)

Fork of koush/quickjs (Bellard 2021 + koush's `quickjs-debugger` VSCode protocol, ~24 debug
commits). Own commits: drop `CONFIG_VERSION` for Zig, a memcheck ifdef. Bellard QuickJS base,
not NG: obsolete.

### `akiselev/mquickjs-rs` (2026-01-09, 9 commits, not a fork)

Bindings to **Fabrice Bellard's MicroQuickJS** (`bellard/mquickjs`, Dec 2025): `mquickjs-sys`
(cc build of `../mquickjs/*.c`, sources not vendored — needs `make mquickjs_atom.h mqjs_stdlib.h`
first), `mquickjs` (no_std safe API, fixed heap `&mut [u8]`), `mquickjs-macros`
(`#[js_function]`, `#[js_method]`, `#[js_constructor]`), examples (gpio class, minimal stdlib).
MicroQuickJS supports "a subset of JavaScript close to ES5", strict-only, stdlib compiled to ROM
tables. No Proxy, no ES modules, no Promise/async, no classes/private fields — everything Svelte 5
runes (`$state` proxies, `WeakMap`, microtasks) need. **Not relevant for the app runtime.** Only
conceivable use: a tiny config/scripting engine on microcontroller targets; out of scope.

## 3. Old (2021) hooked module importer

See §3 of the proposal for the summary; sources in `v1-legacy` tag:
`lib/compiler/src/{lib,compiler,loader,resolver,task,dependencies,typescript}.rs`,
`lib/compiler/js/compiler.js`, `gtk/modules/loader.rs` (introduced `feecc8a`/`cb485d0`, 2021-06-20).

- `SvelteCompiler` = a `Resolver` + `Loader<Script>` registered on the app runtime
  (`rt.set_loader(resolver, loader)` via `gtk/lib.rs:91`).
- Resolver: `node_resolve` crate, extensions `[svelte, ts, mjs, js]`, main fields
  `[svelte, module]` (pre-`exports` era); base = canonicalized importer dir. So bare imports
  resolved straight into `node_modules`.
- Loader: absolute paths only; dispatch on extension: `.ts` -> swc parse + vendored swc
  `typescript::strip` (1992-line copy) -> swc codegen; `.svelte` -> `compile_sync` -> source;
  else read file. Then `Module::new(ctx, name, src)`.
- `.svelte` compile ran the **JS Svelte 3 compiler inside a second QuickJS runtime** on a
  dedicated thread (tokio current-thread + LocalSet + rquickjs `spawn_executor(Tokio)`), with
  polyfills (`polyfill/index.js`, 2984 lines) and `self.js`; the app thread sent
  `(path, src, reply)` over a zero-capacity flume channel and blocked (`compile_sync`).
  `compiler.js` = `svelte.preprocess` (script: Rust `transpile` = swc TS strip) then
  `svelte.compile(code, {namespace:'foreign', accessors:true})`.
- `dependencies.rs`: dev = `BuiltinResolver/BuiltinLoader` serving `svelte`, `svelte/internal`
  from `node_modules` read at startup + `FileResolver` + `ScriptLoader` + `NativeLoader`;
  prod = same but `include_str!` the Svelte runtime into the binary.
- No caching, no invalidation, no source maps (a TODO in `task.rs`), no CSS (styles came later
  via svelters-zig).

What carries over: the shape (resolver + compiling loader on the app runtime, embedded table for
release). What doesn't: compiler-in-JS (now Rust, in-process, no second runtime/thread),
`main` fields resolution (now `exports` + conditions), no map.

## 4. Upstream rquickjs since the fork point / since 0.14

Fork point `5e34dae` (2026-01-12) -> 0.12.0 (2026-05-26) -> 0.12.1/0.12.2 -> 0.13.0 (2026-09-07)
-> 0.14.0 (2026-09-17, what we use) -> master `cb3bf65` (2026-10-16). Relevant to us:

- **0.12 (breaking):** import attributes on `Loader`/`Resolver` (`Option<ImportAttributes>` —
  we already implement the new signature), f16/`Float16Array`, `Value::new_big_int -> Result`,
  MSRV 1.87, `async_with` closure syntax. Added: `#[qjs(prop)]`, `FromJs`/`IntoJs` derives for
  plain structs, `Object::new_proto`, `Symbol::new*`, `ArrayBuffer::from_source` (zero-copy
  external buffers incl. `bytes::Bytes`). Fixes: interrupt handler + `JS_ExecutePendingJob` GC
  assertion (#664, we use the interrupt handler), rope strings not recognised as strings,
  promise polling, `set_max_stack_size(usize::MAX)` crash, many class-macro fixes.
- **0.13:** QuickJS-NG 0.16.2; `TypedArray/ArrayBuffer::as_bytes` now `unsafe` (soundness);
  parallel-feature deadlock fix.
- **0.14:** `Exception::throw_dom`/`DOMException`, wasm32-unknown-unknown; fix:
  `execute_pending_job` now clears a job's exception (we rely on job errors in `run_jobs`).
- **master (unreleased):** QuickJS-NG **0.17.0** (private symbols, atom/wire-format and
  peephole perf, DOMException conformance, several UAF/OOB fixes), `Value::loose_eq/strict_eq/
  same_value[_zero]`, `#[methods]` attribute flags, null-byte eval fix, exotic-class double free.
- Nothing upstream adds a debugger, profiler, or Node resolver. The `loader` module still ships
  only `FileResolver`/`ScriptLoader`/`BuiltinResolver`/`BuiltinLoader`/`ModuleLoader`/
  `NativeLoader`/`Bundle` + `embed!` macro (compile JS to bytecode at Rust build time) +
  `Compile` (record bytecode while loading).

QuickJS-NG master is at 2026-10-16; v0.17.0 is the latest tag. The open NG PR **#1421 "Add
Debugging Interface"** (2026-03-26, +620 lines in quickjs.c/h, still open 2026-09 with "gentle
ping"s) adds `JS_SetDebugTraceHandler` (callback per statement boundary via a new `OP_debug`,
only for code parsed after the handler is installed), `JS_GetStackDepth`,
`JS_GetLocalVariablesAtLevel`/`JS_FreeLocalVariables`. A VSCode adapter exists
(G-Yong/QuickJS-Debugger). This is the one patch that would justify a QuickJS-NG fork.

Ecosystem crates: `llrt_modules`/`llrt_fetch` 0.8.1-beta (2026-07) pin `rquickjs ^0.11` and
tokio; `rquickjs-extra` 0.3.0 (2026-10-10: console, os, sqlite, timers, url). None are
usable unchanged with 0.14 + our toolkit-loop executor; treat them as reference code.

## 5. Probes (docs/notes/scripts/qjs-loader-probe)

`qjs-loader-probe app/main.js packages/native-runtime` (release build, rquickjs 0.14.0,
oxc_resolver 11.24.3, svelters-compiler by path). Entry imports `svelte/internal/client`,
`svelte`, `@svelters/native-runtime` and `./App.svelte` (`lang="ts"`, `$props`, `$derived`,
imports `./store.svelte.js`).

- **Works with no Vite:** oxc_resolver with conditions
  `[custom-renderer, browser, import, module, production, default]` + the 3 hidden-module
  aliases of `native-runtime/vite.js` + `@svelters/native-runtime$` alias resolves the whole
  graph; the loader compiles `.svelte`/`.svelte.js` in-process; everything evaluates.
- Graph: **115 modules, 635 KiB source** (Svelte's client runtime unbundled is most of it).
- From source: **~95-120 ms** total (resolve 5-8 ms, fs 2-4 ms, svelters-compiler 1.3 ms for
  the two Svelte files, the rest QuickJS parse/compile). Note QuickJS-NG loads imports
  *during* `Module::declare` (nesting depth 14), so per-module declare times are inclusive.
- Recorded `Module::write` bytecode, then a fresh runtime from the table only: **~12-13 ms**.
  Table 979 KiB with debug info, 322 KiB stripped (`strip_source+strip_debug`, ~11.7 ms).
- Comparison, a Vite bundle (`fixtures/styles/dist/app.js`, 229 KiB, tree-shaken, JS pipeline):
  ~45 ms parse (the probe parses the entry twice: 80-90 ms), bytecode 330 KiB / 118 KiB
  stripped, **3-4 ms** from bytecode.
- Conclusion: unbundled-from-source is fine for dev (~0.1 s cold; a per-module bytecode cache
  keyed by content hash makes warm loads ~13 ms + changed modules). For release, a tree-shaken
  bundle is ~3x smaller and ~4x faster to load than the unbundled graph, so release still wants
  a bundler (Rolldown is on crates.io as `rolldown` 1.2.14) or our own shaking.

`features` binary (same build) — what QuickJS-NG 0.16.2+26 gives us today:

| Probe | Result |
|---|---|
| `Error.prepareStackTrace` with V8 CallSites | works: `getFileName/LineNumber/ColumnNumber/FunctionName` |
| `Error.captureStackTrace`, `stackTraceLimit` | work (limit 10) |
| WeakRef / FinalizationRegistry | work; FR callback runs as a job after `run_gc` |
| SharedArrayBuffer / Atomics | present |
| `Intl` | **undefined**; `toLocaleString('de-DE')` ignores locale; NG docs: "unlikely to ever support Intl" |
| Iterator helpers, Set methods, `Promise.withResolvers`, `Array.fromAsync`, `using`/`DisposableStack`, RegExp `v`, `Float16Array`, `Math.sumPrecise`, `Error.isError`, private fields, static blocks, BigInt | all work |
| `queueMicrotask`, `performance` | present |
| `structuredClone`, `Temporal` | undefined |

## 6. Design notes feeding the proposal

- Dev loader gaps found while reading the compiler: `compile_module` parses with TS off (no
  `.svelte.ts`), and there is no standalone `.ts` strip entry point (the acorn-typescript port
  and `remove_typescript_nodes` exist; only `.svelte` `lang="ts"` uses them). No `hmr` codegen.
- CSS: `<style>` scoping/pruning still runs in the JS Svelte compiler inside
  `native-runtime/vite.js` (`svelteCompile(..., {css:'external', cssHash})`), and
  `@svelters/style/build` (`translate.js`) imports `parseCss` from `svelte/compiler`. So both
  CSS halves need Rust ports before dev is Node-free. Biggest single item.
- Runtime-lane interface (`plans/research/RUNTIME.md`): its "public modules redirected by vite.js
  aliases" table is exactly the loader's alias table; proposal: declare it as data in the runtime
  package so Vite and the Rust loader share it.
- rquickjs `futures` = `AsyncRuntime` + Send executor; does not fit a toolkit-owned loop or the
  engine-agnostic `JsEngine`; prefer a host completion queue + backend `wake` (as `reload.rs`).
- Bytecode: tied to the exact NG build (BC_VERSION + config); `JS_READ_OBJ_BYTECODE` trusted
  input only; `WriteOptions{strip_source, strip_debug}`; `embed!` macro compiles at cargo time.
- QuickJS-NG module registry: modules stay in the context once loaded (believed; not verified),
  so HMR via versioned re-imports leaks one module per swap until a full reload.
- Rolldown is on crates.io (`rolldown` 1.2.14, 2026-10-14) — R2 candidate for tree-shaking in
  the build tool only.

## 7. Status

Proposal written: `plans/research/QUICKJS-RUNTIME.md`. Probe crate:
`docs/notes/scripts/qjs-loader-probe` (standalone `[workspace]`, `target/` ignored; built with
`CARGO_TARGET_DIR`/`CARGO_BUILD_BUILD_DIR` in the scratchpad through `tools/dev/cargo-slot`).
