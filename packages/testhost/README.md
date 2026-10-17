# @svelters/testhost

Node `SveltersHost` recording the op log, virtual timers, and `svelters-test <fixtureDir> [--update]` which runs `dist/app.js` in a bare `vm` context. Needs `node --experimental-vm-modules`. Step semantics: `docs/notes/p1-js-lane.md`.

Prop coercion follows a coarse-type schema per toolkit vocabulary: `src/schema.json` (GTK, the default) or `src/schema.qt.json` (Qt). A fixture in the Qt vocabulary sets `"svelters": { "toolkit": "qt" }` in its `package.json`; `runBundle({ toolkit: 'qt' })` / `createTestHost({ toolkit: 'qt' })` also answer the QSS style dialect.
