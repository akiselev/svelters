# Fixtures

Each fixture: `src/App.svelte`, `src/main.js`, `script.json` (steps), `expected.oplog` (byte-exact op log). `pnpm build` writes `dist/app.js`; `pnpm test` builds and compares. Regenerate a log with `UPDATE=1 pnpm test` inside the fixture, then review the diff.

- `counter`, `keyed-list`, `if-toggle`: interaction and tree ops.
- `timers`: `setTimeout` + promise chain updating a label (for the host job pump).
