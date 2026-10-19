---
title: Getting started
description: Build a fixture bundle and run the counter app on the GTK4 and Qt backends.
---

Svelters compiles a Svelte 5 component with its own compiler, `svelters-compile`, bundles it with Vite into one ES module, loads that into an embedded QuickJS host, and renders the result with GTK4 or QtWidgets. This page builds the `counter` fixture and its Qt twin and runs them.

:::note[State of the project]
Both backends run compiled bundles today. A bundle is compiled for one toolkit, because element names and props are each toolkit's own. There is no installable release and no `create-svelters` template. You work inside the repository checkout.
:::

## Prerequisites

| Tool | Version | Used for |
| --- | --- | --- |
| Node.js | 22 or newer (`engines.node` in the root `package.json`) | Vite build, JS tests |
| pnpm | 10 (`packageManager: pnpm@10.33.0`; `corepack pnpm` works) | workspace install |
| Rust | pinned by `rust-toolchain.toml` (channel `1.97.0`, with `rustfmt` and `clippy`) | the compiler, host, GTK backend, `counter` binary |
| GTK 4 development packages | notes were taken on GTK 4.22.4 | GTK backend |
| Qt 6 development packages with `pkg-config` `Qt6Widgets` | notes were taken on Qt 6.11 | Qt backend (compiled by `cc` from a `build.rs`) |
| Python 3 | any recent 3 | the Qt generator runs from `build.rs`; the metadata ingesters and generators are Python |
| Xvfb (`xvfb-run`) | any | headless runs, screenshots and the integration tests |

The exact package names depend on your distribution. The project notes do not list them. See `docs/notes/p5-integration.md` for the environment the baselines were measured on (Arch Linux, GTK 4.22.4, Qt 6.11, Xvfb).

## Build the compiler

Every `.svelte` file is compiled by the `svelters-compile` binary, so build it before any bundle:

```sh
cargo build -p svelters-compiler --bin svelters-compile
```

The Vite plugin uses the newest of `target/release/svelters-compile` and `target/debug/svelters-compile`, or the path in `SVELTERS_COMPILE`. Rebuild it after pulling compiler changes; a `.svelte` edit never needs it rebuilt.

## Build a fixture bundle

Fixtures live in `fixtures/`: `counter`, `keyed-list`, `if-toggle`, `bind-value`, `menus`, `dialogs`, `styles` and more (`fixtures/README.md`). Each has `src/App.svelte`, `script.json` and an `expected.oplog`. Fixtures are written in the GTK vocabulary; `<name>-qt` is the Qt twin.

```sh
pnpm install
pnpm -r build          # writes fixtures/<name>/dist/app.js for every fixture
```

To build one fixture, run `pnpm build` inside its directory. `pnpm test` builds first and then compares the bundle's op log with `expected.oplog` byte for byte.

```sh
cd fixtures/counter
pnpm build
```

The bundle is one self-contained ES module (the native runtime, Svelte's signal runtime and the compiled app) with no `import`, `export` or dynamic `import()`. A source map is written beside it.

## The counter component

`fixtures/counter/src/App.svelte`:

```svelte
<script>
  let count = $state(0);
</script>

<window title="Counter">
  <box orientation="vertical" spacing="8">
    <label>Count: {count}</label>
    <button onclicked={() => count++}>Increment</button>
  </box>
</window>
```

What each part does:

- `<window>`, `<box>`, `<label>` and `<button>` are not HTML. They are native element tags: the native class name without its toolkit prefix, in kebab case. The compiler checks each against the GTK semantic spec and turns it into a `create <kind>` host operation; the GTK backend maps the tag to `GtkWindow`, `GtkBox`, `GtkLabel` and `GtkButton`. Each toolkit has its own vocabulary, so the Qt twin (`fixtures/counter-qt`) writes `<main-window window-title="Counter">`, `<box-layout direction="top-to-bottom">`, `<label>` and `<push-button>` (`QMainWindow`, a `QWidget` carrying a `QBoxLayout`, `QLabel`, `QPushButton`). Per-element pages: [GTK button](/api/gtk/button/), [Qt push-button](/api/qt/push-button/).
- `title`, `orientation` and `spacing` are properties set with `set_prop`. Static values are typed at compile time from the spec: `spacing="8"` is emitted as the integer 8, and `spacing="abc"` is a compile error.
- Text inside `<label>` and `<button>` becomes their `label` property (`text` on Qt). Text in an element without a text property is a compile error (`text_not_allowed`).
- `onclicked` listens to the native `clicked` signal (handlers are `on` plus the native signal name; there are no DOM aliases such as `onclick`). The runtime calls `listen(button, clicked, on)`, and a real click comes back as a queued event that runs the handler. See [Events](/guides/events/).
- `$state` drives updates. `count++` re-runs only the label's text effect, which produces one `set_prop` call.

The fixture's `vite.config.js` is two lines of configuration on top of `@svelters/build`:

```js
import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

export default defineConfig(svelteConfig(import.meta.dirname));
```

`svelteConfig(root)` compiles `src/App.svelte` and everything it imports for one toolkit and writes `dist/app.js`. The toolkit comes from the app's `package.json`: `"svelters": { "toolkit": "qt" }` (default `gtk`; also `appkit` and `winui`). `svelteConfig(root, { toolkit, dev })` overrides it; `dev: true` keeps `$inspect`. `svelters-build [--watch] [dir]` runs the same build from the command line.

Element typings for editors and `svelte-check` are in `@svelters/native-runtime/types` (add it to `types` in the app's `tsconfig.json`).

## Run the counter

The `counter` binary is `examples/counter`. One binary serves both toolkits through `--backend`.

```sh
# GTK4
cargo run -p counter -- --backend gtk --fixture counter

# Qt (the Qt twin, compiled for the Qt vocabulary)
cargo run -p counter -- --backend qt --fixture counter-qt
```

`--fixture` takes a fixture name or a directory that contains `dist/app.js`. With no `--script`, `--screenshot` or `--report`, the binary is an ordinary app that runs until its window closes.

### Headless run with a screenshot

```sh
xvfb-run -a cargo run -p counter -- --backend gtk --fixture counter --screenshot out.png
xvfb-run -a cargo run -p counter -- --backend qt  --fixture counter-qt --screenshot out.png
```

The option list (`examples/counter/src/main.rs`):

```text
counter --backend gtk|qt [--fixture <name|dir>] [--script f] [--screenshot png]
        [--report json] [--end quit|close] [--timing] [--timeout secs]
        [--watch [--reload-exit n]]
```

| Option | Effect |
| --- | --- |
| `--script f` | Replay a JSON step file (`{"event": [h, "clicked", []]}`, `{"checkpoint": true}`) inside the toolkit loop. A `clicked` step becomes a real button click. |
| `--screenshot png` | Save a PNG of the window after the script. |
| `--report json` | Write the widget tree read back from the toolkit, plus what the tests assert on. |
| `--end quit\|close` | Finish by asking the backend to quit, or by closing the window natively so `run()` must return on its own. |
| `--timing` | Turn on the time-split counters (JS, host/ABI, toolkit). |
| `--watch` | Reload `dist/app.js` in the running host whenever it is rebuilt (run `pnpm watch` in the fixture next to it). State resets on reload. |

Handles in scripts are backend handles. Both backends number them 1, 2, ... in creation order, the same order as `expected.oplog`.

### Build only one toolkit

`examples/counter` has features `gtk` and `qt`, both on by default.

```sh
cargo build --release -p counter --no-default-features --features gtk
cargo build --release -p counter --no-default-features --features qt
```

## Run the tests

```sh
pnpm test                                              # JS packages, every fixture, compiled harness, conformance ratchet, metadata checks
tools/dev/headless cargo nextest run --workspace       # Rust, incl. GTK/Qt end-to-end and screenshot tests on a private Xvfb
tools/dev/headless cargo nextest run -p counter        # only the counter integration tests
```

Build the compiler and the bundles first (`cargo build -p svelters-compiler --bin svelters-compile`, `pnpm install && pnpm -r build`). `pnpm test` builds every fixture with the compiler and compares its op log with `expected.oplog` on the Node `TestHost`. The Rust log backend replays the same bundles in QuickJS, so Node and QuickJS must agree. `xvfb-run -a` works in place of `tools/dev/headless` where X11 sockets are writable. Screenshots from the integration tests land in `target/p5/<backend>-<fixture>.png`.

## What to try next

- Edit `fixtures/counter/src/App.svelte`, run `pnpm build` in that directory, and rerun `cargo run`, or keep `pnpm watch` and `cargo run -p counter -- --backend gtk --fixture counter --watch` running. No Rust or C++ recompilation is triggered by a `.svelte` edit: the Qt backend is compiled by a leaf crate (`examples/counter-qt`) whose build script does not rerun when `examples/counter/src/*` changes.
- Read [Architecture](/guides/architecture/) for how the layers fit together.
- Look at [`<button>` on GTK](/api/gtk/button/) and [`<push-button>` on Qt](/api/qt/push-button/) to see which properties and signals each backend exposes.
- Check [Svelte support](/guides/svelte-support/) before using a Svelte feature that is not in the fixtures.

Sources: `plans/milestones/P5-integration.md`, `docs/notes/p5-integration.md`, `fixtures/README.md`, `fixtures/counter/`, `fixtures/counter-qt/`, `packages/build/index.js`, `packages/native-runtime/vite.js`, `examples/counter/src/main.rs`, `examples/counter/Cargo.toml`, `STATUS.md` (handoff), `package.json`, `rust-toolchain.toml`, `README.md`.
