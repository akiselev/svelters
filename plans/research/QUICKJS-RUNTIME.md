# QUICKJS-RUNTIME: the rquickjs fork, a Rust module loader, and what else QuickJS-NG gives us

Status: research proposal, 2026-10-17. Working notes, sources and numbers:
`docs/notes/qjs-research.md`. Probes: `docs/notes/scripts/qjs-loader-probe/` (a standalone
crate; `cargo run --release -- app/main.js ../../../../packages/native-runtime`, and
`--bin features`).

## Summary

1. **There is no fork to maintain.** `akiselev/rquickjs` (2026) has zero commits of its own. It
   is upstream as of 2026-01-12, now 283 commits behind. The workspace already uses the latest
   release (crates.io `rquickjs 0.14.0`, QuickJS-NG 0.16.2+26). The real fork is the 2021
   `rquickjs-old-fork`. Upstream has since covered every patch in it except the koush/quickjs
   VSCode debugger. Stay on crates.io. Fork only when we carry a C patch: the QuickJS-NG
   debugger PR #1421 is the one candidate. Even then, fork only the QuickJS submodule pointer and
   switch to it with `[patch.crates-io]`.
2. **The host can load apps from source, with no Vite.** The probe resolves a real app graph
   from `node_modules` with `oxc_resolver`, compiling `.svelte`/`.svelte.js` in-process with
   `svelters-compiler`. That covers `exports` conditions plus the native-runtime aliases. The
   graph is 115 modules and 635 KiB, and it evaluates in about 0.1 s cold. From cached bytecode
   it evaluates in about 13 ms. Only one piece is still missing for a Rust-only `svelters dev`:
   style handling. Svelte's CSS scoping and `@svelters/style` still run in Node.
3. **Release does not need Vite either.** The same graph walk writes a bytecode module table.
   A tree-shaken bundle is about 3x smaller and loads about 4x faster than the unbundled graph,
   so add tree-shaking (Rolldown is a Rust crate) once size matters. Once the own runtime core
   from `RUNTIME.md` shrinks the graph, tree-shaking matters less.
4. **The most valuable additions**, in order:
   - the dev loader with a module graph and bytecode cache;
   - in-memory source maps;
   - async host ops on our own completion queue (not rquickjs `futures`);
   - built-in `svelters:*` modules;
   - a debugger, which needs the C patch.

   Intl is the notable gap: QuickJS-NG has none, and it never will.

## 1. The forks

| Repo | What it is | Verdict |
|---|---|---|
| `akiselev/rquickjs` (created 2026-01-14) | GitHub fork, `master` only, 0 own commits, head = upstream `5e34dae` (2026-01-12, between 0.11 and 0.12, QuickJS-NG 0.11+56). 283 behind master | Nothing to keep. Sync `master` if we ever branch from it |
| `akiselev/rquickjs-old-fork` (2021, v1 submodule) | 10 commits on katyo/rquickjs: public raw `Ctx`, `eval_global*`, typedmap userdata, Rust backtraces in errors, `is_null/undefined`, a reentrant runtime mutex, QuickJS → `akiselev/quickjs` | Upstream has replaced all of it: `Ctx::as_raw/from_raw`, `eval_with_options{global}`, `store_userdata/userdata`, `Exception`. The host's `RunningCtx` handles re-entry. Drop |
| `akiselev/quickjs` (2021) | koush/quickjs = Bellard QuickJS 2021 + the VSCode debugger protocol | Obsolete base. Its debugger is the only feature without a current equivalent; see §4.4 |
| `akiselev/mquickjs-rs` (2026-01) | no_std bindings + macros for Bellard's **MicroQuickJS**. The C sources are not vendored | MicroQuickJS is "a subset close to ES5": no Proxy, modules, Promise or classes, so Svelte 5 cannot run on it. Not relevant for the app runtime |

**Upstream since the fork point**, with what matters to us:
- 0.12 brought:
  - import attributes on `Loader` (we already implement them);
  - the fix for the interrupt handler + `ExecutePendingJob` GC assertion (we use that path);
  - rope-string fixes;
  - `ArrayBuffer::from_source` for zero-copy;
  - `FromJs`/`IntoJs` derives;
  - many fixes to the class macros.
- 0.13 moved to QuickJS-NG 0.16.2 and made the `TypedArray` byte views `unsafe`.
- 0.14 changed `execute_pending_job` to clear the job's exception.
- Master (unreleased) is on QuickJS-NG **0.17.0**, which brings private symbols, performance
  work and UAF/OOB fixes, plus `Value` equality helpers.
- No upstream release adds a debugger, a profiler or a Node resolver.

**Plan.**
1. Keep `rquickjs = "0.14"` from crates.io. Take 0.14.x patch releases, and take 0.15 when it
   ships with NG 0.17. Re-run the host suites and the turn-budget tests on each bump: QuickJS
   performance changes move the turn budget.
2. When a C patch is needed (the debugger), make a fresh branch at the upstream release tag.
   Don't rebase the stale fork's `master`: it has no commits, so a rebase is only a
   fast-forward. Name the branch `svelters/v0.14.x` in `akiselev/rquickjs`. Its only change is
   the `sys/quickjs` submodule pointer, set to `akiselev/quickjs-ng` branch `svelters` = the NG
   tag the release pins + PR #1421.
   - Declare the handful of new C functions by hand as `extern "C"` in `svelters-host`, so we
     don't have to regenerate rquickjs' per-target pre-generated bindings. The functions are
     in the same static library.
   - Switch to the branch in the root `Cargo.toml` only:
     `[patch.crates-io] rquickjs = { git = …, branch = … }`, plus `rquickjs-core` and
     `rquickjs-sys`. Member crates keep `rquickjs.workspace = true`. Dropping the patch
     returns us to crates.io. Cargo checks out submodules of git dependencies.
3. **Don't vendor.** The QuickJS C sources and the bindings for each target are big, and
   vendoring them makes every bump a large diff. A git patch keeps the change down to one
   branch with one commit.

## 2. What the 2021 importer did

The code is in the `v1-legacy` tag: `lib/compiler/src/{loader,resolver,compiler,task,dependencies}.rs`
and `gtk/modules/loader.rs`, added on 2021-06-20.
- **Hooks.** A `SvelteCompiler` implemented rquickjs' `Resolver` + `Loader<Script>`, and was
  registered on the app runtime with `rt.set_loader`.
- **Resolver.** It used the `node_resolve` crate on the importer's directory, with extensions
  `svelte, ts, mjs, js` and main fields `svelte, module`. So bare imports went straight into
  `node_modules`; `exports` maps came later.
- **Loader.** It accepted absolute paths only and dispatched on the extension:
  - `.ts`: swc parse + a vendored swc TypeScript strip + swc codegen;
  - `.svelte`: the JS Svelte 3 compiler (`preprocess` with the swc strip, then
    `compile({namespace:'foreign'})`);
  - anything else: read as is.
- **The compiler ran in a second QuickJS runtime** on its own thread (tokio + rquickjs
  executor, with 3k lines of polyfills). The app thread blocked on a channel for each
  component.
- **Release** used `BuiltinLoader` with `include_str!` of the Svelte runtime.
- **Not there:** caching, invalidation, source maps (a TODO) and CSS.

The shape carries over: a resolver plus a compiling loader on the app runtime in dev, and an
embedded table for release. The rest doesn't:
- the compiler is now Rust, in-process, with no second runtime or thread;
- resolution is now `exports` + conditions;
- the compiler now emits source maps.

## 3. Module loader design

### 3.1 Shape

```
                      svelters dev (runner binary: host + backend + loader)
  ┌─────────────────────────────────────────────────────────────────────────────────────┐
  │  notify watcher ──changed paths──▶ ModuleGraph ──stale set──▶ Reloader (reload.rs)  │
  │  (all graph files,                 id → {path, kind, hash,     full reload, or      │
  │   svelters.toml)                    deps, importers, output,   HMR boundary (Q5)    │
  │                                     map, bytecode}                                  │
  │                                         ▲          │                                │
  │        resolve(base, spec, attrs)       │          │ load(id)                       │
  │  QuickJS Resolver ─────────────▶ Resolver          ▼                                │
  │  (quickjs.rs adapter)            oxc_resolver   Transform by kind:                  │
  │                                  + alias table    .svelte      → compile()          │
  │                                  + conditions     .svelte.js/ts→ compile_module()   │
  │  QuickJS Loader ◀── Source{js,map} | Bytecode ─── .ts/.mts     → TS strip           │
  │  Module::declare / Module::load                   .json        → export default     │
  │        │                                          asset        → export path/id     │
  │        ▼                                          <style>      → registerStyles(…)  │
  │  SourceMaps (in memory, by id)                    .js/.mjs     → as is (CJS: hint)  │
  │                                        │                                            │
  │                                  BytecodeCache (.svelters/cache, key = content      │
  │                                  hash + compiler ver + QuickJS ver + flags)         │
  └─────────────────────────────────────────────────────────────────────────────────────┘

                      svelters build (same Resolver + Transform, no runtime needed for R2)
  entry ─▶ graph walk ─▶ R1: per-module bytecode table (+ specifier table)  ─▶ app.qbc / include_bytes
                      └▶ R2: Rolldown + svelters plugin ─▶ one shaken ESM ─▶ bytecode ─▶ app.qbc
```

**Engine boundary.** `JsEngine::load_module(entry: &Path)` becomes
`load_module(entry: ModuleId, provider: Rc<dyn ModuleProvider>)`.
`ModuleProvider { resolve(base, spec, attrs) -> ModuleId; load(id) -> ModuleCode }` is
engine-agnostic. `ModuleCode` is `Source { js, map }`, or `Cached(engine tag, bytes)`, which
only the engine that wrote the bytes reads. `quickjs.rs` keeps only a thin
`Resolver`/`Loader` adapter. The current `BundleResolver`/`BundleLoader` become the
`ModuleProvider` for "a directory with a built bundle", so `--bundle dist/app.js` keeps working
unchanged. The graph, the resolver and the transforms live in a new crate
(`svelters-modules`). That crate depends on `svelters-compiler` and `oxc_resolver`, not on
rquickjs.

### 3.2 Dev pieces

| Piece | Design | Status after the probe |
|---|---|---|
| Bare imports | `oxc_resolver`: full Node ESM resolution with `exports`/`imports`, conditions, symlinks (pnpm), `browser` field. Conditions are `svelte, custom-renderer, browser, import, module, development\|production, default` | **Works** (probe) |
| Runtime aliases | A data table, not code. Today it holds the three hidden Svelte modules + `@svelters/native-runtime`. Under `RUNTIME.md` it holds `svelte` → `shims/svelte.js`, `svelte/store`, …, and an error for `svelte/internal/*`. The runtime package declares the table (e.g. a `"svelters": {"aliases": …}` field in its package.json), so `vite.js` and the Rust loader read the same table and can't drift. This is the interface with the runtime lane | **Works** for today's table (probe) |
| `.svelte` | `compile(src, {filename, backend: SpecBackend for the runner's toolkit, dev: true, css_scope})`. About 0.6 ms per component in the probe | **Works** |
| `.svelte.js` | `compile_module` | **Works** |
| `.svelte.ts`, `.ts` | **Missing.** `compile_module` parses with TS off. Add `strip_ts_module(src) -> (js, map)` over the ported acorn-typescript + `remove_typescript_nodes`, and a TS flag for `compile_module`. Both are small additions in the compiler crate | to do |
| CSS | **Missing, and the main blocker.** Today the Vite plugin runs the *JS* Svelte compiler to scope and prune `<style>`, then `@svelters/style` (`translate.js` imports `svelte/compiler`'s `parseCss`) to translate per toolkit. Rust-only dev needs two ports into `svelters-compiler`: Svelte's CSS analyze/prune/transform (it already has the parser port), and `translate.js`/`decls.js`. Then the loader emits the same `registerStyles(file, {gtk, qt, qtDark})` module as `style-plugin.js`. This belongs to the ST1/X6 owners. Until it lands, an app with `<style>` uses the Vite path | to do (biggest item) |
| JSON | `with {type:'json'}`, or a bare `.json`: declare `export default <parsed JSON>`. The `ImportAttributes` reach our loader already | trivial |
| Assets | `import icon from './a.png'` → `export default "<absolute path>"` in dev. In release, an asset id served by an embedded resource table (open question) | small |
| CJS | QuickJS has no `require`. v0: refuse with a hint naming the package. Later: wrap `(module, exports, require)` and derive named exports with a lexer pass, as Node does. Svelte libraries are ESM, so this is rarely needed | deferred |
| Source maps | The compiler already returns `map`. The loader hands it to `SourceMaps::insert(id, map)`. Module ids are absolute source paths, so even unmapped frames name real files, and `node_modules` JS needs no map. `sourcemap.rs` keeps doing the mapping in Rust, which is engine-agnostic. `Error.prepareStackTrace` CallSites work on NG and could return structured frames instead of regex-parsed text: an optional refinement | small |
| Graph + invalidation | Each node records its deps from the resolver and its importers as the reverse edges. The watcher covers every file node + `package.json`s + `svelters.toml`, not just `dist/`. A change marks the node stale; a package.json change drops resolver caches (`oxc_resolver::clear_cache`) | to do |
| Cache | In memory: transform output by content hash. On disk: QuickJS bytecode by (hash, compiler version, rquickjs-sys/NG version, dev flag). Probe: cold 95-120 ms → warm about 13 ms for 115 modules | to do |
| Reload | **v1 = today's `reload.rs` full reload**, fed by the graph instead of a bundle file. The new runtime loads warm modules from the cache plus the recompiled changed ones, so the 120 ms debounce dominates. **v2 = component HMR**, see §3.4 | v1 small |

### 3.3 Fit with the M2 watch loop

`reload.rs` already has the hard parts:
- a debounced watcher on its own thread;
- a wake through the backend;
- a fresh runtime;
- "load new, destroy old";
- keeping the previous version on error, with mapped errors.

Only two things change. "The bundle path" becomes "the graph's file set". "Read `dist/app.js`"
becomes "ask the provider", so a failed compile becomes a load error with compiler diagnostics,
rendered with `Diagnostic::render`. The debounce no longer has to absorb Vite writing the
bundle and its map separately; it can drop to about 30 ms. Vite watch mode becomes unnecessary
when the frontend is the Rust compiler.

### 3.4 HMR (later)

The pieces:
- **What QuickJS allows.** A module name is instantiated once per context, and its
  `import.meta` object is reachable from Rust (`Module::meta`).
- **The compiler side.** The compiler gains Svelte's `hmr` output:
  `if (import.meta.hot) { C = $.hmr(C); import.meta.hot.accept(…) }`. Svelte's
  `internal/client/dev/hmr.js` already swaps a component behind a source signal.
- **The loader side.** It defines `import.meta.hot` per module. On a change to an accepting
  module it dynamic-imports `"<path>?v=<n>"` (`Module::import`). The new module's own imports
  resolve to the existing instances, except for other stale modules, which also get a new
  version.
- **When to fall back to a full reload:** for non-accepting changes, such as `.svelte.js` state
  modules, the runtime itself, or `svelters.toml`.
- **The cost:** QuickJS-NG appears never to free replaced modules while the context lives (believed from the source; not verified), so memory
  grows by one module per edit. Do a full reload every N swaps or above a memory threshold.

### 3.5 Release

- **R1, first: an unbundled bytecode table.** The release build walks the same graph with the
  same resolver and transforms (`dev: false`, `production` condition). It writes each module
  with `Module::write`, plus a table mapping `(importer id, specifier)` to an id. Both go into
  `app.qbc`, a file with an index, or `include_bytes!` in a generated runner.
  - At startup a table resolver and `Module::load` take over (the probe's pass 2).
  - Dev and release share one code path, so they can't diverge.
  - No new dependencies.
  - Size: 979 KiB with debug info and 322 KiB stripped for the probe app.
  - Default to `strip_source` only, so stacks keep line numbers. Its size was not measured
    separately.
- **R2, when size matters: tree-shaking.** Rolldown (`rolldown` 1.2.14 on crates.io, the Rust
  core of Vite 8) runs in the build tool with one plugin that calls `svelters-compiler`. Its
  output is one shaken ESM, which we then write as bytecode.
  - The comparison point is a Vite bundle: 229 KiB → 118 KiB stripped bytecode, 3-4 ms to
    load. That's about 3x smaller and 4x faster than R1.
  - The cost is a heavy oxc/rolldown dependency tree, but only in the build tool, not in the
    runner.
  - An alternative is a small shaking pass of our own over the compiler's acorn port, sized to
    our runtime.
  - After `RUNTIME.md`'s own core (about 25-35 KB), most of the gap closes. Decide on R2 after
    that lands.
- **Bytecode caveats.**
  - Bytecode is tied to the exact QuickJS-NG build, so stamp `app.qbc` with the rquickjs-sys
    version and a hash of the build options, and refuse a mismatch.
  - `JS_READ_OBJ_BYTECODE` is for trusted input only, so never load `.qbc` files from users.
  - The current `embed!` macro does the same at Rust compile time, but it needs the JS when
    Cargo builds. The CLI flow writes the bytecode itself.
- **Vite stays an escape hatch** for user plugins: `frontend = "vite"` builds `dist/app.js`,
  and the runner loads it through the bundle provider as it does today.

### 3.6 What this means for `plans/research/CLI.md`

CLI.md picked a Node CLI because "every build already needs Node… the native compiler's
`svelteNative` plugin" runs in Vite. It rejected a single Rust binary because "it would still
need Node for Vite", and said to revisit "if Vite is ever replaced". With this loader:
- **`svelters dev` can be Rust-only:** the runner binary is host + loader + compiler + watcher.
  `svelters dev` just execs it with `--entry src/main.js` and the toolkit.
- **`svelters build` can be Rust-only** with R1 or R2.

Node remains for two things:
- the package manager, since `node_modules` still has to be installed;
- the JS-side tools (`docs` rendering, `svelte-check`).

The CLI language choice can then be revisited. A Rust CLI plus the npm package manager is now
viable. The precondition is the CSS port; until it lands, `dev` falls back to Vite for apps with
`<style>`. This document doesn't decide the CLI; it removes the constraint.

## 4. What else rquickjs / QuickJS-NG can do, ranked

Value: H/M/L for svelters apps and developer experience. Cost: S (≤1 day), M (2-5 days),
L (>1 week).

| # | Feature | Works today? | Value | Cost | Note |
|---|---|---|---|---|---|
| 1 | Dev loader: resolver + compile on import + graph | yes (probe) | H | M | §3 |
| 2 | Bytecode cache (dev) and table (release) | yes (`Module::write/load`, probe) | H | S | 10x faster warm loads |
| 3 | In-memory source maps; optional CallSite frames | yes (`prepareStackTrace` CallSites work) | H | S | |
| 4 | Async host ops: a JS promise resolved from a completion queue drained at the checkpoint, with worker threads calling the backend `wake` | patterns exist (timers, `wake`) | H | M | Engine-agnostic, unlike rquickjs `futures`. That feature brings `AsyncRuntime` + a Send executor, and doesn't fit a toolkit-owned loop or the `JsEngine` trait. Use it for fs, process, fetch (a reqwest/ureq thread pool) and dialogs |
| 5 | Built-in modules `svelters:app`, `svelters:fs`, … | `ModuleDef` / `#[rquickjs::module]` work | M | S | Declare them through `ModuleProvider` as host-function tables, so any engine can serve them |
| 6 | Debugger: NG PR #1421 (`JS_SetDebugTraceHandler`, stack depth, locals) + a DAP server in the host (VSCode) | no; needs the C patch | H (DX) | L | The only reason to fork. The handler only instruments code parsed after it is installed, so it must be dev-only and set before load. A CDP/Chrome DevTools front end would cost more again |
| 7 | WeakRef / FinalizationRegistry for handle GC | yes, used already; FR runs as a job after GC | M | — | Keep relying on it, but the FR timing is unspecified, so keep the explicit destroy paths |
| 8 | Memory limit, interrupt handler, stack size | yes, used | — | — | Add `memory_usage()` to the dev overlay/log: S |
| 9 | Worker-like extra runtimes: a separate `Runtime` per thread, messages as `svelters_abi::Value` | yes (one `Runtime` per thread, no `parallel` feature) | M | M | For CPU work off the UI thread. No shared JS objects; `SharedArrayBuffer` + `Atomics` exist if needed |
| 10 | Profiler: a sampling hook in the interrupt handler (it runs every N ops) that records the current function via `script_or_module_name` / PR #1421's stack API, written as folded stacks | partly | M | M | `perf` sees only C frames. `JS_SetDumpFlags` is for debugging, not profiling |
| 11 | Intl | **no**, and NG will never ship it | M | L | Options: FormatJS polyfills (large, slow in QuickJS); a host `svelters:intl` over ICU4X for number/date/plural; or the toolkit's locale APIs. Decide per app need |
| 12 | `structuredClone`, `Temporal` | no | L | S / L | `structuredClone`: a small polyfill. `Temporal`: wait |
| 13 | `#[rquickjs::class]` bindings for widgets | yes | L | — | Widgets go through handles and the C ABI by design, so the engine stays swappable. Don't use it |
| 14 | `parallel` feature, multi-context | yes / disabled upstream | L | — | Not needed |
| 15 | Newer ES (iterator helpers, Set methods, `using`, RegExp `v`, `Float16Array`, `Array.fromAsync`, `Error.isError`) | yes on 0.16.2 | — | — | No transpile target lowering needed; es2022+ output is fine |
| 16 | QuickJS-NG 0.17 (private symbols, perf, safety fixes) | with rquickjs 0.15 | M | S | Take it on release |
| 17 | Heap snapshots | no (QuickJS has none) | — | — | Bytecode is the only startup cache |

## 5. Engines (a sanity check)

- **QuickJS-NG stays.**
  - Measured startup is 6 ms with a 1.5 MB static binary (`apple-gui-backend.md` §8).
  - It has current ES coverage and an actively maintained binding (rquickjs commits through
    2026-10-16).
  - Our loader, bytecode cache and host design all fit it.
  - Weak points: no Intl, no JIT (an interpreter at 1/12-1/85 of JSC JIT on compute
    kernels), and no debugger yet.
- **MicroQuickJS: no.** It supports only an ES5-ish subset, which cannot run Svelte 5.
- **JSC: an Apple-only option behind `JsEngine`.**
  - It has a JIT on macOS (needs `allow-jit` when hardened).
  - On iOS it runs 1.6-1.8x faster than QuickJS but has no JIT (`apple-gui-backend.md` §8).
  - It costs about 27-32 ms startup and 36 MB RSS.
- **Boa / Nova:** pure Rust, which helps the build story.
  - Boa's conformance is good but it is historically slower than QuickJS. Not measured here.
  - Nova is still experimental.
  - Neither offers anything we lack except no C.
- **quickjs-jit** (Cranelift, an rquickjs fork): already rejected in `quickjs-jit-cranelift.md`.
  It tied or lost on the real Svelte runtime, and it pins NG 0.15.1.

Keep the `JsEngine` boundary honest. The loader goes through `ModuleProvider`, and cached
bytecode is tagged by engine, so swapping the engine costs only the adapter.

## 6. Phasing

| Phase | Content | Size | Depends on |
|---|---|---|---|
| Q0 | Stay on crates.io 0.14.x. Write down the bump procedure: host suites + turn-budget tests. No fork | 0.5 d | — |
| Q1 | `svelters-modules` crate: `ModuleProvider`, oxc resolver + data-driven aliases, transforms for `.svelte`, `.svelte.js`, `.js`, `.json` and assets, in-memory maps. Runner gets `--entry`; the bundle provider keeps `--bundle`. Graph-driven watcher with today's full reload | 3-4 d | N1 rename settled (package names) |
| Q2 | TS: `.ts` and `.svelte.ts` entry points in the compiler; on-disk bytecode cache; CJS hint; diagnostics as load errors | 2-3 d | Q1 |
| Q3 | CSS in Rust: Svelte CSS prune/scope port + the `@svelters/style` translator port; the loader emits `registerStyles`. **Vite leaves dev here** | 4-6 d | ST1/X6 owners |
| Q4 | Release R1: `app.qbc` (bytecode table, version stamp, `strip_source`), runner `--app app.qbc` / embedded | 2-3 d | Q1 |
| Q5 | Component HMR (`hmr` codegen, `import.meta.hot`, versioned re-import, full-reload fallback) | 3-5 d | Q1, compiler |
| Q6 | Async host ops (completion queue), `svelters:*` built-in modules, fs/fetch | 3-5 d | — |
| Q7 | Debugger: QuickJS-NG fork with PR #1421 via `[patch.crates-io]`, DAP server | 1-2 wk | upstream PR status |
| Q8 | R2 tree-shaking (Rolldown or our own), profiler, Intl decision | as needed | `RUNTIME.md` core |

## 7. Open questions

1. **CSS port ownership and order.** Should Q3 come before or after the own-runtime work in
   `RUNTIME.md`? Until it lands, Rust-only dev covers only style-less apps.
2. **Where the alias table lives.** A `"svelters"` field in the runtime's package.json, or
   `svelters.toml`? Either way, `vite.js` should read the same source.
3. **The asset model.** Absolute paths in dev; in release, an embedded resource table or files
   next to the binary? The answer touches the packaging plans.
4. **CJS.** Refuse, or implement Node-style interop? Do we know of any dependency that needs it?
5. **Release default.** R1 only, or R2 from the start? The answer depends on the runtime-core
   size after `RUNTIME.md`.
6. **The debugger.** Wait for quickjs-ng#1421 to merge, or carry it now? Is DAP (VSCode) enough,
   or do we need CDP (Chrome DevTools)?
7. **Intl.** Do any target apps need locale formatting soon? If so, which route: ICU4X host
   module, polyfill, or toolkit APIs?
8. **The CLI language** (`CLI.md`). With Vite out of `dev` and `build`, should the CLI become
   Rust, with npm used only as the package manager?
9. **HMR memory.** How many swaps before a forced full reload? Measure once Q5 exists.
