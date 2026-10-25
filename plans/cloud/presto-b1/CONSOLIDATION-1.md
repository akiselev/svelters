# presto consolidation 1 (2026-10-25)

You are the consolidation session for the presto build-out. Read `CLAUDE.md`, then
`plans/cloud/presto-b1/COORDINATOR.md` (lanes, decisions, the "For the consolidation" list), then this file.
The coordinator is `session_01CqZ417NuvkRujZaYSm7Pwo`; report to it with `send_message`.

Lanes are still running. This first consolidation finds integration breaks early. A second one picks up the
lanes' final heads.

## Branch

Create `ci/presto-integration` from `origin/ci/presto-base`. Push it early, and only to that branch. Never push
`master` or another lane's branch, and never force-push.

## Merge order (`--no-ff`, the current `origin` head of each)

1. `ci/presto-runtime`
2. `ci/presto-compiler`
3. `ci/presto-text-input`
4. `ci/presto-widgets-views`
5. `ci/presto-widgets-tier1`
6. `ci/presto-svelters-presto`
7. `ci/presto-egui-island`
8. `ci/presto-motion-css`
9. `ci/presto-win32`
10. `ci/presto-wayland`
11. `ci/presto-macos`

`ci/presto-base` already holds core, core-2, core-3, text, render, frame-anim, compositor, platform,
widgets-basic and app-test.

On each conflict:
- Regenerate generated files with their generator; never hand-merge them.
- Resolve `Cargo.lock` with `cargo update -w` or by letting cargo rewrite it.
- When two lanes changed the same shared file (for example `presto-platform`'s Clipboard, presto-base enums, or
  `presto-widgets` host.rs), keep both additions.

After each merge, run `cargo check` on the crates that merge touched.

## What you fix, and what you record

You fix build breaks only, so that everything compiles together. Examples: a renamed type, a missing match arm
for a new enum variant (`ToUi::Occluded`, `ToUi::FrameTimings`), or a signature that moved under a lane.

Everything else goes into `plans/cloud/presto-b1/FIX-QUEUE.md`: one row per item, with the crate, the test or
symptom, the owning lane and a suggested fix. That covers test failures, behaviour bugs, design gaps and the
items in COORDINATOR.md's "For the consolidation" list.

## Checks

1. `cargo check --workspace --all-targets`
2. Filtered tests for each presto crate:
   `cargo test -p presto-base -p presto-core -p presto-text -p presto-anim -p presto-frame -p presto-compositor -p presto-platform -p presto-widgets -p presto -p presto-test -p svelters-presto`
   Run them once and record the failures; don't loop on them.
3. Push, then wait on CI with `tools/dev/ci-wait <sha> --jobs`. The runners are backlogged, so expect a long wait.
   The coordinator needs all six workflows green: js, linux-gtk, linux-qt, macos-appkit, windows-winui and presto.
4. Fix CI build breaks (compile errors, a missing apt package) and push again. Record test failures in the
   FIX-QUEUE and send each to the coordinator, naming the owning lane, so it can route the fix.

## Report

Send the coordinator:
- the integration sha;
- the merged heads;
- the build fixes you made;
- the local test counts per crate;
- the CI run ids and results;
- the FIX-QUEUE path.

Then stop.
