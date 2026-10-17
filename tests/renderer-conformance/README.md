# Renderer conformance

Runs the Svelte custom-renderer API (vendored fork, `vendor/svelte/PIN`) over a plain-JS widget tree and checks the exact op stream a native backend would receive.

- `renderer.mjs`: test renderer implementing all 19 `createRenderer` methods; logs every mutating call (`create`, `insert`/`move`, `set`, `unset`, `text`, `remove`, `listen`/`unlisten`) and counts reads. Not the production renderer (that is `packages/renderer`); this one keeps every node, including fragments and comments, visible in the log.
- `harness.mjs` + `apps/all.mjs`: step-by-step scenarios over the apps in `apps/*.svelte` (counter, if, keyed each, snippets, key, await, attach, props, events with raw args and return values, prop value types, directives, `$bindable` wrapper, the original shim-trial apps) plus `v2.mjs` (value types without spread) and `hmr.mjs` (hot swap).
- `expected/*.txt`: golden output per run (`prod`, `dev`, `values2`, `hmr`). Read counters are excluded from comparison and node ids are ignored (structure only).
- `run.mjs`: bundles each run with esbuild (components compiled with `experimental.customRenderer`, export conditions `custom-renderer` + `browser` + `production|development`), runs the bundle under Node and under quickjs-ng, requires Node output == qjs output, and compares with `expected/`.
- `qjs-prelude.mjs`: the only host globals qjs needs (`setTimeout`/`clearTimeout` from `qjs:os`, `console`); `queueMicrotask` is built in.

## Running

```sh
pnpm install
cd tests/renderer-conformance
node run.mjs              # Node + qjs
node run.mjs --node-only  # skip qjs
node run.mjs --update     # rewrite expected/ from Node output; review the git diff
```

## Where qjs comes from

`get-qjs.sh` downloads the quickjs-ng release binary `qjs-linux-x86_64` from `https://github.com/quickjs-ng/quickjs/releases/download/v0.17.0/` into `.qjs/qjs` (git-ignored) and verifies sha256 `0bfc02511a9f549c28b53880d988fc7cd5d361e90c5e8afdfcd7dc6774ceace5`. `run.mjs` calls it on first use. Use `QJS=/path/to/qjs` to supply another build or platform. The Rust host (P2) embeds QuickJS-NG through `rquickjs`; this binary is only the quick stand-in used here.

Upgrade procedure for the vendored Svelte: `WEEKLY.md`.
