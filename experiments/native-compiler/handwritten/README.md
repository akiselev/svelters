# Hand-written components in the X1 generated-code shape

Each directory holds `App.js`, written the way the X1 compiler is to emit
`fixtures/<name>/src/App.svelte` (`docs/notes/native-compiler/runtime-contract.md`), a `main.js`
entry calling `$n.mount`, the fixture's `script.json` (copied unchanged: the same handle numbers
drive both pipelines) and our own `expected.oplog`.

```sh
pnpm --filter @svelters-experiments/handwritten test   # build + run on the Node TestHost
UPDATE=1 node --experimental-vm-modules run.mjs        # rewrite our expected.oplog files
cargo test -p svelters-compiler --test runtime_bundles # same bundles on the Rust log backend
node tools/oplog-compare.mjs fixtures/<name>/expected.oplog experiments/native-compiler/handwritten/<name>/expected.oplog
```

`run.mjs` checks our log byte for byte, then replays it and the JS pipeline's
`fixtures/<name>/expected.oplog` with `tools/oplog-compare.mjs` and compares the native states
(kinds, props, child order per slot, live handles, listened signals, calls) before every
`event`/`gone` line and at the end.

| Fixture | vs JS pipeline | Ops (ours / JS) |
| --- | --- | --- |
| counter | identical log | 16 / 16 |
| if-toggle | identical log | 32 / 32 |
| timers | identical log | 9 / 9 |
| keyed-list | state-equal at all 9 boundaries, order differs | 78 / 78 |
| native-close | state-equal at all 4 boundaries, order differs | 28 / 28 |
| keyed-stress | state-equal at the end (no boundaries: one checkpoint), order differs, one redundant op | 98 / 97 |

## Why the logs differ

- **Effect order within an update** (keyed-list `set #4 text ""`, native-close
  `set #3 label "child open"`). Our template effects are created in source order (the contract's
  initial op order), and Svelte runs dirty render effects in effect-tree order, so the entry's
  text effect runs before the `{#each}` below it and the label before the `{#if}`. Svelte's own
  output groups a fragment's template effects after its blocks, so the JS pipeline updates the
  blocks first. Same ops, same final state per step.
- **Which items move** (keyed-list Reverse, every keyed-stress step). Our keyed each keeps a
  longest increasing subsequence in place and moves the rest back to front; Svelte's algorithm
  picks other items. Move counts are equal here (keyed-list reverse: 2 vs 2; keyed-stress: 18
  remove+insert moves in both logs) and the unit tests check minimality (reverse n-1, rotate 1,
  insert in the middle 0). Removed items are removed and destroyed one by one (ours) instead of
  all removes first (Svelte).
- **`set #3 label ""`** (keyed-stress). The label's text is `{expected}`, initially `''`. The
  renderer folds text children into `label` and skips an empty label; generated code writes the
  first value of every dynamic prop. Redundant (GTK/Qt labels start empty). `run.mjs` passes
  `label=""` as a default to the comparison, though at the end the label is set in both logs.
- **keyed-list entry echo.** `text={draft}` with `onchanged` would echo `set #4 text "alpha"`
  after each `changed` event. The JS renderer hard-codes a mirror for `entry`/`changed`; generated
  code uses `$n.track(n4, 'text', 'changed', 0)` from the spec binding (contract addition), so
  there is no echo and the logs agree.

The native-close fixture also exercises liveness: after `gone #5`/`gone #6` no op targets the dead
window or label, toggling the branch off emits nothing, and toggling it on builds `#7`/`#8`.
