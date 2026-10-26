# SKELTER-frame-anim: skelter-frame and skelter-anim (skelter batch 1, lane FRAME-ANIM)

Branch `ci/presto-frame-anim` from `ci/presto-base`. Brief: `plans/cloud/presto-b1/LANE-frame-anim.md`.
Date: 2026-10-25.

## Landed

### skelter-frame (`crates/skelter-frame`)

- `clock`: `FrameClock` (compositor side, one per window), a pure state machine: `on_signal(now, Signal) ->
  SmallVec<[Action; 4]>`, `next_wakeup()`, `frame_args()`. Deadline modes Immediate / Regular / Late /
  Blocked; the two-deadline rule (the compositor draws its own damage at `D_comp` when the UI is late);
  urgent and idle paths (no open tick or nothing presented for two intervals: draw or start the UI at once
  for the next latchable target); unsubscribe after one quiet tick; hidden windows unsubscribe and keep
  damage, one immediate frame on show; output changes reset the phase; VRR keep-alive (`Represent`) for 1 s
  after high-rate input; back-pressure (one present in flight, no `BeginUiFrame` while a commit is
  unapplied); a draw that starts past its deadline (beyond the margin) retargets to the next reachable
  vblank; a frame shown before its target frees the vblanks up to it; feedback timeout (3 intervals);
  forced full redraw after 3 failed draws; device loss resets estimates.
- `estimate`: 64-sample rings, p90, 0.25 ms floor, outliers capped at 2 intervals; cold-start seeds (UI 2 ms,
  comp CPU 1 ms, GPU 2 ms, margin 1 ms); the KWin-style latch margin (miss: up by the overshoot, at least the
  floor; hit: down by a tenth of the slack; clamp `[0, min(3 ms, interval/2)]`); the margin is floored by the
  p95 of timer wake error (§4); spike tracking (8 provisional frames) on the UI series only; `S_ui`, `D_ui`,
  `D_comp` (`ε_commit` 50 µs); `est_total` clamped to 2 intervals; separate `ui_input` / `ui_other` series.
- `rate`: `InputRateTracker` (GPUI defaults: 100 ms window, 60 events/s, 1 s sustain).
- `ui`: `UiFrameState`: `mark_dirty` (a level), `on_urgent_input(now) -> RunNow | Wait` (`now +
  est_ui_input ≤ D_ui`, or the Mutter idle rule), `start_urgent`, `on_begin_frame` (refuses while a frame
  runs or a commit is unapplied), `commit_sent -> CommitReport`, `frame_skipped`, `commit_applied`.
- `sim`: the deterministic simulator (SplitMix64-seeded discrete-event loop): fixed and VRR displays with
  phase, jitter, latch and composed frames; present engine (first latch after GPU done, FIFO); shared GPU
  queue; lognormal cost models with spikes for UI, UI-input, compositor CPU and GPU; Poisson and scripted
  discrete input, continuous streams; timer slack and late wakeups; visibility; faults: slow UI frame,
  sustained slow UI, GPU spike, dropped feedback, discarded presents, no buffer release, output change,
  hide/show, device loss. Invariants 1, 2, 3, 4, 5, 7, 8 of `round2/scheduler-latency.md` §6.2 checked after
  every event; input latency in frames; `SKELTER_SEED`, `SKELTER_ITERATIONS`, `SKELTER_TRACE`.
- Tests: unit tests per module; `tests/sim.rs` (17 scenarios: seeded invariants, key latency in frames,
  idle = no wakeups, the fault table, two windows at 60 + 144 Hz on one GPU, VRR keep-alive, blocked mode,
  determinism).

### skelter-anim (`crates/skelter-anim`)

- `easing`: `cubic-bezier()` (Newton, bisection fallback, end-tangent extrapolation), its slope, `steps()`
  with the before flag, `linear()` with input canonicalisation; `easing::svelte`: all 31 `svelte/easing`
  functions in `f64`.
- `interp`: `lerp` (numbers, lengths, premultiplied sRGB colours, vectors, rects, transform lists),
  `compose`, CSS Transforms 2-D decomposition/recomposition and decomposed interpolation (short way round,
  flip-as-rotation), per-function interpolation for matching lists (identical matrices count as
  matching, so Svelte's `<computed matrix> translate() scale()` interpolates per function).
- `timing`: `sample(&Generator, start, now) -> Sample`: Web Animations phases, fill, iterations,
  `iteration_start`, directions, the effect easing with the before flag, missing keyframe offsets (NaN),
  per-keyframe easing; springs evaluate the closed form; `retarget` for springs; `placement(property)`.
- `spring`: closed form from the poles (distinct real, repeated, complex); `{duration, bounce}`, physical,
  ω₀/ζ, and Svelte's `{stiffness, damping}` through the 60 Hz recurrence (default → ω₀ 33.4, ζ 1.45);
  `svelte_velocity` makes the closed form equal Svelte's integrator at every 60 Hz frame; settle rule;
  `svelte_tick` (Svelte's integrator, for comparison).
- `scroll`: Chromium `ScrollOffsetAnimationCurve` (inverse-delta 200 → 100 ms, constant 150 ms,
  delta-based programmatic `(0.4, 0, 0, 1)` ≤ 1.5 s, velocity-keeping `UpdateTarget` with the 2.5×
  duration bound), `PhysicsBasedFlingCurve`, exponential (iOS) fling, Android `OverScroller` spline (table
  built as the Java initialiser does), rubber band (resistance curve, critically damped return ω₀ 10,
  5000 px/s cap), least-squares fling velocity over 150 ms (200 ms start timeout), per-gesture
  `ScrollLatch` (300 ms wheel latch), per-OS `defaults`, reduced motion. Constants read from the Chromium
  and AOSP sources at the research pins (fetched today; cited in the file header).
- `css`: `parse_declarations` (Svelte's `css_to_keyframe` rules), `camel_case`, `property`, `parse_value`
  (numbers incl. exponents, lengths with all CSS absolute/relative units, `#hex`, `rgb[a]()`, `hsl[a]()`,
  transform functions incl. 2-D `matrix3d`, filter functions) and `resolve` against `ResolveCx`.
- Tests: unit tests per module; `tests/svelte.rs`: fade, fly (plain and over a computed matrix), scale,
  slide, blur, draw built exactly as Svelte 5.57.2's `animate()` builds keyframes, sampled at 60 Hz against
  the formulas in `svelte/transition` (cited); Svelte springs against Svelte's integrator at 60 Hz; Web
  Animations timing edge cases.

## API notes against the brief

- `sample(generator, …)`: `gen` is a reserved keyword in edition 2024, so the parameter is `generator`.
- `Sample.value` is `Option<AnimValue>`: an effect with no keyframes (Svelte's delay animation) or outside
  its active interval without fill has no value. `Sample` also carries `progress` and `iteration`.
- `Signal` adds `UiInputCost`, `NeedsUiFrame(bool)`, `UrgentInput`, `BufferAvailable`, `UiFrameSkipped`,
  `Submitted`, `Discarded`, `DrawFailed`, `OutputChanged`, `DeviceLost`. `Action` adds `ApplyCommit`,
  `ForceFullRedraw`, `Represent`, `ClearTimer`. Contract for the compositor and platform lanes:
  - `Vsync { target }` is the next latchable vblank (the pacing callback after vblank n announces n+1);
  - signals from the UI thread arrive in the order sent (a commit before the dirt announced after it);
  - send `Submitted` after `queue.submit` + present, `GpuDone` from `on_submitted_work_done`,
    `BufferAvailable` on acquire success / waitable, `Presented` from platform feedback;
  - on `ApplyCommit`, apply the mailbox commit before the `Draw` that follows, and tell the UI
    (`UiFrameState::commit_applied`).
- `UiFrameState`: the caller calls `mark_dirty()` (send `NeedsUiFrame(true)` when it returns true), then
  `on_urgent_input(now)`; on `Wait` it also sends `UrgentInput`.
- `TransformFn` angles are radians throughout skelter-anim; `Keyframe.offset = NaN` means "missing".

## Not implemented (left out, not faked)

- skelter-anim: filter values (`css` parses them; `resolve` returns `None`, because skelter-base has no
  `AnimValue::Filter`); 3-D transforms (rejected); `add`/`accumulate` composition and the underlying value
  (a missing 0 or 1 keyframe holds the nearest keyframe); Svelte's `preserveMomentum`; scroll snapping
  (Chromium snap fling) and anchoring (core's); input-delay compensation in smooth-scroll retarget
  (`delayed_by`); named colours beyond black, white, red, lime, green, blue, yellow, gray/grey and
  transparent (computed styles serialise colours as `rgb()`).
- skelter-frame: invariant 6 (native-owned revisions: skelter-core's commit); earliest-deadline GPU ordering
  across windows and the `comp_contention` counter (the simulator's GPU is FIFO); `LatencyRecorder` and
  the §5.3 histograms; DRR boost and ProMotion frame-rate-range requests (only `Represent`); the
  occluded-but-present 250 ms throttle; the display-off fallback; golden timeline files; steady-rate
  animation pacing under VRR (VRR draws `Immediate`); the pipelined state is reported with hysteresis but
  does not change scheduling (the UI starts for the earliest reachable target, which pipelines by itself).

## What was run, what was not

- Run (to debug; they found real state-machine bugs): `cargo check -p skelter-frame -p skelter-anim
  --all-targets`; `cargo test -p skelter-frame` (also once with `SKELTER_ITERATIONS=150`); `cargo test -p
  skelter-anim`. All pass on Linux at the final commit.
- The Web Animations timing follows WA-1 §4.7 as published at drafts.csswg.org today (re-read for the
  after-phase active time).
- Not run: clippy, Windows and macOS, any other crate, the svelters workflows. GitHub CI runs them.

## Cross-lane edits

- `THIRD_PARTY_NOTICES.md`: new section "skelter: ported curves and constants" (Chromium BSD-3-Clause with
  its text, AOSP Apache-2.0, Svelte/eases MIT). No edits to `skelter-base`.

## Questions for the coordinator (each with a recommendation)

1. **`AnimValue::Filter`** (additive to skelter-base) so blur/brightness keyframes (Svelte `blur`) animate.
   Recommend: add `AnimValue::Filter(Vec<FilterFn>)` with `FilterFn` moved to skelter-base; skelter-anim
   interpolates per function (same list) else discrete. Not done because skelter-base is the contract.
2. **Document units in skelter-base::anim**: `TransformFn` angles in radians, `Keyframe.offset` NaN for a
   missing offset (or make it `Option<f32>`). Recommend: doc comments now, `Option` at consolidation.
3. **Apache-2.0 text**: the notices entry for the AOSP port links the license; RENDER ships the text as
   `crates/skelter-render/LICENSES/Apache-2.0-GPUI.txt`. Recommend: consolidation moves it to one
   repository-level `LICENSES/Apache-2.0.txt` that both entries cite.
4. **Defaults chosen from the research's open questions**: wheel latch 300 ms (no source); Linux wheel
   browser-like (120 px per detent, animated); a missed latch with positive slack raises the margin by the
   0.25 ms floor. Recommend: keep, revisit with the hardware rig.
5. **Platform and compositor contract** (above): `Vsync` target semantics, FIFO UI signals,
   `BufferAvailable`. Recommend: the coordinator forwards it to the COMPOSITOR and PLATFORM lanes.
