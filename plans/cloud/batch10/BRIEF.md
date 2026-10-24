# svelters cloud batch 10: common brief (for cadatomic)

The user approved these svelters lanes on 2026-10-24 ("Lets build the svelters lane now"; batch 10 planned
and started the same day) and made the cadatomic batch coordinator, cloud session
`session_01RQkPkX9QqiFN52Rrofmi7z`, responsible for them while svelters-agent is down. svelters' `CLAUDE.md`
binds you, with these differences (as in batch 8, `plans/cloud/batch8/BRIEF.md`):
- **Coordinator:** that session, in svelters-agent's place. It merges your branch into `master` (`--no-ff`)
  once all five CI workflows are green on it. Lanes never ask the user; questions go in your milestone record
  with a recommendation.
- **Your branch** is `ci/b10-<lane>` (the `ci/**` pattern runs the five workflows). Base: `master`
  (`7196c95d` or later). Push only there; no pull requests; never push `master`.
- **Your record** is a new milestone file `plans/milestones/<ID>-<name>.md` (scope, decisions, what landed,
  public API, tests, CI run ids, handoff). Don't edit `STATUS.md`, `MATRIX.md` or `plans/HANDOFF.md`.
- **Read first:** `CLAUDE.md`, `plans/HANDOFF.md`, `docs/egui/START-HERE.md`, `docs/egui/rust/automation.md`,
  `plans/research/E2E-INSPECTOR.md` §1.5–1.6 and §2, and the cadatomic design you implement: `add_repo`
  `akiselev/cadatomic` (read), then its `docs/research/2026-10-24-waiting-and-synchronization.md` (the waits
  lane) or the tours-shell report `docs/notes/batch9-reports/tours-shell.md` G-shell-7 on branch
  `claude/b9-tours-shell` (the list-a11y lane).
- **Setup (cloud container, 4 cores, ~39 GB disk):** rustc 1.97.0; `corepack enable` and
  `pnpm install --frozen-lockfile`; every cargo command through `tools/dev/cargo-slot` with `BUILD_SLOTS=1
  BUILD_SLOT_JOBS=4 CHECK_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0`; `git config core.hooksPath .githooks` once.
- **Build little:** `cargo check -p <crate>` while writing; your own tests filtered at the end; **GitHub CI is
  the gate**. Wait with `tools/dev/ci-wait <sha> --jobs` (polls every 600 s; one waiter per sha).
- **Commit and push often** (session limits are near): after every item and at least every 30–45 minutes.
- **cadatomic is the consumer:** its batch 10 lane `waits-adopt` builds the domain events and sources on the
  waits lane's API after this merges. Keep APIs additive and list every public API in your record.
- **Security:** leave room only (`#[non_exhaustive]`, a principal id where cheap); build no policy.

| Lane | Branch | Record | Scope | Merge order |
|---|---|---|---|---|
| waits | `ci/b10-waits` | `plans/milestones/AU7-waits.md` | one wait engine: event journal with cursors, named busy sources (foreground/background), actionability, clocks, record/replay hooks | 1 |
| list-a11y | `ci/b10-list-a11y` | `plans/milestones/AX1-list-a11y.md` | scroll-area, `ListView`/virtual-list and context-menu content in the AccessKit tree and reachable by automation | 2 |

Expected conflicts: `crates/svelters-automation/src/session.rs` is waits'; list-a11y works in `svelters-egui`
(scroll and list widgets, menus) and touches automation only for its tests.
