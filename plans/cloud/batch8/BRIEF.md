# svelters cloud batch 8: common brief (for cadatomic)

The user approved two svelters lanes for cadatomic on 2026-10-24 and made the cadatomic batch
coordinator, cloud session `session_01MAioA6KK9ya3iTSXBHf3Q3`, responsible for them while
svelters-agent is down. svelters' `CLAUDE.md` binds you, with these differences:
- **Coordinator:** that session, in svelters-agent's place. It merges your branch into `master`
  (`--no-ff`) once all five CI workflows are green on it. Lanes never ask the user; questions go in
  your milestone record with a recommendation.
- **Your branch** is `ci/b8-<lane>` (the `ci/**` pattern runs the five workflows). Base: `master`
  (`b6efd3cf` or later). Push only there; no pull requests; never push `master`.
- **Your record** is a new milestone file `plans/milestones/<ID>-<name>.md` in the usual form
  (scope, decisions, what landed, tests, CI run ids, handoff). Don't edit `STATUS.md`, `MATRIX.md`
  or `plans/HANDOFF.md`; the coordinator does at the merge.
- **Read first:** `CLAUDE.md`, `plans/HANDOFF.md`, `docs/egui/START-HERE.md` and the pages it names
  for your work, `plans/research/CADATOMIC.md` (the SV rows you serve), `plans/research/TRACKS.md`
  (LISTS, for the tables lane), and cadatomic's request text: `add_repo` `akiselev/cadatomic`
  (read) and its `docs/plan/04-kernel-requests.md` (SV41–SV53).
- **Setup (cloud container, 4 cores, ~39 GB disk):** rustc 1.97.0 is installed; `corepack enable`
  and `pnpm install --frozen-lockfile`; run every cargo command through `tools/dev/cargo-slot`
  with `BUILD_SLOTS=1 BUILD_SLOT_JOBS=4 CHECK_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0`; GTK/Qt tests under
  `tools/dev/headless` only if you touch those backends (you shouldn't). `git config core.hooksPath
  .githooks` once.
- **Build little** (`CLAUDE.md`): `cargo check -p <crate>` while writing; your own tests filtered
  at the end; **GitHub CI is the gate**. Wait for it with `tools/dev/ci-wait <sha> --jobs` (no
  credentials, polls every 600 s; one waiter per sha). Fix what CI reports in one batch.
- **Commit and push often** (quota limits are near): after every item and at least every 30–45
  minutes, WIP or not, with the milestone record kept current from the first push.
- **cadatomic is the consumer:** its batch 8 lane `eu-content` builds the ECAD nets list and
  property grid on your table work in parallel. Keep APIs additive where you can and list every
  public API in your record.
- **Security:** leave room only (`#[non_exhaustive]`, a principal id where cheap); build no policy.

| Lane | Branch | Scope | Merge order |
|---|---|---|---|
| tables | `ci/b8-tables` | LS2 core: Rust row models for `table` and tree rows, columns, benches (SV41, SV44, SV47, SV52) | 1 |
| canvas-seams | `ci/b8-canvas-seams` | SV49 hit tests from a non-scene pane, SV50 `pointermove`, SV51 automation drag/move/wheel, SV53 anchor ids | 2 |

Expected conflicts: `crates/svelters-egui/src/events.rs` (canvas-seams) and the table view
(tables) are disjoint; `ids.rs` is canvas-seams'; both add `docs/egui` pages (separate files).
