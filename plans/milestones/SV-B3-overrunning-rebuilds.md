# SV-B3: SV87, a block rebuild that overruns the budget again makes progress

Lane `ci/sv-b3` (2026-10-28), from svelters master `17057b8`. Request row SV87 of cadatomic's request
table (coordinator brief, 2026-10-28).

| Item | What | Test |
| --- | --- | --- |
| SV87 | A rebuild the turn budget stops again gets a doubled budget on each retry (up to `HostConfig::repair_budget`, default 2 s), the host reports the interrupt once, and a rebuild past that budget leaves its block empty | `crates/svelters-skelter/tests/overrunning_rebuilds.rs` (6 tests) |

## The bug

SV84 (`ac247f7`, lane `ci/sv-b2`) rebuilds a block the host interrupted mid-update from nothing on
its next run: `{#each}`, and `{#if}`/`{#key}`/`{#await}`/snippet/component blocks through
`Branches`. When that rebuild costs more than the interrupt budget (50 ms by default; a plain
`{#each}` over ~50k rows takes ~100 ms per debug pass), it is interrupted again, the block is marked
again and rebuilt from nothing again, forever: the host never settles, and every retry reports
another interrupt (132 in one host test). Before `ac247f7` the block stayed half built, but the host
settled.

## Design: a growing budget for the retries (and a cap)

Two ways for the retry to make progress:

1. **Resume where it stopped.** Keep the items the interrupted update finished and build only the
   rest. This only works for `{#each}` (an item is the unit of work). An `{#if}`, `{#key}` or
   `{#await}` branch is built in one go: a branch that costs more than the budget can never be built
   under that budget, so resuming cannot help those, and the request asks for all four kinds. It
   would also need `keyed`/`indexed` to keep their item list consistent at every step.
2. **A growing budget** (chosen). Each time an abort stops a block's update again, the host doubles
   the budget of the next entries, so a rebuild of any finite cost finishes after
   `log2(cost / budget)` retries. It covers every block kind with one rule in one place, and keeps
   SV84's rebuild from nothing (so its guarantee of no stale nodes is unchanged).

Without a limit, a block that never finishes (a loop that never ends, a rebuild that costs minutes)
would freeze the host for 50 ms, 100 ms, 200 ms, ... forever. So the growth stops at
`HostConfig::repair_budget` (2 s by default; never below the turn budget itself). A retry stopped
under that budget **gives up**: the block is scrapped as usual (no stale nodes) and left empty, its
guard reset, so the next change of its inputs builds it again. The host settles in every case.

How it works:

- `packages/native-runtime`: `recover(empty)` returns how many blocks the abort stopped mid-update
  (`abortUpdates`); `empty` marks them to stay empty. A marked block's next run scraps every region
  (`repairStart`, shared by `each_block` and `Branches.repair`) and then rebuilds (REBUILD) or returns
  (EMPTY). The scrap itself is now registered as updating, so an abort during it marks the block
  again (before, it cleared the mark first). `globalThis.__svelters_repairing()` says whether a
  marked block still has to run its rebuild (blocks destroyed meanwhile are dropped).
- `crates/svelters-host` (`quickjs.rs`): `repair_level` counts the aborts in a row that stopped a
  block's update. While it is above 0, `arm` gives entries the turn budget doubled that many times,
  at most `repair_budget`. It goes back to 0 when an entry ends cleanly and the runtime has no
  rebuild left. An abort when the armed budget was already the largest calls `recover(true)`.
- **One report.** An aborted job's interrupt is held until `recover` says what it stopped: the first
  one is reported (`EmbedEvent::Error { root: None }`, kind `Interrupted`), the retries' are not.
  A give-up is reported once more (kind `Interrupted`, the message says the block stays empty), so
  a host sees at most two events for one overrunning block, never a stream.
- **Finding: blocks left behind by an aborted branch.** Svelte links an effect to its parent only
  after its first run ends. A block inside a branch the abort stopped mid-build (an `{#each}` in an
  `{#if}` branch) is never linked, so destroying the branch does not mark it destroyed. Its entry in
  the repair set would keep the budget grown for good; `repairing()` drops a block when its effect
  or any effect above it is destroyed. (Its natives go with the branch's scope as before.)
- While `repair_level` is above 0, every entry gets the grown budget, not only the one running the
  rebuild (the host cannot tell which entry will run it). That lasts until the rebuild finishes.

## Tests

`crates/svelters-skelter/tests/overrunning_rebuilds.rs`, interrupt budget 100 ms, eight labels per
block at 32 ms each (~256 ms per rebuild: interrupted at 100 ms and 200 ms, done at 400 ms):

- `sv87_an_overrunning_{each,if,key,await}_rebuild_settles`: two steps each; every step settles
  within 12 passes, the block holds exactly the step's labels, and exactly one interrupt is reported.
  These failed before the fix (no settle, the interrupt reported on every pass).
- `sv87_a_rebuild_past_the_largest_budget_leaves_the_{each,if}_block_empty`: `repair_budget` 200 ms,
  ~480 ms per rebuild: the block ends empty, the host settles, at most two events; with the cost back
  to 0 the next step builds the block.

## Not run

Only the tests above and SV84's (`keyed_each.rs`, `interrupted_blocks.rs`) ran locally; everything
else is left to CI.
