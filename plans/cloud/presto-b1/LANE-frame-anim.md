# Lane FRAME-ANIM: presto-frame and presto-anim

Branch `ci/presto-frame-anim`. Owns `crates/presto-frame/**`, `crates/presto-anim/**`. Read: architecture
§10, §12; notes `round2/scheduler-latency.md` (state machine, estimator, simulator, constants),
`round2/animation-scroll.md` (Svelte's usage, the Web Animations subset, springs, scroll physics per OS).

presto-frame (the brief's API first, within 45 min):
1. `FrameClock` per window (compositor side) as in scheduler-latency §2: states, deadline modes (Immediate,
   Regular, Late, Blocked), two-deadline rule, urgent frames, idle unsubscription, hidden windows, VRR sustain
   (GPUI `InputRateTracker`: 100 ms window, 60 events/s → 1 s), back-pressure (one frame in flight).
2. The estimator (§3): p90 of 64-sample rings per cost series (ui split input/other, comp_cpu, gpu), 0.25 ms
   floor, KWin-style latch margin, cold-start seeds, outlier cap; `S_ui` formula.
3. `UiFrameState` (UI side): urgent-input rule (`now + est_ui_input ≤ D_ui(T)`), never start while the last
   commit is unapplied.
4. `sim`: the deterministic simulator (seeded): display (refresh, phase, jitter, VRR, compositor mode),
   present engine, GPU/UI cost models, input streams, timer error, visibility; fault cases; invariant checks
   after every event (§6.2); latency in frames.

presto-anim:
1. `easing` (cubic-bezier with Newton+bisection, steps, `linear()`), `interp` (numbers, lengths, colours,
   transform lists with CSS interpolation rules incl. matrix decomposition fallback), `css` value parser for
   the strings Svelte puts in keyframes.
2. `sample(&Generator, start, now)`: Web Animations timing (delay, end delay, iterations, direction, fill,
   iteration start), keyframe offsets with per-keyframe easing; `placement(property)`.
3. `spring`: closed-form damped harmonic (under/critical/over), `{duration, bounce}` ↔ physical, the Svelte
   60 Hz recurrence mapping (exact at 60 Hz), retarget keeping position and velocity.
4. `scroll`: per-OS wheel curves (Chromium smooth scroll: 200→100 ms, velocity-preserving retarget), flings
   (Chromium `PhysicsBasedFlingCurve`, exponential, Android spline), macOS rubber band, scroll latching
   helper, reduced-motion switch. Constants and sources in animation-scroll.md §4.

Tests (write, don't run): Svelte built-in transitions sampled at 60 Hz equal Svelte's JS formulas (write the
expected values from the formulas in svelte@5.57.2 `svelte/transition`, cited); spring mapping vs Svelte's
integrator at 60 Hz; timing edge cases from the Web Animations spec; simulator invariants over seeded runs.
