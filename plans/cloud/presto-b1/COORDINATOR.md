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

## Wave 2 (presto-b2) and early wave 3 (presto-b3), launched 2026-10-25 18:27 UTC from `ci/presto-base` efe20dd5

The user (2026-10-25): "You can run more than 8 in parallel". Wave 2 starts at once, coding against the b1
brief's signatures until the wave-1 skeletons are merged into `ci/presto-base`.

| Lane | Branch | Session |
|---|---|---|
| WIDGETS-BASIC | `ci/presto-widgets-basic` | `session_012eauTkCP7to4arcKCPJvRq` |
| WIDGETS-VIEWS | `ci/presto-widgets-views` | `session_01UQTt4dFy9hz4VEv6q2vmpg` |
| TEXT-INPUT | `ci/presto-text-input` | `session_017NL3B2S8QKfkboTAfbVntW` |
| WIDGETS-TIER1 | `ci/presto-widgets-tier1` | `session_01DjBCnf7oMK3XEzkghWoiQs` |
| SVELTERS-PRESTO | `ci/presto-svelters-presto` | `session_01497YqbhMLBiY7YWGtiLKAX` |
| APP-TEST | `ci/presto-app-test` | `session_012jLSJr3WKZ7XDr4KKJtnf1` |
| EGUI-ISLAND | `ci/presto-egui-island` | `session_01YP64x1tuvgBxa9JAtpuUpj` |
| MOTION-CSS | `ci/presto-motion-css` | `session_011YTnnfgQ149nNJtSdVuDWS` |
| WIN32 (b3) | `ci/presto-win32` | `session_01AJfMrChD4ZXtP37EMUYQ8Y` |
| WAYLAND (b3) | `ci/presto-wayland` | `session_01JDnE12Ws2QHwSfPYzesDaR` |
| MACOS (b3) | `ci/presto-macos` | `session_01TjQo3Ks6gxG7GdDLdgUZ86` |

## Later lanes

| Lane | Branch | Session | Note |
|---|---|---|---|
| SVELTERS-PRESTO (2) | `ci/presto-svelters-presto` | `session_012kp44HgxNR751yz6GvNAH5` | continues the first session's hand-off (port to the real Document) |
| CORE-2 | `ci/presto-core-2` | `session_01QDYxkmxLL2rFtDDfSbu4ow` | style table, DESC_NONPASSIVE, CORE's not-done list (CORE's a07c3189 wins on overlap) |
| BERTH | berth `claude/presto-berth` | `session_01FfrTeoE8knLYGyFeFf9Ss3` | presto_berth |
| QUIRE | quire `claude/presto-quire` | `session_01VFabqgKPiq5AmCkua2ntGG` | presto_quire |
| CORE-3 | `ci/presto-core-3` | `session_012X4RdtAUrgfNuRfD1Vh4ge` | WIDGETS-BASIC core requests (a)–(j) plus the shared drag/anchor members |

Merged into `ci/presto-base` so far: platform skeleton, frame-anim, compositor skeleton, core (full batch-1 scope and
the request work a07c3189), text, render (full API, then GPU references 90959a28), the widgets substrate (WIDGETS-BASIC d46dcd96),
CORE-2 (8d45666c), FRAME-ANIM test fixes (1ea8983c), platform try_create_window (5aab7622), CI fixes 14981893 and 2a12e530.
Done lanes: CORE, CORE-2, PLATFORM, RENDER, RUNTIME (4c65de8d, all five svelters workflows green; merged at consolidation).
Decisions: SVELTERS-PRESTO Q1–Q4 and CORE Q1–Q3 as in their records (coordinator, 2026-10-25).

## Waves 2 and 3 (original plan)

`plans/cloud/presto-b2/` (wave 2: widgets, svelters-presto, the app runner and test harness, the egui island,
native motion and CSS) launches when CORE's and TEXT's skeletons are in `ci/presto-base`. Wave 3 (cadatomic
adoption, presto_berth, presto_quire, OS layers and canvases, the wgpu fork, own platform backends) follows
the first consolidation.

## Consolidation

One consolidation per wave: merge lane branches into `ci/presto-integration` in the order base-crates → core
→ text → render → frame-anim → compositor → platform → compiler → runtime; fix build breaks; run the presto
and svelters CI; record failures in `plans/cloud/presto-b1/FIX-QUEUE.md`; the coordinator merges to master
when the five svelters workflows plus `presto` are green.
