# Fixtures

Each fixture: `src/App.svelte`, `src/main.js`, `script.json` (steps), `expected.oplog` (byte-exact op log). `pnpm build` writes `dist/app.js`; `pnpm test` builds and compares. Regenerate a log with `UPDATE=1 pnpm test` inside the fixture, then review the diff.

- `counter`, `keyed-list`, `if-toggle`: interaction and tree ops.
- `native-close`: a child window closed natively (`{"gone": [5, "closed"]}` in the script), then toggled by the app; the log shows `gone` lines and no ops for the dead handles.
- `timers`: `setTimeout` + promise chain updating a label (for the host job pump).
