---
title: Loading code and release bundles
description: "How an app or extension is loaded into QuickJS: SourceProvider (live compile), SvbProvider (.svb bundles), loader hooks, TypeScript and the bytecode cache."
status: available
verified_at: d0542a3a
sources:
  - crates/svelters-modules/src/{lib,compose,native,dylib}.rs
  - crates/svelters-modules/src/source/
  - crates/svelters-modules/src/svb/
  - crates/svelters-host/src/{lib,hooks,bytecode}.rs
  - crates/svelters-egui/src/embed/mod.rs     # load_from, load_app_from
  - crates/svelters-egui/src/bin/{svelters-egui-run,svelters-bundle}.rs
  - plans/milestones/Q1-module-loader.md
  - plans/milestones/Q2-typescript-bytecode.md
  - plans/milestones/Q4-bundle-format.md
  - plans/milestones/Q4b-bundle-followups.md
  - plans/milestones/LD1-loader-hooks.md
  - plans/milestones/LD2-live-styles.md
  - plans/milestones/LD3-loader-fixes.md
---

# Loading code and release bundles

svelters has its own Rust module loader. It loads JS modules into QuickJS and compiles `.svelte`,
`.svelte.js` and `.ts` **live with the Rust compiler, in release builds too**. Vite is a dev convenience
only. Every load goes through a `ModuleProvider`:

| Provider | Use it for | Reads |
|---|---|---|
| `SourceProvider` | dev, HMR, self-editing UI, extensions shipped as source | the app's files, compiled live |
| `SvbProvider` | release: one `.svb` file, optionally with bytecode | the bundle only, no file system |
| `BundleProvider` | a Vite build (`dist/app.js`) | the bundle directory |

## Load an app or embed entry

```rust
// not run
use std::rc::Rc;
use svelters_host::modules::{SourceConfig, SourceProvider};

let cfg = SourceConfig::for_path(Path::new("ui"))?;      // nearest package.json is the app root
let provider = Rc::new(SourceProvider::new(cfg)?);
let entry = embed.load_from(provider)?;                    // embed entries (exports for slots)
// or: embed.load_app_from(provider)?                      // an app that mounts itself
```

`SourceConfig::for_path` takes a directory (its embed entry, or `src/App.svelte`) or a file (`.svelte`
mounts it; anything else is evaluated as is). It reads the `"svelters"` field of `package.json`
(`toolkit`, `spec`, `specFragments`, `embed`). Fields you may set: `dev`, `hmr` (see
[Hot reload](hot-reload.md)), `spec_fragments`, `confine`.

From the command line: `svelters-egui-run --entry <app dir | file> [--dev] [--watch]`, `--app app.svb`, or
`--bundle dist/app.js`. In tests: `harness::TestApp::from_source(path)`.

## What the source loader understands

| File | Becomes |
|---|---|
| `.svelte` | a component (the toolkit's spec plus fragments), with its `<style>` (egui) |
| `.svelte.js`, `.svelte.ts` | a runes module |
| `.js`, `.mjs` | as is |
| `.ts`, `.mts` | types stripped, JS kept as written, with a source map |
| `.json` | `export default <json>` |
| anything else | an asset: `export default "<path>"` |
| `.d.ts`, `.tsx`/`.jsx`, `.cjs`/`.cts`, `.css` | load errors that say why (`import type`; no JSX; no `require`; styles go in `<style>`) |

- Resolution is Node ESM with Vite's client conditions (`browser, import, module, development|production,
  default`), `exports`/`imports` maps, and TypeScript's rule that `./util.js` may name `./util.ts`.
- The runtime's alias table (`packages/native-runtime/package.json` `"svelters".aliases`) is shared with
  Vite. `svelters:behaviors/<ns>` maps to the app's dumped behaviours.
- `import.meta.glob` works (relative literals, `*`, `**`, `{a,b}`, `!`); a new file appears when the
  importing module next compiles.
- TypeScript errors are the `lang="ts"` ones: no enums, value namespaces, decorators or parameter
  properties.
- **Styles** (egui only): a component's `<style>` is compiled to a scoped sheet and installed as text
  from a generated `svelters-style:<id>` module. Other backends warn that styles apply from source on
  egui only.
- Compile errors are load errors made of the rendered diagnostics; warnings go to the console and to
  `provider.take_warnings()`.

### Editor buffers: the overlay

```rust
// not run
let overlay = provider.overlay().clone();    // Arc inside; move it to an editor thread
overlay.set("ui/src/Panel.svelte", buffer_text);
let check = provider.check(Path::new("ui/src/Panel.svelte"));   // diagnostics only, app untouched
embed.host().request_reload();                // or apply_changes(paths): reads the overlay
```

Every read goes to the overlay before the disk. `Check { diagnostics, error }` uses
`svelters_modules::Diagnostic` (re-exported as `svelters_host::modules::Diagnostic`).

## Loader hooks and native modules

`Host::set_module_hooks(ModuleHooks)` wraps every provider in a `ComposedProvider`:

```rust
// not run
use svelters_host::ModuleHooks;
host.set_module_hooks(ModuleHooks::default()                // primary host: every svelters:app/* visible
    .scheme(Rc::new(CadScheme))                              // `cadatomic:` specifiers
    .natives(my_static_modules));
```

- `SchemeHook` serves a scheme of your own (`resolve`, `load`). `svelters` is reserved.
- `Visibility` over `svelters:app/*`: `All`, `Allow(names)`, `Deny(names)` or `Filter(module, importer)`.
  `svelters:core` is always visible. A hidden module fails exactly like an absent one, and
  `construct`/`callStatic` of a hidden export are refused too.
- `NativeModules` / `StaticModules`: modules served from Rust (id + ES source). `DylibModules` (feature
  `dlopen`, off by default) loads them from a library exporting `svl_modules_v1()`; JS only, no native
  ops.
- **Extensions: deny by default.** An extension host uses `ModuleHooks::isolate()`: no app module visible
  and no added native module loadable until the policy allows it. Its sources use
  `SourceConfig::for_extension(path)` (`confine`): no absolute specifiers, and relative imports stay
  inside the importing package. Extension bundle bytecode is ignored unless trusted.

## Release: `.svb` bundles

A `.svb` is one file holding the compiled JS, source maps, assets, manifests, optionally the sources,
and optionally QuickJS bytecode. Module ids are logical (`app/src/App.svelte`), so a bundle contains no
build-machine paths and the same app gives the same bytes from any checkout.

```text
svelters-bundle build ui -o app.svb [--no-sources] [--backend egui] [--precompile]
svelters-bundle precompile app.svb -o app.pre.svb
svelters-bundle ls app.svb
svelters-bundle cat app.svb app/src/App.svelte [--map]
svelters-egui-run --app app.svb
```

From Rust (the builder needs no JS engine, so it can run in `build.rs`):

```rust
// not run
let bytes = svelters_modules::svb::build(cfg, &BuildOptions { sources: true, meta: serde_json::Value::Null })?;
// at run time
let svb = Arc::new(Svb::from_static(include_bytes!(concat!(env!("OUT_DIR"), "/app.svb")))?);
let provider = Rc::new(SvbProvider::new(svb)?);
embed.load_app_from(provider)?;            // or host.load_svb(svb) on a bare Host
```

- Imports resolve through each module's import table: no resolver, no file system.
- Bytecode in the bundle is used only when it was precompiled by **the same build** of the binary that
  runs it (`bytecode::stamp()`) and the provider is trusted. Otherwise the JS is compiled through the
  disk cache (below).
- `Svb::from_static` serves assets zero-copy. Format 1 is uncompressed.

## The bytecode cache

`Host::set_dx(dx)` opens a cache under `dx.paths.cache()/bytecode`; `Host::set_bytecode_cache(Some(dir) |
None)` overrides it. A module's bytecode is keyed by the engine stamp, the module name and its JS text;
damaged entries are misses and are rewritten. HMR's versioned re-imports are never cached.

Release startup of the counter fixture (122 modules, `tests/startup.rs`, one machine):

| Case | Load | First frame |
|---|---|---|
| live compile from source | 83 ms | 93 ms |
| bundle, empty cache (first launch) | 84 ms | 95 ms |
| bundle, warm cache | 20 ms | 30 ms |
| bundle with its own bytecode | 11 ms | 23 ms |

Ship a precompiled bundle when startup matters; the warm cache recovers most of it otherwise.

## Mistakes we have seen

- Importing a `.css` file: use `<style>` in the component.
- Building `precompile` with a debug binary and running a release one: the stamp differs, so the bytecode
  is ignored (it still runs, from JS).
- Giving an extension the primary host's `ModuleHooks::default()`: it then sees every app module.

## Not done

Live compile on top of a release bundle (the layer stack); `deflate`, `memmap2`; extension `.svb`
bundles; native ops from dlopen'd libraries.
