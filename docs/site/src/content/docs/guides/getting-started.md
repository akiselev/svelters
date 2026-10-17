---
title: Getting started
description: Build a fixture bundle and run the counter app on the GTK4 and Qt backends.
---

Svelters compiles a stock Svelte 5 component with Vite into one ES module, loads it into an embedded QuickJS host, and renders the result with GTK4 or QtWidgets. This page builds the `counter` fixture and runs it on both toolkits.

:::note[State of the project]
Both backends run the same bundle today (milestone P5, done). There is no installable release, no `create-svelters` template, and no dev loop that reloads a bundle in a running host; all of that is milestone M2 or later. You work inside the repository checkout.
:::

## Prerequisites

| Tool | Version | Used for |
| --- | --- | --- |
| Node.js | 22 or newer (`engines.node` in the root `package.json`) | Vite build, JS tests |
| pnpm | 10 (`packageManager: pnpm@10.33.0`) | workspace install |
| Rust | pinned by `rust-toolchain.toml` (channel `1.97.0`, with `rustfmt` and `clippy`) | host, GTK backend, `counter` binary |
| GTK 4 development packages | notes were taken on GTK 4.22.4 | GTK backend |
| Qt 6 development packages with `pkg-config` `Qt6Widgets` | notes were taken on Qt 6.11 | Qt backend (compiled by `cc` from a `build.rs`) |
| Python 3 | any recent 3 | the Qt generator runs from `build.rs`; the metadata ingesters and generators are Python |
| Xvfb (`xvfb-run`) | any | headless runs, screenshots and the integration tests |

The exact package names depend on your distribution. The project notes do not list them. See `docs/notes/p5-integration.md` for the environment the baselines were measured on (Arch Linux, GTK 4.22.4, Qt 6.11, Xvfb).

## Build a fixture bundle

Fixtures live in `fixtures/`: `counter`, `keyed-list`, `keyed-stress`, `if-toggle` and `timers`. Each has `src/App.svelte`, `src/main.js`, `script.json` and an `expected.oplog`.

```sh
pnpm install
pnpm -r build          # writes fixtures/<name>/dist/app.js for every fixture
```

To build one fixture, run `pnpm build` inside its directory. `pnpm test` builds first and then compares the bundle's op log with `expected.oplog` byte for byte.

```sh
cd fixtures/counter
pnpm build
```

The bundle is one self-contained ES module (Svelte runtime, renderer and app) with no `import`, `export` or dynamic `import()`. A source map is written beside it.

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

- `<window>`, `<box>`, `<label>` and `<button>` are not HTML. They are native element tags: the native class name without its toolkit prefix, in kebab case. The renderer turns each into a `create <kind>` host operation; the GTK backend maps the tag to `GtkWindow`, `GtkBox`, `GtkLabel` and `GtkButton`. Each toolkit has its own vocabulary, so the Qt twin (`fixtures/counter-qt`) writes `<main-window window-title="Counter">`, `<box-layout direction="top-to-bottom">`, `<label>` and `<push-button>` (`QMainWindow`, a `QWidget` carrying a `QBoxLayout`, `QLabel`, `QPushButton`). Per-element pages: [GTK button](/api/gtk/button/), [Qt push-button](/api/qt/push-button/).
- `title`, `orientation` and `spacing` are properties set with `set_prop`. `spacing="8"` arrives as the string `"8"` and the renderer coerces it to an integer (`INT_PROPS` in `packages/renderer/src/index.js`).
- Text inside `<label>` and `<button>` becomes their `label` property (`text` on Qt). Text anywhere else is dropped.
- `onclicked` listens to the native `clicked` signal (handlers are `on` plus the native signal name; there are no DOM aliases such as `onclick`). The renderer calls `listen(button, clicked, on)`, and a real click comes back as a queued event that runs the handler. See [Events](/guides/events/).
- `$state` drives updates. `count++` re-runs only the label's text effect, which produces one `set_prop` call.

The fixture's `vite.config.js` is two lines of configuration on top of `@svelters/build`:

```js
import { defineConfig } from 'vite';
import { svelteConfig } from '@svelters/build';

export default defineConfig(svelteConfig(import.meta.dirname));
```

## Run the counter

The `counter` binary is `examples/counter`. One binary serves both toolkits through `--backend`.

```sh
# GTK4
cargo run -p counter -- --backend gtk --fixture counter

# Qt
cargo run -p counter -- --backend qt --fixture counter
```

`--fixture` takes a fixture name or a directory that contains `dist/app.js`. With no `--script`, `--screenshot` or `--report`, the binary is an ordinary app that runs until its window closes.

### Headless run with a screenshot

```sh
xvfb-run -a cargo run -p counter -- --backend gtk --fixture counter --screenshot out.png
xvfb-run -a cargo run -p counter -- --backend qt  --fixture counter --screenshot out.png
```

The full option list from the P5 completion record:

```text
counter --backend gtk|qt [--fixture <name|dir>] [--script f] [--screenshot png]
        [--report json] [--end quit|close] [--timing]
```

| Option | Effect |
| --- | --- |
| `--script f` | Replay a JSON step file (`{"event": [h, "clicked", []]}`, `{"checkpoint": true}`) inside the toolkit loop. A `clicked` step becomes a real button click. |
| `--screenshot png` | Save a PNG of the window after the script. |
| `--report json` | Write the widget tree read back from the toolkit, plus what the tests assert on. |
| `--end quit\|close` | Finish by asking the backend to quit, or by closing the window natively so `run()` must return on its own. |
| `--timing` | Turn on the time-split counters (JS, host/ABI, toolkit). |

Handles in scripts are backend handles. Both backends number them 1, 2, ... in creation order, the same order as `expected.oplog`.

### Build only one toolkit

`examples/counter` has features `gtk` and `qt`, both on by default.

```sh
cargo build --release -p counter --no-default-features --features gtk
cargo build --release -p counter --no-default-features --features qt
```

## Run the tests

```sh
pnpm test                                              # renderer unit tests, 5 fixtures, node == QuickJS conformance
xvfb-run -a cargo nextest run --workspace --test-threads 4
xvfb-run -a cargo nextest run -p counter               # only the 12 integration tests
```

Build the bundles first (`pnpm install && pnpm -r build`). The P5 completion record reports 57 of 57 nextest tests passing, 12 of them in `counter::integration`. Screenshots from those runs land in `target/p5/<backend>-<fixture>.png`.

## What to try next

- Edit `fixtures/counter/src/App.svelte`, run `pnpm build` in that directory, and rerun `cargo run`. No Rust or C++ recompilation is triggered by a `.svelte` edit: the Qt backend is compiled by a leaf crate (`examples/counter-qt`) whose build script does not rerun when `examples/counter/src/*` changes.
- Read [Architecture](/guides/architecture/) for how the layers fit together.
- Look at [`<button>` on GTK](/api/gtk/button/) and [`<push-button>` on Qt](/api/qt/push-button/) to see which properties and signals each backend exposes.
- Check [Svelte support](/guides/svelte-support/) before using a Svelte feature that is not in the fixtures.

Sources: `plans/milestones/P5-integration.md`, `docs/notes/p5-integration.md`, `fixtures/README.md`, `fixtures/counter/`, `examples/counter/Cargo.toml`, `package.json`, `rust-toolchain.toml`, `README.md`.
