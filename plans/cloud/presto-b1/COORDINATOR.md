# presto build-out: coordinator record

Coordinator session: `session_01CqZ417NuvkRujZaYSm7Pwo` (started 2026-10-25). The user (2026-10-25): build the
full framework now, no M1 gate, GitHub CI for Windows/macOS, recommendations taken, minimal building and
testing until consolidation, "ready to go tonight".

## Wave 1 (batch presto-b1), launched 2026-10-25 18:23 UTC from `ci/presto-base` 6b13f4ae

| Lane | Branch | Session |
|---|---|---|
| CORE | `ci/presto-core` | `session_013K2YZSUHRdTwCmv2DuBCqE` |
| TEXT | `ci/presto-text` | `session_01MSpaSgqNxdmRY5R5WEUdho` |
| RENDER | `ci/presto-render` | `session_01YXr8ajAcdJzov6eqCHbEL2` |
| FRAME-ANIM | `ci/presto-frame-anim` | `session_01QXUoKfVdDsZJ9NiLeS5bWS` |
| COMPOSITOR | `ci/presto-compositor` | `session_01FvoComfRkHRa6Z2sPhbyz7` |
| PLATFORM | `ci/presto-platform` | `session_01UJ89x3UhYpz8PAmkjLFg9G` |
| COMPILER | `ci/presto-compiler` | `session_014u9BiDM8fXY3HswkrJESuw` |
| RUNTIME | `ci/presto-runtime` | `session_0165tCmCPXxjyHJbM8gXBxXG` |

Skeleton merges: the coordinator merges each lane's first compiling API push into `ci/presto-base` and tells
the dependent lanes (COMPOSITOR needs RENDER/FRAME-ANIM; wave 2 needs CORE/TEXT/PLATFORM/COMPOSITOR).

## Waves 2 and 3

`plans/cloud/presto-b2/` (wave 2: widgets, svelters-presto, the app runner and test harness, the egui island,
native motion and CSS) launches when CORE's and TEXT's skeletons are in `ci/presto-base`. Wave 3 (cadatomic
adoption, presto_berth, presto_quire, OS layers and canvases, the wgpu fork, own platform backends) follows
the first consolidation.

## Consolidation

One consolidation per wave: merge lane branches into `ci/presto-integration` in the order base-crates → core
→ text → render → frame-anim → compositor → platform → compiler → runtime; fix build breaks; run the presto
and svelters CI; record failures in `plans/cloud/presto-b1/FIX-QUEUE.md`; the coordinator merges to master
when the five svelters workflows plus `presto` are green.
