# Q4b: `.svb` follow-ups (machine-independent bundles, `--app`, `svelters-bundle`, release timings)

Design: `plans/research/ASSETS.md` §4 (the "Q4 (extended)" row) and the deferred list of
`plans/milestones/Q4-bundle-format.md`. Lane: Q4b, branch `ci/q4b`.

## What landed

### Machine-independent bundles

LI2 (logical ids in the dev provider and the layer stack) has not landed in svelters, so this is done
inside the bundle builder (`svelters_modules::svb::build`, `source/build.rs`). The builder already gave
modules logical ids (`app/src/…`, `app/node_modules/<package>/…`). Now the bundle's contents use them too:

- **The generated entry** (`svelters:entry`) imported the app's files by the build machine's absolute
  path. The builder now rewrites each absolute specifier to the module's logical id
  (`import App from "app/src/App.svelte"`), and the entry's import table is keyed by that id.
- **Source maps**: each absolute `sources` entry becomes the file's logical id. Relative sources are kept.
  Before this, every component map and every package module's map named the checkout.
- **Scoped styles (LD2, egui).** Three things named a component's sheet after the build machine's
  path, and all three now use the component's logical id:
  - **the scope class.** `CompileOptions::css_scope` is now `svelte-<hash(logical id)>`; it was
    Svelte's default `hash(filename)` of the absolute path. This is set in `transform.rs`
    (`Compiler::root`), so the dev provider gives the same class names as the bundle.
  - **the sheet key.** `registerStyles("app/src/App.svelte", …)` is the logical id, in dev too. The
    key is stable for each file, so LD2's style-only HMR replaces the sheet as before.
  - **the component's sheet import.** The builder rewrites `svelters-style:<abs>` to
    `svelters-style:app/src/App.svelte`, the same rewrite as the entry's. LD2's `Logical::id` hunk
    already names that module this way. The dev provider keeps the absolute specifier, which its
    resolver checks against the importing component.
  - Logical ids come from one function, `source::build::bundle_path`/`logical_id`. `Logical::rel`
    and the dev side share it.

The bundle no longer contains the checkout path. The same app built from two checkout paths gives the
same bytes, so the same `build` id. When LI2 lands, the dev provider's ids can follow the same scheme,
and the rewrite here becomes a no-op.

### Runner `--app <file.svb>`

`svelters-egui-run --app app.svb` opens the bundle (`Svb::open`, in svelters-modules) and loads it
through `SvbProvider` with this binary's `bytecode::stamp()`. A bundle precompiled by the same build
runs its bytecode. Any other bundle compiles its JS through Q2's disk cache (the runner gets the cache
from `set_dx`, as before). `--bundle`, `--entry`, `--dev`, `--watch` and `--idle-report` are unchanged.

### `svelters-bundle` (svelters-egui binary)

It was small (about 120 lines over the existing APIs), so it is in this lane:

```text
svelters-bundle build <app dir | file> -o <app.svb> [--no-sources] [--backend <name>] [--precompile]
svelters-bundle precompile <app.svb> -o <out.svb>
svelters-bundle ls <app.svb>                       # index summary, modules (js/map/bytecode sizes), files, sections
svelters-bundle cat <app.svb> <module id | file path> [--map]
```

The backend defaults to `egui`. `precompile` stamps the bytecode with the engine of the binary that ran it,
which must be the same build (release or debug, and the same svelters-host) as the runner that will
load it. Any other runner ignores that bytecode and compiles the JS. The binary sits in svelters-egui
because that crate already enables the `source` feature and links svelters-host. It carries a
file-level `#[allow(clippy::disallowed_methods)]` because it is a CLI, not the UI thread.

`TestApp::with_loader(cfg, load)` (kittest) is new: it gives the `Embed` to a closure before anything is
loaded, so that a test can set a bytecode cache or load a `.svb`.

## Release startup timings

`crates/svelters-egui/tests/startup.rs`, `release_startup_timings` (`#[ignore]`, not in CI):

```text
tools/dev/cargo-slot cargo test --release -p svelters-egui --test startup -- --ignored --nocapture
```

The fixture is counter-egui: 122 modules (the app, `@svelters/native-runtime`, Svelte's client
runtime). The bundle is 1,033,347 B, or 2,304,304 B precompiled. Building it took 323.9 ms, and
precompiling it 69.0 ms. The test ran headless under kittest, release build, on thinkpadx1 (i7-8850H,
12 threads) with another lane's build running. Each case is the median of 7 launches, each on a new
embed and QuickJS runtime.

- **Load** is `Embed::load_app_from`: resolving, compiling or reading bytecode, and evaluating every
  module, plus the first job drain. That is the module-eval time.
- **First frame** is the time from opening the `.svb` (reading and validating it) to the end of the
  first egui frame. It includes creating the embed.

| Case | What it is | Load | First frame |
|---|---|---|---|
| source | `--entry`: compiled live by the Rust compiler, no bytecode cache | 83.2 ms | 93.3 ms |
| cold | bundle JS, empty Q2 disk cache (first launch: compile and write) | 83.6 ms | 94.9 ms |
| warm | bundle JS, the cache the cold launch filled | 19.6 ms | 30.4 ms |
| precompiled | the bundle's own bytecode (`bytecode::precompile`), cache empty | 11.1 ms | 23.4 ms |

What the numbers show:

- QuickJS compiling the JS dominates a cold start, about 64 ms of the 84 ms. The Rust Svelte compile of
  the app's own two components is small: source and cold are within noise, and the bundle's resolve
  is a table lookup.
- Q2's disk cache recovers most of it (warm load is 4.3× faster). Bundle bytecode is better still
  (7.5× faster than cold): it skips the cache's file reads and hash checks, at the cost of 2.2× the
  bundle size.
- With bytecode, about 12 ms of the first frame is embed creation and the first egui pass, not JS.

## Tests

- `svelters-host/tests/modules.rs`, `a_bundle_does_not_depend_on_the_checkout_path` (new). The counter
  app, with a `<style>` added and built for egui, is built from two scratch copies at different
  paths. The test checks that:
  - the entry imports `app/src/App.svelte`;
  - the component imports `svelters-style:app/src/App.svelte`, whose module registers the sheet under
    `app/src/App.svelte`;
  - neither bundle contains the repo path;
  - the build ids and the bytes are equal.
- `an_app_loads_from_its_release_bundle_without_its_sources` (Q4), LD2's
  `a_style_only_edit_reruns_only_the_sheet_module`, and `svelters-egui/tests/live_styles.rs` pass with the
  logical-id entry, scope and key.
- `svelters-egui/tests/startup.rs` (ignored), above.

## Deferred

- The `Layer` stack and logical ids in dev (LI2), so live compile works on top of a release bundle.
- `--precompile` as a hidden runner flag. `svelters-bundle precompile` covers it.
- Globs, rasters, `deflate` and `memmap2`, and extension bundles (AS3), as in Q4.
