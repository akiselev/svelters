# presto batch 2 (wave 2: widgets, the svelters backend, the app runner, tests, island, motion): common brief

Date: 2026-10-25. Coordinator: `session_01CqZ417NuvkRujZaYSm7Pwo`. **Everything in
`plans/cloud/presto-b1/BRIEF.md` applies** (where you work, read first, the minimal building and testing rule,
rules, attribution, no fake successes), with these differences:

- Base: branch `ci/presto-base` as it is when your session starts. It holds presto-base plus the wave-1 API
  skeletons the coordinator merged (presto-core, presto-text, presto-render, presto-frame, presto-anim,
  presto-compositor, presto-platform). Those crates are still being filled in by wave-1 lanes on their own
  branches: **code against their public APIs, not their internals**, and when the coordinator tells you a
  newer `ci/presto-base` exists, merge it into your branch.
- Your branch: `ci/presto-<lane>`; your record: `plans/milestones/PRESTO-<lane>.md`.
- If you need an API a wave-1 crate doesn't have yet, add the smallest thing you need **in your own crate**
  behind a module named `shim` with a comment naming the missing API, list it in your record, and the
  consolidation replaces it. Never edit a wave-1 crate on your branch.

Read `plans/research/presto/architecture.md` §4.2 (the `Kind` trait), §11 (input), §15 (island), §16 (testing),
and `plans/research/presto/notes/round2/svelters-inventory.md` (what each svelters-egui module is and the
port action for it). Source to port lives in `crates/svelters-egui/src/` (it stays, frozen, as the egui
backend: copy, don't move).
