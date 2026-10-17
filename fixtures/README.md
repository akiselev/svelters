# Fixtures

Each fixture: `src/App.svelte`, `src/main.js`, `script.json` (steps), `expected.oplog` (byte-exact op log). `pnpm build` writes `dist/app.js`; `pnpm test` builds and compares. Regenerate a log with `UPDATE=1 pnpm test` inside the fixture, then review the diff.

- `counter`, `keyed-list`, `if-toggle`: interaction and tree ops.
- `timers`: `setTimeout` + promise chain updating a label (for the host job pump).
- `bind-this`, `bind-value` (renderer backend only; `SVELTERS_BACKEND=shim` skips them): element `bind:this` (native `getProp`/`call` through the node, `StaleHandleError` after destroy) and `bind:text` on an entry with echo suppression. The TestHost does not model natively typed text, so its log shows the seed text where the real toolkits (checked by `examples/counter/tests/integration.rs`) read the typed text.
