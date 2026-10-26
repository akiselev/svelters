# presto build-out, consolidation 2: the skelter rename and the master merges

You are the second consolidation. Read `CLAUDE.md`, `plans/cloud/presto-b1/COORDINATOR.md` (lanes, decisions, the
"For the consolidation" list), `plans/cloud/presto-b1/FIX-QUEUE.md`, then this file. The coordinator is
`session_01CqZ417NuvkRujZaYSm7Pwo`. Report to it with `send_message`. Its decisions carry delegated authority; never ask
the user.

## Start

Branch `ci/skelter-integration`, made from `origin/ci/presto-integration` at consolidation 1's final head (the coordinator
names it). Merge every lane head that moved since then; the coordinator lists them.

## 1. The rename (user, 2026-10-25: "go with svelters-skelter")

presto becomes **skelter**, in one commit, mechanically:
- **Crates and packages:** `crates/presto` → `crates/skelter` (package `skelter`); `crates/presto-*` → `crates/skelter-*`;
  `crates/svelters-presto` → `crates/svelters-skelter`. Update the workspace members and dependency keys.
- **Identifiers:** `presto_*` → `skelter_*` in Rust paths, `Presto*` type names → `Skelter*`, `PRESTO_*` env vars →
  `SKELTER_*` (including `PRESTO_BACKEND`, `PRESTO_GPU_TESTS`), and the `presto.json` spec → `skelter.json`. Regenerate
  generated files with their generators.
- **CI:** `.github/workflows/presto.yml` → `skelter.yml` (job and name `skelter`), plus the paths-ignore lists in the
  five svelters workflows.
- **Docs and records:** `plans/research/presto/` → `plans/research/skelter/`, `plans/milestones/PRESTO-*` →
  `SKELTER-*`, and the text in them. Keep the batch folders' names (`plans/cloud/presto-b*`) as history; add a line
  at the top of each BRIEF saying presto is now skelter.
- **Every user-facing string** says skelter.
- **berth:** on its `claude/presto-berth` branch, `presto_berth` → `skelter_berth`, with its svelters git dependency on
  your branch.
- **quire:** on its `claude/presto-quire` branch, `presto_quire` → `skelter_quire`, likewise.
- **Verify:** `rg -i presto` over the three repositories must find only history (batch folder names, commit messages
  and that one line per BRIEF). `cargo check --workspace --all-targets`, `pnpm -r build` and the renamed crates' tests,
  run once.

## 2. Merges to master

- **svelters:** merge `ci/skelter-integration` into `master` (`--no-ff`) only when all six workflows (js,
  linux-gtk, linux-qt, macos-appkit, windows-winui, skelter) are green on its head. An intermittent macos-appkit
  `counter-mac backend` failure counts as red until its cause is in the FIX-QUEUE and the coordinator has decided.
  Update `STATUS.md`'s handoff block and `MATRIX.md` (a skelter column).
- **berth:** first point `skelter_berth`'s svelters dependency at svelters `master`, then run berth's `check.sh` and the
  skelter_berth tests (never run so far), then merge `claude/presto-berth` into berth `master`.
- **quire:** the same, with quire's tests (presto_quire's 43 were never run).
- **cadatomic is not merged here.** Batch 12 starts from cadatomic `claude/presto-adoption`, renamed in its base commit
  by the batch coordinator.

## 3. Record, don't fix

You fix only build breaks and the rename. Everything else goes into `FIX-QUEUE.md`, and the coordinator routes it to
batch 12's svelters lane `skelter-fixes`.

## Report

Send the coordinator:
- the svelters, berth and quire master shas;
- the CI run ids;
- the rename's leftover `rg` hits;
- the local test counts;
- the FIX-QUEUE delta.

Then stop.
