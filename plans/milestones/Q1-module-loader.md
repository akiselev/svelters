# Q1: the svelters module loader

Design: `plans/research/QUICKJS-RUNTIME.md` §3 (shape, dev pieces, fit with the M2 watch loop) and §6 row Q1,
`plans/research/HMR.md` §3.2 option 2 (delivery through the Rust loader), `plans/research/TRACKS.md` §2.1 LOADER
row Q1. User decision 5 (`CLAUDE.md` "Current focus"): svelters has its own Rust module loader that compiles
`.svelte`/`.svelte.js` live with our compiler, in release builds too. Lane: Q1 (LOADER writer), branch `ci/q1`.
H1 (`ci/h1`) and LD1 (`ci/ld1`) stack on it.

## What landed

### `crates/svelters-modules` (new)

```rust
pub trait ModuleProvider {                      // engine-agnostic; &self, interior mutability
    fn entry(&self) -> ModuleId;
    fn resolve(&self, base: &str, specifier: &str) -> Result<ModuleId, String>;
    fn load(&self, id: &str) -> Result<ModuleCode, String>;   // Source { js, map } | Cached { engine, bytes }
    fn begin(&self);                            // a fresh load: the graph's edges are rebuilt
    fn map_dir(&self) -> Option<PathBuf>;       // a bundle's `.map` directory; None = maps come with the code
    fn watch_files(&self) -> Vec<PathBuf>;      // what the watcher watches
    fn invalidate(&self, changed: &[PathBuf]) -> bool;  // do these changes affect the app?
    fn take_warnings(&self) -> Vec<String>;     // compiler warnings, skipped styles
    fn describe(&self) -> String;               // log lines
}
```

- **`BundleProvider`** (always built): today's `BundleResolver`/`BundleLoader` moved behind the trait. Ids
  relative to the bundle directory, relative imports only, nothing leaves the directory (symlinks included),
  `.map` files next to the modules, watch set = the bundle and its map. `Host::load_bundle` uses it, so
  `--bundle dist/app.js` works unchanged.
- **`SourceProvider`** (feature `source`, default on; the host links the crate without it):
  - `SourceConfig::for_path(dir | file)`: the app root is the nearest package.json. Its `"svelters"` field
    (`toolkit`, `spec`, `specFragments`, `embed`: the fields `@svelters/build` reads) gives the compiler setup.
    `Entry::App(App.svelte)` generates `mount(App, { props: {} })` as `@svelters/build`'s entry does.
    `Entry::Embed([(name, file)])` generates the EM1 embed entry. `Entry::Module(file)` evaluates a `.js` as
    is, and its namespace is the entry's.
  - Resolution:
    - the runtime's alias table first (see below);
    - `@svelters/native-runtime/kinds` is generated with `semantics::kinds_module` from the same spec;
    - `svelters:behaviors/<ns>` → `<root>/.svelters/app/behaviors/<ns>.js` (EV2b, the app binary's dump, as
      `vite.js` maps it);
    - otherwise Node ESM resolution (`source/resolve.rs`) with Vite's client conditions: `browser, import,
      module, development|production, default`; main fields `module, main`; extensions `.mjs, .js, .json`;
      `exports` (conditions in the package's key order, `*` patterns, longest prefix), `imports` (`#`),
      directory indexes, symlinks resolved. Not `oxc_resolver`: it turns serde_json's `preserve_order` on for
      the whole build, which reordered every JSON map the workspace generates (6 linux-gtk failures on
      491dcc16). package.json is read with an order-keeping value type instead.
      Decided (coordinator, 2026-10-22): own resolver, option (a); oxc_resolver dropped.
    - Ids are canonical absolute paths. Generated modules are `svelters:entry` and `svelters:kinds`.
  - Transforms by kind:
    - `.svelte` → `compile` (the toolkit's spec plus fragments, `dev` from the config);
    - `.svelte.js` → `compile_module`;
    - `.js`/`.mjs` as is;
    - `.json` → `export default <json>` (validated);
    - anything else → an asset, `export default "<absolute path>"`;
    - `.ts`/`.svelte.ts`/`.cjs`/`.css` are load errors that say why (Q2, no `require`, styles belong in
      `<style>`).
    - Compile errors are load errors made of the rendered diagnostics; warnings go to the console.
  - Transform cache: output by file, keyed by a hash of the text. A reload re-reads every file and recompiles
    only those whose text changed.
  - Module graph: nodes (id → file, imports) are rebuilt by every load. `graph_files()`, `importers(id)` (reverse
    edges, for H1). Watch set: the graph's files outside `node_modules` (the app, and workspace packages such
    as `packages/native-runtime/src`), plus the app's and the runtime's package.json and the spec files. A
    manifest or spec change clears the resolver cache, reloads the alias table and the spec, and drops the
    transform cache.
  - **Source overlay** (`provider.overlay()`, `Overlay`: `Arc<Mutex<..>>`, clone it into an editor thread):
    `set(path, text)`, `clear(path)`. Every read goes to the overlay before the disk. `check(path) -> Check {
    diagnostics, error }` compiles one file (overlay first) for its diagnostics without touching the running app
    or the cache. Setting the overlay does not reload by itself; `Host::request_reload` does, and reads it.

### The alias table (shared with Vite)

`packages/native-runtime/package.json` `"svelters": { "aliases": { ... } }`. A key is a specifier (matched with
or without `.js`) or a pattern with one `*` (one path segment: `svelters:gtk/*`). A value is `./<path>` in the
runtime package or `<package>/<path>`, a file inside a dependency found through `node_modules` without its
exports map. The table covers:
- Svelte's hidden modules;
- the `svelte` facade, `svelte/transition` and `svelte/animate`;
- the runtime's entry points, embed, lists;
- the `svelters:<toolkit>/<ns>` type modules.

`vite.js` builds its `resolve.alias` entries from the same table (`ALIAS_TABLE`, `ALIASES`, `runtimeAliases()`).
The exported path constants (`HIDDEN_MODULES`, `effectsPath`, …) keep their names and values. `@app`,
`svelters:app/*` and the kinds virtual module stay in `vite.js`: they are app-relative or generated.

### Host (`crates/svelters-host`)

- `JsEngine::load_module(Rc<dyn ModuleProvider>)` and `load_entry(Rc<dyn ModuleProvider>)` (were `&Path`).
- `quickjs.rs` is a thin adapter: `ProviderResolver` and `ProviderLoader`.
  - Host built-ins (`svelters:core`, `svelters:app/*`) are served first. LD1 moves them into a provider.
  - `ModuleCode::Source` maps go to the core's map store.
  - `ModuleCode::Cached { engine: "quickjs-ng" }` is read as bytecode (nothing writes it yet: Q2/Q4).
- `Host::load_from(provider)` and `Host::load_entry_from(provider)`. `load_bundle`/`load_entry(path)` wrap a
  `BundleProvider`. Re-exports: `svelters_host::{modules, ModuleProvider, ModuleCode, BundleProvider}`.
- `sourcemap.rs`: maps by module id from the provider (`insert`). They are kept across loads, so a failed reload
  still maps the old runtime. Positions in `.svelte` module ids are found (`App.svelte:7:3`), and absolute
  sources are shown as is.
- `reload.rs`: the watcher watches the directories of `provider.watch_files()` non-recursively and keeps the
  events of those files.
  - At the checkpoint, `invalidate(changed)` decides whether a reload is due: a file outside the graph does not
    reload, and a bundle that is mid-write waits as before.
  - After every attempt the watched set follows the new graph.
  - After a failed attempt from source, any change in a watched directory retries, since the fix may be a file
    the graph doesn't reach yet.
  - `request_reload()` always reloads.
  - The full reload itself is unchanged (M2b).
- The provider's warnings go to the console (`warn`) after each load.

### Runner and egui

- `svelters-egui-run [--bundle] <dist/app.js>` as before, or `--entry <app dir | file> [--dev] [--watch]`.
  Components are compiled for egui.
- `Embed::load_app_from(provider)` and `Embed::load_from(provider)` (embed entries).
- `svelters-egui` feature `source` (default on).
- `harness::TestApp::from_source(path)`.

## Tests

- `crates/svelters-modules` (unit): bundle resolution, alias matching and package splitting, kinds by extension,
  style detection, JSON and asset modules.
- `crates/svelters-host/tests/modules.rs` (log backend; a scratch app under `target/q1/`, borrowing the counter
  fixture's `node_modules`):
  - `an_app_loads_from_source_with_json_assets_and_runes_modules`: the graph reaches the app, the runtime and
    Svelte's hidden modules; `node_modules` is not watched; the reverse edges are right.
  - `runtime_errors_map_to_the_svelte_source` (`src/App.svelte:3:` through the in-memory map).
  - `compile_errors_are_load_errors_with_diagnostics`.
  - `the_overlay_is_read_before_the_disk` (`check` on a broken buffer, then the buffer is what runs; the file is
    untouched).
  - `styles_and_typescript_are_reported_not_silently_dropped`.
  - `editing_a_graph_file_reloads_and_other_files_do_not`.
  - `a_failed_reload_keeps_the_app_and_a_fix_recovers`.
  - `an_embed_entry_from_source_keeps_its_namespace`.
- `crates/svelters-host` `sourcemap` unit: `finds_positions_of_source_modules`, absolute sources shown as is.
- `crates/svelters-egui/tests/counter.rs` `counter_from_source_runs_under_kittest`: `fixtures/counter-egui` from
  source on egui under kittest, clicked, with no bundle and no Vite.
- Unchanged and passing locally: `svelters-host` `reload` (bundle path through `BundleProvider` and the new
  watcher), `started`, `sync_frame`.
- CI: `-p svelters-modules` added to linux-gtk's nextest list.

## Not done here (by design or later rows)

- Security (user decision 2026-10-22, deny-by-default extension policy; `plans/research/ARCH-SECURITY.md` §5,
  Q1 row). Done here: `SourceConfig::confine`, on through `SourceConfig::for_extension`. It refuses absolute
  specifiers (except in the host-generated entry), and `./`/`../` must stay inside the importing module's
  package root after symlinks are resolved. Test: `an_extension_cannot_import_files_outside_its_package`
  (the primary host may read the file; the extension may not). The primary app host stays unconfined.
  Deferred to SEC1/SEC2:
  - per-principal package roots (which packages an extension may load) and the module lock;
  - the overlay keyed by principal (none for locked principals);
  - a stable content hash per loaded source (the transform cache's hash is `DefaultHasher`, not a lock hash);
  - the watcher limited to package roots.
  Extension hosts also reach the provider only through LD1's policy-carrying `ComposedProvider`.

- `<style>` from source: Q3 (Svelte's CSS analyze/prune/scope in Rust). Until then the loader warns and the app
  runs unstyled; style-using apps keep the Vite path.
- TypeScript entry points and the on-disk bytecode cache: Q2. CJS interop: not planned (load error with a hint).
- `import.meta.hot`, versioned re-import, per-component swap: H1 (`ci/h1`).
- Scheme hooks, the `svelters:app/*` visibility filter, native module providers (moving the built-ins out of
  `quickjs.rs`): LD1 (`ci/ld1`).
- Release (`app.qbc`, compiler linked in production): Q4. Assets resolve to absolute paths, which is dev only.
- The debounce stays 120 ms for both providers; QUICKJS-RUNTIME §3.3 suggests ~30 ms from source. Not measured.
- `counter` (GTK/Qt runner) has no `--entry` yet; only the egui runner does (current focus).
