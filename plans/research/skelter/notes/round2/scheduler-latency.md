# Frame scheduler, render-time estimation, clocks and latency instrumentation for tessel

Research notes for the retained GUI toolkit ("tessel") in
`cadatomic/docs/research/2026-10-25-own-gui-toolkit.md` (§2 budgets, §4 threads, §5 frame, §8 platform).
Written 2026-10-25 by a research subagent. Web and source reading only; nothing was built or measured.

**Grades.**
- **[src]**: read in the source file named, at the sha given.
- **[doc]**: stated by the vendor's documentation, protocol XML or the author's own post.
- **[src2]**: secondary source (press, forum, third-party write-up). Treat it as a lead.
- **[inf]**: my inference or design proposal. No source states it.
- **[mem]**: background knowledge I could not re-fetch today.

**Pins (read 2026-10-25).** The `ls-remote` sha was taken at fetch time. Files were fetched by sha where the host
allowed it (Chromium, gamesdk), otherwise from the branch head in the same minute.

| Source | Ref | Sha |
|---|---|---|
| Chromium `chromium/src` | `main` | `521441860f5056571081713ee25da002549bc63a` |
| KWin `plasma/kwin` | `master` | `f41c984c8109ac4785daa817c61eea18da791a44` |
| Mutter `GNOME/mutter` | `main` | `2b9c0787f99dacef16ba987656e28ffcb344b9a2` |
| Android Game SDK (Swappy) `platform/frameworks/opt/gamesdk` | `main` | `044fd03c4a7d3b75aeb6ca2bd7fb6155d2cdb787` |
| Zed `zed-industries/zed` | `main` | `a5c4ca8bf6c4717a328a4a61b42d6d1d38949bcc` (2026-10-25 08:55 UTC) |
| PresentMon `GameTechDev/PresentMon` | `main` | `9db97248df5322a0aa1581d9141d1de6b058085b` |
| wayland-protocols | `main` | `b9edf66597586c914b5b1ff3d964d7ed201adab9` |
| Vulkan-Docs | `main` | `a8b088a3fae12c8575407ea90c7c09bd653db923` |
| wgpu / wgpu-types | docs.rs | 30.0.1 |
| Tracy | latest tag | v0.14.1 (manual read from `master`) |

URL shorthands used below:
- `CR/<path>` = `https://chromium.googlesource.com/chromium/src/+/521441860f5056571081713ee25da002549bc63a/<path>`
- `KW/<path>` = `https://invent.kde.org/plasma/kwin/-/blob/f41c984c8109ac4785daa817c61eea18da791a44/<path>`
- `MU/<path>` = `https://gitlab.gnome.org/GNOME/mutter/-/blob/2b9c0787f99dacef16ba987656e28ffcb344b9a2/<path>`
- `SW/<path>` = `https://android.googlesource.com/platform/frameworks/opt/gamesdk/+/044fd03c4a7d3b75aeb6ca2bd7fb6155d2cdb787/<path>`
- `ZED/<path>` = `https://github.com/zed-industries/zed/blob/a5c4ca8bf6c4717a328a4a61b42d6d1d38949bcc/<path>`

Companion notes in `cadatomic/docs/research/notes/2026-10-25-own-gui-toolkit/` already cover the following, so
this file cites them rather than repeating them:
- `browsers-flutter-latency.md`: the cc pipeline, EventLatency stages, Levien's pacing posts and the DXGI waitable.
- `gtk-qt-wayland.md`: GdkFrameClock, wp_presentation, fifo/commit-timing, KWin/Mutter summaries.
- `windows.md`: DXGI flip model, wgpu-hal DX12/Vulkan swapchains, the compositor clock.
- `apple-android.md`: CADisplayLink/CAMetalDisplayLink, Choreographer, input batching.

---

## 0. Summary

1. **Two state machines, one clock.** The compositor thread owns one `FrameClock` per window. Each clock is a small
   Chromium-style state machine (`Idle → Armed → InBeginFrame → InDeadline → InFlight`) with deadline modes
   `Immediate | Regular | Late | Blocked`. The UI thread runs a smaller one (`Idle → Scheduled → Running →
   Committed`) and is started by `BeginUiFrame` messages or, for urgent discrete input, by itself.
2. **Two deadlines.** The UI must commit by `D_ui`. The compositor draws at `D_comp`, with or without that commit.
3. **One frame in flight.** There is at most one present not yet acknowledged and at most one unapplied commit. The UI
   never starts a frame while its last commit is unapplied (Chromium `kMaxPendingSubmitFrames = 1`; KWin's
   `maxPendingFrameCount` is 1 under VRR).
4. **Just-in-time start.** The UI start time is
   `S_ui = T_present − L_latch − est_gpu − est_comp_cpu − est_ui`. `T_present` comes from presentation feedback.
   Each estimate is the p90 of the last 64 samples (Chromium uses p90 over 60). The latch margin `L_latch` adapts
   with KWin's fast-attack, slow-decay rule (+δ at once on a miss, −slack/10 on a hit, clamped to
   `[0, min(3 ms, interval/2)]`).
5. **Urgent and idle.** After an idle period, start at once (Mutter `should_update_now`: "lowest average latency for
   sporadic user input"). When nothing is dirty, unsubscribe from the clock. Keep presenting for 1 s after
   high-rate input (GPUI `InputRateTracker`: ≥ 60 events/s over 100 ms, sustained 1 s).
6. **Clocks.**
   - Windows: the DXGI waitable plus `GetFrameStatistics`; `DCompositionWaitForCompositorClock` on Windows 11
     (build 22000); QPC timebase.
   - macOS: `CAMetalDisplayLink.targetTimestamp` (macOS 14) plus `MTLDrawable.presentedTime`;
     `mach_absolute_time` timebase.
   - Wayland: `wl_surface.frame` plus `wp_presentation`, and commit-timing; `CLOCK_MONOTONIC`.
   - Fallback everywhere: a phase-locked synthetic timer.
7. **Latency records.** Every input gets an OS timestamp and an `InputSeq`. The seq rides through
   dispatch → JS → transaction → commit → draw → submit → present → feedback, Chrome EventLatency-style. Per-stage
   histograms go in-app; traces go to Perfetto/Chrome JSON and Tracy; on Windows we emit Intel-PresentMon ETW
   events.
8. **Tests.** The scheduler core is a pure state machine (`on_event`, `next_action`). It is tested under a seeded
   virtual clock with a simulated display, GPU and presentation feedback, plus fault injection, in Zed's
   `TestScheduler` style. CI asserts latency *in frames*. Real milliseconds come from a Teensy+photodiode rig run by
   hand per release and per OS.

---

## 1. Prior art, read in source

### 1.1 Chromium cc: `SchedulerStateMachine` and `Scheduler`

Files: `CR/cc/scheduler/scheduler_state_machine.{h,cc}`, `CR/cc/scheduler/scheduler.cc`,
`CR/cc/scheduler/scheduler_settings.h`, `CR/cc/metrics/compositor_timing_history.cc`,
`CR/components/viz/common/frame_sinks/begin_frame_args.h`, `CR/components/viz/common/frame_sinks/begin_frame_source.cc`,
`CR/components/viz/service/display/display_scheduler.cc`.

**Design principle [src, header comment].**
- "The state machine tracks internal state but is also influenced by external state. Internal state includes
  things like whether a frame has been requested, while external state includes things like the current time being
  near to the vblank time."
- "The scheduler seperates 'what to do next' from the updating of its internal state to make testing cleaner."
  `NextAction()` is `const`; `WillX()`/`DidX()` mutate.

**States [src, `scheduler_state_machine.h`].**
- `BeginImplFrameState { IDLE, INSIDE_BEGIN_FRAME, INSIDE_DEADLINE }`. The header notes it "does not cycle through
  these states in a fixed order on all platforms".
- `BeginMainFrameState { IDLE /*A new BeginMainFrame can start*/, SENT, READY_TO_COMMIT }`.
- `LayerTreeFrameSinkState { NONE, ACTIVE, CREATING, WAITING_FOR_FIRST_COMMIT, WAITING_FOR_FIRST_ACTIVATION }`.
- `ForcedRedrawOnTimeoutState { IDLE, WAITING_FOR_COMMIT, WAITING_FOR_ACTIVATION, WAITING_FOR_DRAW }`, with
  `maximum_number_of_failed_draws_before_draw_is_forced = 3` (`scheduler_settings.h`).
- `BeginImplFrameDeadlineMode` (header comments quoted):

  | Mode | Value | Header comment | Deadline time (`Scheduler::ScheduleBeginImplFrameDeadline`) |
  |---|---|---|---|
  | `NONE` | 0 | "No deadline should be scheduled e.g. for synchronous compositor." | none |
  | `IMMEDIATE` | 1 | "Deadline should be scheduled to run immediately." | `base::TimeTicks()` (now) |
  | `WAIT_FOR_SCROLL` | 2 | "Deadline should be delayed as we are expecting input for a scroll." | `frame_time + interval × scroll_deadline_ratio` (0.333; `scroll_deadline_mode_enabled = false` by default) |
  | `REGULAR` | 3 | "…run at the deadline provided by in the BeginFrameArgs." | `args.deadline` (already adjusted, below) |
  | `LATE` | 4 | "…run when the next frame is expected to arrive." | `frame_time + interval`; sends an early `DidNotProduceFrame(kNoDamage)` if `!NewActiveTreeLikely()` |
  | `BLOCKED` | 5 | "…blocked indefinitely until the next frame arrives." | none (full-pipeline / headless) |

**Actions [src].** `Action { NONE, SEND_BEGIN_MAIN_FRAME, COMMIT, POST_COMMIT, ACTIVATE_SYNC_TREE,
PERFORM_IMPL_SIDE_INVALIDATION, DRAW_IF_POSSIBLE, DRAW_FORCED, DRAW_ABORT, BEGIN_LAYER_TREE_FRAME_SINK_CREATION,
PREPARE_TILES, INVALIDATE_LAYER_TREE_FRAME_SINK }`. `NextAction()` tests them in this priority order:
send BMF → post-commit → activate → commit → draw → impl-side invalidation → prepare tiles → invalidate sink →
create sink.

**Mode selection [src, `CurrentBeginImplFrameDeadlineMode`].**
- Outside a begin frame: `NONE`.
- Headless full-pipeline waiting: `BLOCKED`.
- `ShouldTriggerBeginImplFrameDeadlineImmediately()`: `IMMEDIATE` (or `WAIT_FOR_SCROLL`).
- `needs_redraw_` ("We have an animation or fast input path on the impl thread that wants to draw, so don't wait
  too long for a new active tree"): `REGULAR`.
- Otherwise ("just waiting for a new active tree"): `LATE`.

`ShouldTriggerBeginImplFrameDeadlineImmediately` [src]:
- false if `IsDrawThrottled()`;
- true if `active_tree_needs_first_draw_`;
- false if `!needs_redraw_`;
- true if no BMF was sent this frame and main is not busy ("TriggerDeadlineDueToThrottling");
- true if `!wait_for_main` (no commit pending and no pending tree);
- true if `ImplLatencyTakesPriority()` (tree priority `SMOOTHNESS_TAKES_PRIORITY`).

**Rules that matter for us [src].**
- *One draw per deadline.* `ShouldDraw`: "Do not draw more than once in the deadline", "Do not queue too many
  draws" (`IsDrawThrottled`), "do not draw outside of the BeginImplFrame deadline".
- *One BMF per begin frame.* `ShouldSendBeginMainFrame`: "Do not send more than one begin main frame in a begin
  frame", unless `urgent_begin_main_frame_pending_`. At this sha, the only caller of `SetUrgentBeginMainFramePending`
  is the experimental `Scheduler::SendEarlyFinalBeginMainFrame` (view transitions, crbug.com/496610055). It is not a
  general input path.
- *No BMF while idle.* "We should not send BeginMainFrame while we are in the idle state since we might have new
  user input arriving soon" (`ShouldBlockBeginMainFrameWhenIdle`). The BMF is sent from inside the BeginImplFrame,
  not between frames.
- *Back-pressure.* `constexpr int kMaxPendingSubmitFrames = 1;` and
  `IsDrawThrottled() = pending_submit_frames_ >= kMaxPendingSubmitFrames && !disable_frame_rate_limit` (bypassed by
  feature `kNoCompositorFrameAcks`). BMFs are also throttled on the ack, "unless we just submitted a frame to
  potentially improve impl-thread latency over main-thread throughput".
- *Visibility.* `ShouldSubscribeToBeginFrames()` returns false if `!visible_` ("If we are not visible, we don't
  need BeginFrame messages"). `SetVisible(true)` clears `main_thread_missed_last_deadline_`.
- *Proactive subscription.* `ProactiveBeginFrameWanted()` keeps BeginFrames flowing while a commit is pending, a
  pending tree exists, prepare-tiles is needed, or `did_attempt_draw_in_last_frame_`. This "helps avoid negative
  glitches in our SetNeedsBeginFrame requests". It is a one-frame hysteresis before going idle.
- *Missed-deadline bookkeeping.* At `OnBeginImplFrameIdle`:
  `main_thread_missed_last_deadline_ = CommitPending() || has_pending_tree_ || active_tree_needs_first_draw_`.
  The header says "If the main thread didn't manage to produce a new frame in time for the impl thread to draw, it
  is in a high latency mode."
  - At this sha `Scheduler` reads the flag only for tracing (`BeginImplFrameWithDeadline`).
  - The older explicit "skip a BMF to recover latency" logic, still described in `docs/how_cc_works.md`, is no
    longer in `scheduler.cc`.
  - Recovery now has two parts. MISSED BeginFrames whose deadline has passed are dropped
    (`FrameSkippedReason::kRecoverLatency`, "Discard missed begin frames if they are too late"). Queued BeginFrames
    coalesce to the last one ("we only want to process the last one").
- *Critical-path test.* `bmf_to_activate_threshold = interval − DrawDurationEstimate() − kDeadlineFudgeFactor`.
  `SetCriticalBeginMainFrameToActivateIsFast(BeginMainFrameQueueToActivateCriticalEstimate() < threshold)` predicts
  whether main can make this frame.

**Deadline arithmetic [src].**
- `scheduler.cc`: `const base::TimeDelta kDeadlineFudgeFactor = base::Microseconds(1000);`, then
  `adjusted_args.deadline -= DrawDurationEstimate(); adjusted_args.deadline -= kDeadlineFudgeFactor;`.
- The Viz display reserves the last third of the frame for itself (`begin_frame_args.h`):
  `kDefaultEstimatedDisplayDrawTimeRatio = 1.f / 3`. "Using 1/3 of the vsync as the default adjustment gives the
  display compositor the last 1/3 of a frame to produce output, the client impl thread the middle 1/3 … and the
  client's main thread the first 1/3." `DisplayScheduler::OnBeginFrame` does
  `current_begin_frame_args_.deadline -= DefaultEstimatedDisplayDrawTime(args.interval)`.
- The display has the same deadline modes (`kImmediate` → now, `kRegular` → `args.deadline`, `kLate` →
  `frame_time + interval`, `kNone` → `TimeTicks::Max()`) in `DisplayScheduler::DesiredBeginFrameDeadlineTime`.
- Android frame timelines: `DisplayScheduler` "Arbitrarily use[s] 75% of the deadline for CPU work"
  (`latch_delta * 3 / 4`).
- `DelayBasedBeginFrameSource::CreateBeginFrameArgs`:
  `deadline = time_source_->NextTickTime() − Interval() + interval` (one interval after the tick, or `max_vrr_interval_`
  under VRR). `kDoubleTickDivisor = 2.0` stops a synthetic source sending a MISSED frame less than half an interval
  after the last.

**Estimator [src, `compositor_timing_history.cc`].**
- `kDurationHistorySize = 60`.
- Every estimate is a percentile of a rolling history: `kDrawEstimationPercentile = 90.0`, and likewise 90.0 for
  BMF-queue (critical and not-critical), BMF-start-to-ready-to-commit, commit, commit-to-ready-to-activate and
  activate.
- So `DrawDurationEstimate() = draw_duration_history_.Percentile(90)` over the last 60 draws.

**Tests [src, `CR/cc/scheduler/scheduler_unittest.cc`, `CR/cc/test/scheduler_test_common.h`].**
- Tests run on `base::TestMockTimeTaskRunner`, with `AdvanceMockTickClock(viz::BeginFrameArgs::DefaultInterval())`
  per `SendNextBeginFrame()`.
- `FakeCompositorTimingHistory::SetAllEstimatesTo(kFastDuration /*1 ms*/)` and per-estimate setters inject slow paths
  (`kSlowDuration = 1 s`).
- Test names show the scenarios they pin, for example `MainFrameNotSkippedAfterLateBeginFrame`,
  `ImplFrameNotSkippedAfterLateAck_DrawEstimateTooLong`, `SyntheticBeginFrames_IsDrawThrottled`,
  `BeginFrameAckForLateMissedBeginFrame`, `CriticalBeginMainFrameToActivateIsFast`,
  `SendEarlyDidNotProduceFrameIfIdle`, `ImplLatencyTakesPriority`. This is the model for §6.

### 1.2 KWin `RenderLoop` and `RenderJournal`

Files: `KW/src/core/renderjournal.cpp`, `KW/src/core/renderloop.cpp`, `KW/src/core/renderloop_p.h`,
`KW/src/backends/drm/drm_commit_thread.cpp`.

**RenderJournal [src, quoted].**
```cpp
const auto timeDifference = m_lastAdd ? presentationTimestamp - *m_lastAdd : 10s;
static constexpr std::chrono::nanoseconds varianceTimeConstant = 6s;
const double varianceRatio = std::clamp(timeDifference.count() / double(varianceTimeConstant.count()), 0.001, 0.1);
const auto renderTimeDiff = std::max(renderTime - m_result, 0ns);
m_variance = std::max(mix(renderTimeDiff, m_variance, varianceRatio), renderTimeDiff);
static constexpr std::chrono::nanoseconds timeConstant = 500ms;
const double ratio = std::clamp(timeDifference.count() / double(timeConstant.count()), 0.01, 1.0);
m_result = mix(renderTime, m_result, ratio);
...
std::chrono::nanoseconds RenderJournal::result() const { return m_result + m_variance * 2; }
```
- The mean is an EWMA with a time constant of 500 ms (at 120 Hz, ratio ≈ 8.3/500 ≈ 0.017 per sample).
- "Variance" is really a peak tracker of positive overshoot. It jumps at once to any sample above the mean and decays
  with a time constant of 6 s.
- The estimate is mean + 2 × that peak [inf from the code].
- The sample is the measured render time from the scheduled wakeup to render end. In `notifyFrameCompleted` it is
  floored: "timings are pretty unpredictable in the sub-millisecond range; this minimum ensures that when CPU or GPU
  power states change, we don't drop any frames": `minimumTime = 2 ms`.

**Scheduling [src, `RenderLoopPrivate::scheduleRepaint`].**
- `expectedCompositingTime = std::min(renderJournal.result() + safetyMargin, 2 * vblankInterval)`.
- "if it's been a while since the last frame, the GPU is likely in a low power state and render time will be
  increased": if `pageflipsSince > 100`, then `expectedCompositingTime = max(vblankInterval − 1us, expected)`.
- Triple buffering by `pageflipsInAdvance = min(expected / vblankInterval + 1, maxPendingFrameCount)`, with
  hysteresis. "switching from double to triple buffering causes a frame drop": switch up at once; switch down only
  after `doubleBufferingCounter >= 10` good frames, and never while `expected >= 0.95 × vblankInterval`.
- `nextRenderTimestamp = max(nextPresentationTimestamp − expectedCompositingTime, now)`, started on a `PreciseTimer`.
- VRR: `targetTimestamp = max(presentNotBefore, lastPresentationTimestamp + vblankInterval)`. Tearing: present ASAP.
  Under VRR or tearing, `effectiveMaxPendingFrameCount = 1`. `maxPendingFrameCount` defaults to 1 in
  `renderloop_p.h`.
- When a non-driving item repaints under VRR it is delayed by `s_delayVrrTimer = 1'000ms / 30`.

**DRM safety margin [src, `drm_commit_thread.cpp`].**
- `s_safetyMarginMinimum{ env KWIN_DRM_OVERRIDE_SAFETY_MARGIN or 1000 }` µs; `m_baseSafetyMargin = vblankTime +
  s_safetyMarginMinimum`.
- After each commit: "the commit was done later than desired, immediately add the required difference"
  (`m_additionalSafetyMargin -= safetyDifference`, where the difference is negative). Otherwise "we want to keep
  latency at a minimum, so slowly reduce the safety margin" (`m_additionalSafetyMargin -= safetyDifference / 10`).
- `maximumReasonableMargin = min(3ms, m_minVblankInterval / 2)`, and the margin is clamped to `[0, max]`.

**Field measurement: farnoy.dev, "linux-latency", Jakub Okoński (Phoronix 2026-06-10) [src2].**
https://farnoy.dev/posts/linux-latency , https://www.phoronix.com/news/KDE-KWin-Gaming-Latency-2026
- The rig is a Teensy as a USB HID mouse plus a light sensor, running the Open-Source-LDAT sketch
  (https://github.com/S4N-T0S/Open-Source-LDAT), and logs hundreds of samples to CSV.
- He measured a predicted render time p50 of **11.35 ms** against an actual **2.06 ms**: "KWin consistently
  overestimates how long compositing is going to take and it goes directly toward added latency".
- Qt timers "round every duration up to the next millisecond".
- The 2 ms floor "is high and not scalable to 540 Hz".
- A client presenting every frame shrinks other clients' window: "they often miss the upcoming pageflip".
- His patches: a timerfd timer (p99 wakeup deviation 51 µs), a smaller margin, and a p95 over a 512-frame ring in
  place of the 2 ms floor. They gained about 1.1–1.2 ms at the minimums.
- **Lessons for us [inf]:**
  1. A peak tracker with a 6 s decay plus a 2 ms floor badly over-pads a fast renderer.
  2. Use a windowed percentile and a small floor.
  3. Use a sub-millisecond timer.

### 1.3 Mutter `ClutterFrameClock`

File: `MU/clutter/clutter/clutter-frame-clock.c`; constant `clutter_max_render_time_constant_us = 1000` in
`MU/clutter/clutter/clutter-main.c`.

**States [src].** `INIT, IDLE, SCHEDULED, SCHEDULED_NOW, SCHEDULED_LATER, DISPATCHED_ONE,
DISPATCHED_ONE_AND_SCHEDULED, DISPATCHED_ONE_AND_SCHEDULED_NOW, DISPATCHED_ONE_AND_SCHEDULED_LATER, DISPATCHED_TWO`.
- `schedule_update_now` moves `IDLE/SCHEDULED/SCHEDULED_LATER → SCHEDULED_NOW`.
- It moves `DISPATCHED_ONE → DISPATCHED_ONE_AND_SCHEDULED_NOW` only if `want_triple_buffering`; otherwise it sets
  `pending_reschedule_now`.
- `DISPATCHED_TWO` is the cap ("quad buffering would be a bug").
- Modes are `FIXED`, `VARIABLE` (VRR) and `PASSIVE`. Constants: `SYNC_DELAY_FALLBACK_FRACTION 0.875f`,
  `MINIMUM_REFRESH_RATE 30.f`.

**Estimate [src, `clutter_frame_clock_estimate_max_update_time_us`].**
```
max_update_time = max_update_duration_us + deadline_evasion_us + vblank_duration_us
                + (clutter_max_render_time_constant_us /*1000*/ + max_update_margin.current_us)
clamped to [0, 2 × refresh_interval]   (3 × when triple buffering is wanted)
```
- The comment lists the parts: "Dispatch start lateness", "dispatch start to buffer swap", "maximum of duration from
  buffer swap to GPU rendering finish and … to buffer submission to KMS", "duration of vertical blank", and "A margin".
- The sample is `update_duration_us = MIN(MAX(to_kms_ready_us, to_flip_us), 3 × refresh_interval)`.
- `adjust_update_duration_estimates` tracks the maximum asymmetrically:

  | Case | Change |
  |---|---|
  | `delta < 0` | `max += delta / (refresh_rate × 4)` (slow down, "at least seconds") |
  | `0 ≤ delta < margin` | `max += delta / refresh_rate` |
  | `delta ≥ margin` | jump by `delta` |

- The min is tracked too: up by `delta / (2·rate)`, down by `delta / rate`.
- Dynamic margin: `target = CLAMP(delta + dispatch_lateness − 1000, prev_target, refresh/4)`.
  - It is raised halfway when the delta is under 1 ms, and jumps otherwise.
  - It is lowered by `delta/60` only if it was last set over 1 s ago ("at least a minute for the margin to drop").

**Idle → immediate [src, `should_update_now`].** No update now if a frame is dispatched but not presented, or if the
last presentation had no `VSYNC` flag, or if the next presentation equals the "smooth" next one. Otherwise: "There was
an idle period since the last presentation, so there seems be no constantly updating actor. In this case it's best
to start working on the next update ASAP, this results in lowest average latency for sporadic user input."

**Fallbacks [src, `calculate_next_update_time_us`].**
- With no estimate or no VSYNC-flagged presentation, dispatch at `last_dispatch − lateness + refresh_interval`, or
  now. This is the timer-paced cold start.
- When `next_presentation − max_estimate < now`, try the *min* estimate (can we still make it if this frame is
  cheap?) before skipping to the next interval boundary.
- VRR (`calculate_next_variable_update_time_us`): `next_presentation = last_presentation + refresh_interval`,
  `update = next_presentation − max_estimate`. If that is already past, `next_presentation = now + max_estimate` and
  the update runs now.
- Timer: a `timerfd` with `TFD_TIMER_ABSTIME`.

### 1.4 Android: Choreographer frame timelines and Swappy

- **AChoreographer (NDK) [doc]** https://developer.android.com/ndk/reference/group/choreographer. All API 33:
  - `AChoreographer_postVsyncCallback`: one-shot; "to render continuously the callback should itself call
    AChoreographer_postVsyncCallback".
  - `…getFrameTimelinesLength`, `…getPreferredFrameTimelineIndex` ("the default by which the platform scheduled the
    app").
  - `…getFrameTimelineExpectedPresentationTimeNanos`: "This time should be used to advance any animation clocks".
  - `…getFrameTimelineDeadlineNanos`: "needs to be ready by in order to be presented on time".
  - `…getFrameTimelineVsyncId`.
  - `getFrameTimeNanos` "should **not** be used to advance animation clocks".
  - `AChoreographer_registerRefreshRateCallback` is API 30. From API 31 it is consistent with DisplayManager.
- **Swappy (Android Frame Pacing), page updated 2026-02-26 [doc]** https://developer.android.com/games/sdk/frame-pacing
  - It names the failure: "buffer-stuffing … The display pipeline contains a queue of frames, typically of size 2,
    which fills up if the game is trying to present frames too quickly."
  - It uses `EGL_ANDROID_presentation_time` / `VK_GOOGLE_display_timing` "so that frames are not presented early",
    and fences "to inject waits into the application that allow the display pipeline to catch up, rather than
    allowing back pressure to build up".
- **Swappy constants [src, `SW/games-frame-pacing/common/SwappyCommon.h`, `.cpp`].**
  - `FRAME_MARGIN = 1ms`; `NON_PIPELINE_PERCENT = 50`; `FRAME_DROP_THRESHOLD = 10` (%);
    `FRAME_DURATION_SAMPLE_SECONDS = 2s`; `MAX_DURATION = 100ms` (per-sample clamp);
    `mAutoSwapIntervalThreshold = 50ms` ("20FPS"); `REFRESH_RATE_MARGIN = 500ns`.
  - Frame time: pipelined `max(cpu, gpu) + FRAME_MARGIN`; non-pipelined `cpu + gpu + FRAME_MARGIN`.
  - Auto mode:
    - if missed frames exceed 10%, swap slower;
    - if missed frames are 0% and the pipelined time fits under the lower bound, swap faster;
    - "To be very conservative, switch to non-pipeline if frame time * 50% fits", i.e.
      `nonPipelineFrameTime × 1.5 < upperBound`.
  - Target present: `mPresentationTime = currentFrameTimestamp + (mAutoSwapInterval × intervals) × refreshPeriod`.
    With SurfaceFlinger's sf-to-vsync delay it is corrected by `sfToVsyncDelay − refreshPeriod/2 −
    measuredSwapDuration − 1ms`.
- **[inf] for us:** Swappy's pipeline/non-pipeline choice is our "UI and compositor in the same interval" versus "UI
  starts in the previous interval" (§2.6). Its 1.5× rule is a good hysteresis for switching back to low latency.

### 1.5 Apple

- **`CAMetalDisplayLink` [doc]** (macOS 14, iOS 17).
  https://developer.apple.com/documentation/quartzcore/cametaldisplaylink
  - "a specialized way to interact with variable-rate displays when you need more control over the timing window".
  - The delegate's `metalDisplayLink(_:needsUpdate:)` gets updates "based on the preferredFrameRateRange and
    preferredFrameLatency".
  - `preferredFrameLatency`: "The amount of time, in frames, your app requests to render a frame. The final latency
    may be bigger if the system needs more time, such as for windowed modes on macOS. The only acceptable values are
    1.0 and 2.0."
  - `Update.targetTimestamp`: "A deadline that indicates when your app needs to finish rendering to the drawable.
    Your app needs to call the drawable instance's present() method before the deadline. GPU rendering can continue
    after this time, based on preferredFrameLatency."
  - `Update.targetPresentationTimestamp`: "The time the system estimates until the display of the next frame. Update
    your animations based on the time difference between this timestamp and the previous timestamp."
  - Delegate doc: "Using alternative methods to present() that target the presentation for a specific time cause an
    assert when used with a CAMetalDisplayLink."
- **`NSView.displayLink(target:selector:)` (macOS 14) [doc]**: "If the view is hidden, or not on any display, the
  callback will not be invoked." `NSScreen.displayLink` notes that views and windows "can move between screens".
- **`CVDisplayLinkCreateWithActiveCGDisplays` [doc]**: deprecated macOS 15.0 (Mac Catalyst 18.0).
  - Zed's macOS backend still paces on CVDisplayLink, with one immortal link per display because "CVDisplayLink has
    no safe teardown" (`ZED/crates/gpui_macos/src/display_link.rs`) [src].
  - thume.ca (2017) found it can fall back to a free-running timer on some multi-display setups, adding up to a
    frame [src2, via `browsers-flutter-latency.md` §4.4].
- **Feedback [doc].** `MTLDrawable.addPresentedHandler(_:)` and `presentedTime` (macOS 10.15.4): "The host time, in
  seconds, when the drawable was displayed onscreen … 0.0 if the drawable hasn't been presented or if its associated
  frame was dropped."
- **`CAMetalLayer.maximumDrawableCount` [doc]**: "2 or 3 only … default value is 3".
- **Timebase [doc].**
  - `CACurrentMediaTime()` is "derived by calling mach_absolute_time()".
  - `mach_absolute_time` "does not increment while the system is asleep"; "Prefer … clock_gettime_nsec_np(CLOCK_UPTIME_RAW)".
  - `NSEvent.timestamp`: "The time when the event occurred in seconds since system startup".
  - `UIEvent.timestamp` points to `ProcessInfo.systemUptime` ("time the system has been awake").
  - So events, display-link timestamps and presented times share one clock [inf].
- **Hitches [doc]** https://developer.apple.com/documentation/xcode/understanding-hitches-in-your-app
  - The hitch rate is "milliseconds of pause per second (ms/s) … at or below 10 ms/s is good; at or below 25 ms/s is a
    warning; at or below 50 ms/s is critical".
  - "a delay as small as the length of a single refresh interval … can cause a hitch"; discrete delays become hangs
    "for about 50 ms to 100 ms or longer" (understanding-user-interface-responsiveness).

### 1.6 Windows

- **DXGI frame statistics [doc]** https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ns-dxgi-dxgi_frame_statistics
  (updated 2024-02-22).
  - Fields: `PresentCount`, `PresentRefreshCount`, `SyncRefreshCount`, `SyncQPCTime` (QPC), `SyncGPUTime` ("Reserved.
    Always returns 0").
  - It works only for "swap chains that either use the flip presentation model or draw in full-screen mode".
  - `IDXGISwapChain::GetFrameStatistics` remarks: "Statistics are not reliable in many multiple monitor scenarios, as
    well as scenarios where other fullscreen apps are running. It may be necessary to call DwmFlush before frame
    statistics are updated on systems that support Hardware Flip Queue."
- **Compositor clock [doc]**
  https://learn.microsoft.com/en-us/windows/win32/directcomp/compositor-clock/compositor-clock (updated 2025-03-13).
  - `DCompositionWaitForCompositorClock(count, handles, timeoutInMs)` needs Windows build 22000.
    - "If the display is off, then the function returns immediately with STATUS_GRAPHICS_PRESENT_OCCLUDED."
    - It "will always wake at the vertical blank of a monitor, or on another timer".
    - "the DXGI swapchain API always uses the cadence of the primary monitor".
  - `DCompositionBoostCompositorClock(bool)` drives Dynamic Refresh Rate (e.g. 60 → 120 Hz), with "boost requests …
    reference count[ed]".
  - Stats: `DCompositionGetFrameId(CREATED|CONFIRMED|COMPLETED)`, `DCompositionGetStatistics`,
    `DCompositionGetTargetStatistics` (`COMPOSITION_TARGET_STATS { outstandingPresents, presentTime,
    vblankDuration, presentedStats, completedStats }`).
    - Microsoft says "We expect applications to use the frame statistics feature mostly for telemetry, not for
      adjusting content".
    - The stats are "incomplete … in iflip".
  - `IDCompositionDevice::GetFrameStatistics().nextEstimatedFrameTime` gives the next compositor frame.
- **Composition swapchain [doc]** https://learn.microsoft.com/en-us/windows/win32/comp_swapchain/comp-swapchain
  (Windows 11 build 10.0.22000.194, WDDM 2.0; iflip needs WDDM 3.0).
  - Presents carry a *target time*. Among several ready presents, "all but the latest … will be *skipped*".
  - A statistics queue of "512-1024" items gives per-present status.
  - At most 31 presentation buffers.
  - This is the only Windows API with both "present at time T" and per-present feedback [inf].
- **GPUI Windows vsync [src, `ZED/crates/gpui_windows/src/vsync.rs`]**
  - One vsync thread for all windows waits on `DwmFlush`.
  - "DwmFlush and DCompositionWaitForCompositorClock returns very early instead of waiting until vblank when the
    monitor goes to sleep or is unplugged … We use 1ms as a threshold … and fallback to Sleep()"
    (`VSYNC_INTERVAL_THRESHOLD = 1ms`, `DEFAULT_VSYNC_INTERVAL = 16_666 µs`).
  - The interval comes from `DwmGetCompositionTimingInfo().qpcRefreshPeriod`, with a sanity check: "A 29
    microsecond interval was seen".
- **Timers [doc].**
  - `CreateWaitableTimerExW(…, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION /*0x2*/, …)`: "for time-critical situations
    when short expiration delays on the order of a few milliseconds are unacceptable. This value is supported in
    Windows 10, version 1803, and later."
  - `timeBeginPeriod`: from Windows 10 2004 it "no longer affects global timer resolution". "Starting with Windows
    11, if a window-owning process becomes fully occluded, minimized, or otherwise invisible … Windows does not
    guarantee a higher resolution than the default system resolution."
- **Input time [doc].**
  - `POINTER_INFO.PerformanceCount`: "The value of the high-resolution performance counter when the pointer message
    was received … can be calibrated when the touch digitizer hardware supports the scan timestamp information".
  - `dwTime` is the tick count. `historyCount` is the "Count of inputs that were coalesced into this message".
  - `GetMessageTime()`: "elapsed time, in milliseconds, from the time the system was started to the time the message
    was created (that is, placed in the thread's message queue)", and it wraps.
- **DXGI waitable**: see `windows.md` §2.2 and `browsers-flutter-latency.md` §4.5. wgpu's DX12 backend already
  waits on it in `acquire_texture` (`windows.md` §3).

### 1.7 Linux / Wayland / Vulkan

- **`wl_surface.frame` [doc, wayland.xml]**: a "frame throttling hint". The callback time is ms "with an undefined
  base". "A server should avoid signaling the frame callbacks if the surface is not visible."
  Input `time` arguments are a "timestamp with millisecond granularity". `wl_pointer.frame` (v5) groups events.
- **`wp_presentation` v3** (presented time, refresh, MSC, flags `vsync|hw_clock|hw_completion|zero_copy|fixed_rate|
  variable_rate`, `discarded`, `clock_id`), **fifo-v1** and **commit-timing-v1** (`set_timestamp`: "presented as
  closely as possible to, but not before"): `gtk-qt-wayland.md` §3.2–3.3.
- **`zwp_input_timestamps_v1` (unstable) [doc]**
  `https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/b9edf665…/unstable/input-timestamps/input-timestamps-unstable-v1.xml`.
  It gives a "high-resolution version of the timestamp argument of the associated input event … in the same clock
  domain", as `tv_sec_hi/lo, tv_nsec`. Its compositor support was not checked today [mem: Weston and Mutter
  implement it].
- **evdev [doc]** `Documentation/input/input.rst`: "time is the timestamp, it returns the time at which the event
  happened". `EVIOCSCLOCKID` "Set clockid to be used for timestamps" (`include/uapi/linux/input.h`). [mem] libinput
  sets `CLOCK_MONOTONIC`; HID multitouch reports a device `MSC_TIMESTAMP` in µs.
- **Vulkan present timing [src, `Vulkan-Docs/proposals/VK_EXT_present_timing.adoc`]**. It was merged late 2025 for
  1.4.335 (Khronos blog 2025-12-04: NVIDIA developer drivers now, Mesa "coming soon", Android 17).
  - Stages: `VK_PRESENT_STAGE_QUEUE_OPERATIONS_END_BIT_EXT`, `…_REQUEST_DEQUEUED_BIT_EXT`,
    `…_IMAGE_FIRST_PIXEL_OUT_BIT_EXT`, `…_IMAGE_FIRST_PIXEL_VISIBLE_BIT_EXT`.
  - `vkGetSwapchainTimingPropertiesEXT` returns `refreshDuration`, `refreshInterval`. "If refreshInterval is
    UINT64_MAX, the presentation engine is operating in VRR mode, and refreshDuration is the minimum duration";
    equality means FRR.
  - Scheduling: `VkPresentTimingInfoEXT.targetTime`, with `…PRESENT_AT_RELATIVE_TIME_BIT_EXT` and
    `…PRESENT_AT_NEAREST_REFRESH_CYCLE_BIT_EXT`.
  - Feedback: `vkGetPastPresentationTimingEXT`. Time domains: `VK_TIME_DOMAIN_PRESENT_STAGE_LOCAL_EXT`,
    `…SWAPCHAIN_LOCAL_EXT`. It requires `VK_KHR_present_id2`, `VK_KHR_calibrated_timestamps`.
  - `VK_KHR_present_wait2` (2025-05-30): per-surface replacement for present_wait. It drops "time to light"/"first
    pixel" language.
  - `VK_GOOGLE_display_timing` (2017) is the predecessor.
  - Mesa WSI maps present timing to `wp_presentation` and commit-timing on Wayland (`gtk-qt-wayland.md` §3.3).
- **Timers [doc].** `prctl(PR_SET_TIMERSLACK)`: the default slack is 50 µs (init); it affects `select/poll/epoll/
  clock_nanosleep/nanosleep/futex`; "Timer slack is not applied to threads that are scheduled under a real-time
  scheduling policy". `time(7)`: with high-resolution timers, accuracy "can be as accurate as the hardware allows
  (microsecond accuracy is typical of modern hardware)".

### 1.8 Zed / GPUI

- **`InputRateTracker` [src, `ZED/crates/gpui/src/window.rs`]**: "Tracks input event timestamps to determine if input
  is arriving at a high rate. Used for selective VRR … optimization."
  - Defaults: `window: 100ms`, `inputs_per_second: 60`, `sustain_duration: 1s`. High rate means at least 6 events
    in 100 ms; it is sustained 1 s after the last qualifying event.
  - In the frame callback: "Keep presenting if input was recently arriving at a high rate (>= 60fps) … to prevent
    display underclocking during active input." When not dirty but high-rate, it calls `window.present()` without
    drawing.
- **Throttles [src].**
  - Inactive windows: `inactive_frame_interval: Some(Duration::from_micros(33_333))` (`platform.rs`), skipped while
    high-rate input or `require_presentation`.
  - Thermal Serious/Critical: `16667 µs`.
  - A throttled request calls `schedule_frame()` plus `wake_platform()` "so demand-driven platforms … retry".
- **Re-entrancy [src].** A frame request that arrives while a draw is on the stack (Windows nested message pump) is
  deferred, not nested: "the platform leaves the window invalidated … so a fresh request arrives once the in-progress
  draw unwinds". This is the modal-loop hazard of `windows.md`.
- **TestScheduler [src, `ZED/crates/scheduler/src/test_scheduler.rs`, `clock.rs`]**: see §6.1.

---

## 2. Design: the tessel frame scheduler

All of §2 is **[inf]** (a design proposal derived from §1), except where a source is cited.

### 2.1 Time

- **One timebase per process:** `Ticks(u64 ns)` from the OS monotonic clock that the display and input APIs use:
  - Windows: QPC (DXGI `SyncQPCTime`, `POINTER_INFO.PerformanceCount`, compositor stats).
  - macOS: `mach_absolute_time` / `CLOCK_UPTIME_RAW` (CACurrentMediaTime, NSEvent, `presentedTime`,
    `targetTimestamp`).
  - Linux: `CLOCK_MONOTONIC`. Assert `wp_presentation.clock_id == CLOCK_MONOTONIC`, otherwise map via a paired
    sample.
- Every scheduler input is converted to `Ticks` at the boundary. Millisecond event times (Wayland `time`,
  `GetMessageTime`) are unwrapped and validated as Chromium does (`CR/ui/events/base_event_utils.cc`):
  `IsValidTimebase: 0 ≤ now − ts ≤ 60 s`, else use now. "Some fraction of devices, across all platforms provide bogus
  event timestamps".
- The scheduler core never calls the OS clock. It gets `now` passed in, which is what makes §6 possible.

### 2.2 Signals into the scheduler

| Signal | From | Meaning |
|---|---|---|
| `NeedsUiFrame{reason}` | UI thread | scene dirty (style/layout/content), rAF requested, UI-side animation running |
| `UrgentInput{seq, t_os}` | UI thread | a discrete event dirtied the scene (§5.1 of the proposal) |
| `HighRateInput{t}` | UI thread | a continuous event arrived; feeds the `InputRateTracker` |
| `CommitArrived{commit_id, input_seqs, t}` | UI → compositor | a commit is in the mailbox |
| `PropsArrived` | UI → compositor | fast commit (scroll offset / camera) in the property mailbox |
| `CompositorAnimating(bool)` | compositor | property animations, fling, caret blink need frames |
| `VsyncTick{t_vsync, interval, target_present}` | platform clock source | DisplayLink update, compositor-clock wake, waitable signalled, `wl_callback.done` |
| `PresentFeedback{frame_id, presented_at or Discarded, refresh, flags}` | platform | `wp_presentation`, `presentedTime`, DXGI stats, `vkGetPastPresentationTimingEXT` |
| `GpuDone{frame_id, t_cpu, gpu_ns}` | wgpu `on_submitted_work_done` + timestamp readback | GPU completion and cost |
| `Visibility(Visible / Occluded / Hidden)` | platform | occlusion state, minimise, frame-callback timeout |
| `OutputChanged{interval, vrr_range, latch_hint}` | platform | window moved screens, mode change, DRR boost |
| `Timer{deadline_kind}` | high-res timer | a deadline the scheduler armed fired |

### 2.3 Compositor-thread `FrameClock` (one per window)

State (orthogonal parts):
- `phase ∈ { Idle, Armed, InBeginFrame, InDeadline }`. Idle means unsubscribed from the clock; Armed means subscribed,
  waiting for a tick. This maps to Chromium's `IDLE / INSIDE_BEGIN_FRAME / INSIDE_DEADLINE`, plus the
  unsubscribed/subscribed distinction (`ShouldSubscribeToBeginFrames`).
- `ui ∈ { UiIdle, UiSent{for_target}, UiReady{commit} }`. This maps to Chromium's `BeginMainFrameState
  IDLE/SENT/READY_TO_COMMIT`.
- `in_flight: Option<FrameId>`: a present not yet acknowledged by feedback or by buffer availability. The cap is 1.
- `needs_draw: bool`: damage from props, animations or an applied commit.
- `visible: Visibility`.
- `mode ∈ { Fixed, Variable }` (VRR) and `deadline_mode ∈ { None, Immediate, Regular, Late, Blocked }`.

**Deadline modes (ours):**

| Mode | When | Fires at |
|---|---|---|
| `Immediate` | a commit is applied or ready and nothing else is expected this frame; or urgent; or first frame after idle | now |
| `Regular` | the compositor has its own damage (animation, scroll, caret) **and** a UI frame was sent for this target | `D_comp = T − L_latch − est_gpu − est_comp_cpu` |
| `Late` | only waiting for a UI commit; no compositor damage | `T + interval − L_latch − est_gpu − est_comp_cpu` (give the UI until the *next* deadline; Chromium LATE = `frame_time + interval`) |
| `Blocked` | headless/scenario full-pipeline mode only (§6.3) | never; draw when the commit lands |
| `None` | Idle / hidden | — |

**Transition table (compact):**

| # | From | Event / guard | Action | To |
|---|---|---|---|---|
| 1 | Idle | `NeedsUiFrame` ∨ `CommitArrived` ∨ `PropsArrived` ∨ `CompositorAnimating(true)`, and visible | subscribe to the clock; if `idle_for ≥ 2·interval` and urgent-eligible (§2.5), take the urgent path | Armed |
| 2 | Armed | `VsyncTick(T)` | compute `S_ui, D_ui, D_comp` (§3); if UI wanted and `ui == UiIdle` and no unapplied commit, post `BeginUiFrame{T, D_ui}` at `S_ui` (or now if `S_ui ≤ now`) and set `ui = UiSent`; choose `deadline_mode`; arm the timer | InBeginFrame |
| 3 | InBeginFrame | `CommitArrived` ∧ `ui == UiSent` | `ui = UiReady`; re-evaluate: if no compositor-only work is pending, use `Immediate` | InBeginFrame |
| 4 | InBeginFrame | deadline timer ∨ `Immediate` | | InDeadline |
| 5 | InDeadline | `in_flight.is_none()` ∧ (`needs_draw` ∨ `UiReady`) | apply the newest commit (if any) plus the prop mailbox; tick animations at `target_present`; acquire late; encode; submit; present; `in_flight = Some(id)`; record `ui_missed = (ui == UiSent)` | InBeginFrame*, then Armed on the next tick |
| 6 | InDeadline | `in_flight.is_some()` | **skip** (throttled, Chromium `IsDrawThrottled`); count `comp_throttled`; keep damage | Armed |
| 7 | InDeadline | no damage, no commit | `DidNotProduceFrame` (count `no_damage`) | Armed or Idle (rule 9) |
| 8 | any | `PresentFeedback(id)` | `in_flight = None`; update `L_latch`, estimates, the vsync phase model; if presented later than `T`, record `comp_missed` | same |
| 9 | Armed | after one tick with nothing to do (proactive hysteresis, Chromium `did_attempt_draw_in_last_frame_`), ¬`HighRateInput` sustain, ¬animating | unsubscribe; idle counters | Idle |
| 10 | any | `Visibility(Hidden)` | unsubscribe; drop pending deadline; keep damage; `ui` stays (the UI will be told not to start frames) | Idle (`visible = Hidden`) |
| 11 | Idle (Hidden) | `Visibility(Visible)` | force `needs_draw`; urgent path (one immediate frame) | Armed |
| 12 | any | `OutputChanged` | reset the phase model (keep estimates); first frame `Immediate` | same |

`*` After a draw the clock stays in its current tick until the next `VsyncTick`. A second draw in the same
target interval is never allowed ("Do not draw more than once in the deadline").

### 2.4 UI-thread frame state

`ui_state ∈ { Idle, Scheduled{target T, start S_ui}, Running{T}, Committed{T, commit_id} }`.

| From | Event / guard | Action | To |
|---|---|---|---|
| Idle | scene becomes dirty | send `NeedsUiFrame` (once; it is a level, not a counter) | Idle |
| Idle | `BeginUiFrame{T, D_ui}` | run phases 1–7 of proposal §5.2 | Running |
| Idle | discrete event dirtied the scene ∧ urgent rule (§2.5) says `T_now` is reachable | run phases at once for `T_now` | Running |
| Running | phase 7 commit | `commit_id++`, attach input seqs; if `now > D_ui`, record `ui_late` (the compositor shows it next frame) | Committed |
| Committed | compositor `CommitApplied(commit_id)` | | Idle |
| Committed | scene dirty again | do **not** start: set `NeedsUiFrame` and wait for apply (back-pressure) | Committed |

The UI thread reads a published `FrameArgs` snapshot (an atomic or seqlock written by the compositor) holding
`T_next, D_ui(T_next), interval, est_ui, idle_since`. That lets the urgent check run without a round trip.

### 2.5 Urgent frames

A discrete event (key, button, IME commit, wheel start) that changes the scene is handled at event time (proposal
§5.1). Then:

```text
urgent(now):
  args = frame_args.load()
  if compositor clock is Idle and last present ≥ 2 intervals ago:      // Mutter should_update_now
      run UI phases now; compositor draws Immediate on commit; T = next latchable vblank
  else if ui_state == Idle and now + est_ui_input ≤ D_ui(args.T_next):   // still makes this vblank
      run UI phases now for T_next (a second UI frame for T_next is allowed only if the
      first one has not been drawn yet, like Chromium's urgent_begin_main_frame_pending_ exception)
  else:
      leave it to the normal BeginUiFrame for T_next+1 (starts at S_ui(T_next+1)):
      same present time, and later events in this interval batch into it
```

- `est_ui_input` is a separate estimate for frames that contain discrete input (Chromium separates
  critical/not-critical BMF estimates).
- Native-only feedback (focus ring, press state, caret) never waits for JS (proposal §7.2), so the native commit can
  go out urgently even when the JS-dependent change goes out a frame later.

### 2.6 Missed deadlines and the two-deadline rule

- **UI misses `D_ui`.** The compositor still fires at `D_comp` (Regular) and draws compositor-only updates: scroll,
  props, animations, caret. The late commit is applied at the next tick.
  - This is "two deadlines": the compositor's deadline is final, the UI's is soft (Chromium REGULAR mode;
    `how_cc_works.md` "draws without waiting for the commit").
  - If the compositor has no damage, it waits in `Late` mode up to the next deadline instead of drawing nothing.
- **Sustained UI overrun (high-latency mode).** When `est_ui + est_comp > interval − L_latch`, the UI start for `T`
  falls in the previous interval: `S_ui(T) < T − interval`.
  - That is allowed (pipelined) because there is still only one commit in flight.
  - The clamp is `est_total ≤ 2 × interval` (KWin and Mutter clamp to 2× refresh). Above that the UI frame rate
    halves while the compositor keeps full rate.
  - Return to non-pipelined when `est_total × 1.5 < interval − L_latch` (Swappy `NON_PIPELINE_PERCENT`) for 10
    consecutive frames (KWin double-buffer hysteresis).
- **Compositor misses its latch.** The present lands at `T + k·interval` (k ≥ 1) per feedback.
  - Raise `L_latch` by the overshoot at once (KWin DRM rule) and record `comp_missed`.
  - Nothing queues behind it, because `in_flight` blocks the next draw.
- **Missed tick (clock source late, e.g. a debugger or system load).** If a `VsyncTick` arrives after its own
  `D_comp`, drop it (Chromium: "Discard missed begin frames if they are too late") and re-arm for the next. Never run
  two frames back to back to "catch up".
- **Forced progress.** If the UI is `Committed` but the compositor has failed to draw 3 times in a row (for example
  lost surface or acquire timeout), force a full redraw (Chromium
  `maximum_number_of_failed_draws_before_draw_is_forced = 3`).

### 2.7 Idle

- Nothing dirty, no animations, no high-rate sustain, nothing in flight → unsubscribe after one quiet tick (rule 9).
  With no clock subscription:
  - Windows: we stop waiting on the compositor clock, and the doc says the OS turns off vblank interrupts when no one
    waits.
  - macOS: `CAMetalDisplayLink.isPaused = true`.
  - Wayland: no `wl_surface.frame` is requested.
- The proposal's target ("0 frames and no timer wakeups") then holds by construction. Verify with counters:
  `wakeups_per_s`, `ticks_without_work`.
- Idle → active uses the urgent path (§2.5). The KWin "> 100 vblanks → GPU likely in a low power state" rule is moot
  for the first frame (we start at once) but matters for animations: the first animation frame after a long idle uses
  `est_gpu = max(est_gpu, interval/2)` [inf].

### 2.8 Hidden and occluded windows

| OS | Detect | Behaviour |
|---|---|---|
| Windows | `WM_SIZE`/minimise, `DXGI_STATUS_OCCLUDED` from Present [mem], compositor-clock wait returning `STATUS_GRAPHICS_PRESENT_OCCLUDED` "If the display is off" [doc], a wait returning in < 1 ms (GPUI heuristic) [src] | Hidden: unsubscribe. Occluded-but-present: throttle to 1 frame per 250 ms for liveness [inf]. Win11 does not guarantee timer resolution for occluded processes [doc], so do not depend on high-res timers there. |
| macOS | `NSWindow.occlusionState`, `NSView.displayLink` not invoked when hidden [doc] | stop on occlusion; one urgent frame on becoming visible |
| Wayland | no frame callbacks for invisible surfaces [doc]; 100 ms frame-callback timeout marks hidden (Qt heuristic, proposal §8.2) | stop requesting callbacks; if using fifo-v1, keep frame-callback throttling ("Clients must use an additional mechanism") [doc] |

The UI thread keeps processing events and JS timers while hidden. rAF callbacks and UI animations pause [inf: browser
convention]. Commits are not produced; dirt accumulates and one frame is produced on visibility.

### 2.9 Multi-window, different refresh rates

- **One compositor thread, N `FrameClock`s** (proposal §8.3).
- The thread's single wait is the minimum over all windows' next timer, plus the platform handles:
  - Windows: per-swapchain waitable objects plus app events, via `MsgWaitForMultipleObjectsEx`/`WaitForMultipleObjects`.
  - macOS: CAMetalDisplayLinks on the compositor thread's run loop.
  - Wayland: the compositor thread's own `wl_event_queue` [inf].
- Each window has its own phase model, `L_latch` and estimates. The UI estimate is shared, since the UI thread is
  one thread.
- **GPU contention.** Draws for windows whose deadlines collide are ordered by deadline (earliest first). A draw
  that cannot start before its deadline because another window's encode is running counts as `comp_contention`.
  Estimates are per window, so a heavy viewport window does not inflate a light window's estimate.
- **UI frames for several windows.** The UI thread runs one UI frame per `BeginUiFrame` and may update several
  windows' scenes in one pass. It serves the earliest `D_ui` first. Commits are per window.
- **Windows specifics.** DXGI uses the primary monitor's cadence [doc]. Per-window waitable objects give per-window
  cadence. The compositor clock is "display-independent" but one heartbeat: use it as the wake source only when all
  windows are on one output, otherwise use the waitables [inf].
- **macOS.** Use `NSView`/`NSWindow`-derived display links, which "track those changes automatically" [doc]. Never
  use one CVDisplayLink per process.

### 2.10 VRR, ProMotion and dynamic refresh

- `mode = Variable` when the platform reports it:
  - `wp_presentation` `variable_rate` flag;
  - `VkSwapchainTimingPropertiesEXT.refreshInterval == UINT64_MAX`;
  - `CAMetalDisplayLink` frame-rate ranges;
  - Windows DRR "boost" state.
- **Variable mode target:** `T = max(now + est_total, last_present + min_refresh_duration)`. This is Mutter's
  `calculate_next_variable_update_time_us` and KWin's adaptive-sync branch.
- Input-driven frames present `Immediate` after the commit. Animation frames are paced at a steady rate (the panel
  maximum, or the highest divisor the estimate sustains), so animation timing does not jitter (Levien,
  `browsers-flutter-latency.md` §4.3).
- **Keep presenting after input.** On high-rate input (GPUI defaults: ≥ 60/s over 100 ms, sustain 1 s), present
  every tick even without damage (re-present the last frame; GPUI calls `present()` without drawing). This keeps
  ProMotion/DRR from downclocking. Stop when the sustain lapses.
- **Windows DRR.** `DCompositionBoostCompositorClock(true)` on pointer-down of a drag, pan or ink, and `false` on
  release (ref-counted) [doc].
- **macOS ProMotion.** Set `preferredFrameRateRange` to the panel maximum while input or animation is active, and
  lower it when idle.
- **Animation time** = `targetPresentationTimestamp` (Apple), `ExpectedPresentationTime` (Android), the predicted
  `T` elsewhere. Never use frame-start wall time.

### 2.11 Back-pressure (never more than one frame in flight)

1. **Swapchain:** `desired_maximum_frame_latency = 1` (wgpu: DX12 `SetMaximumFrameLatency(1)`, Metal
   `maximumDrawableCount = 2`, Vulkan `minImageCount = 2`; `browsers-flutter-latency.md` §4.6). Prefer Mailbox
   where it is cheap, otherwise Fifo with `fifo-v1`.
2. **Compositor:** `in_flight ≤ 1` (rule 6; Chromium `kMaxPendingSubmitFrames = 1`; KWin `maxPendingFrameCount = 1`
   under VRR). Acquire the surface texture as late as possible, after encoding instance buffers (MS: "delay this
   point as far into the frame as possible").
3. **UI → compositor:** one commit slot. The UI never starts a frame while its last commit is unapplied (§2.4).
   Fast commits (props) go to a separate newest-wins mailbox and never wait.
4. **Feedback absent.** If no `PresentFeedback` arrives within `3 × interval`, treat the frame as presented at
   `T + interval` (count `feedback_timeout`). `in_flight` is still released by buffer availability (acquire success
   or the DXGI waitable), so a missing feedback never wedges the loop.

### 2.12 Pseudocode (compositor thread, per iteration)

```rust
loop {
    let next = clocks.iter().map(FrameClock::next_wake).min();   // timer deadlines + platform waits
    let ev = platform.wait(next);                                // ONE blocking wait (proposal §5.3)
    let now = clock.now();
    for c in clocks.affected_by(&ev) {
        c.on_event(ev.for_window(c.id), now);                    // pure state update
        while let Some(action) = c.next_action(now) {            // pure decision (Chromium split)
            match action {
                Action::PostBeginUiFrame { target, d_ui, at } => ui.post_at(at, BeginUiFrame { target, d_ui }),
                Action::ArmTimer { at, kind }                => timers.arm(c.id, at, kind),
                Action::Draw { target }                      => { let f = c.draw(target, now); c.did_draw(f, clock.now()) }
                Action::DidNotProduceFrame                   => c.did_not_draw(now),
                Action::Subscribe(on)                        => platform.subscribe(c.window, on),
                Action::Present { re_present: true }         => c.represent(),   // VRR keep-alive
            }
        }
    }
}
```

---

## 3. Render-time estimation

### 3.1 Prior art compared

| System | Sample | Statistic | Margin | Floor / clamp | Sources |
|---|---|---|---|---|---|
| Chromium cc | draw duration; BMF queue→activate etc. | **p90 of last 60** | `kDeadlineFudgeFactor = 1 ms` | Viz reserves `interval/3` for the display | §1.1 |
| KWin | wakeup → GPU end | EWMA τ = 500 ms + 2 × positive-peak (τ = 6 s) | DRM safety: base `vblank + 1000 µs` + adaptive (+δ now, −slack/10, ≤ min(3 ms, interval/2)) | sample floor 2 ms; total ≤ 2 × vblank; > 100 idle vblanks → ≈ 1 vblank | §1.2 |
| Mutter | dispatch → max(GPU done, KMS ready) | asymmetric max tracker (down /(4·rate), up /rate or jump) | 1000 µs + dynamic margin ≤ refresh/4 (raise ½, lower /60 after 1 s) | ≤ 2× (3× triple) refresh; min estimate as rescue | §1.3 |
| Swappy | CPU and GPU per frame | mean over 2 s | `FRAME_MARGIN = 1 ms` | per-sample ≤ 100 ms | §1.4 |
| Levien | — | p99.99 ("one jank per minute") | — | — | `browsers-flutter-latency.md` §4.3 |
| farnoy patch | KWin samples | decay toward p95 of 512 frames | KWin margin reduced | no 2 ms floor | §1.2 |

### 3.2 tessel estimator [inf]

Per window there are four cost series. A fifth, the UI cost, is shared by all windows:

| Series | Sample (all in `Ticks`) | Source |
|---|---|---|
| `ui` | `BeginUiFrame` received (or urgent start) → commit sent; split `ui_input` / `ui_other` | UI-thread clock reads |
| `comp_cpu` | deadline fired → `queue.submit` + `present` returned | compositor clock reads |
| `gpu` | submit → GPU done | `Queue::on_submitted_work_done` callback time (wgpu 30.0.1: "invoked when the previous Queue::submit finishes executing on the GPU" [doc]), cross-checked with timestamp queries (below) |
| `latch` (`L_latch`) | present call → the latch the frame made; learned from feedback | platform feedback |

**Statistic.** A ring of the last **64** samples. `q = p90(ring)`, like Chromium's 60-sample p90. 64 samples is
about 0.5 s at 120 Hz.

**Margin.** One margin per window, `m`, updated from feedback, following KWin's DRM rule:
- `slack = (T − L_latch_used) − t_present_call`.
- On a miss, `m += −slack` (now).
- On a hit, `m −= min(slack, m)/10`.
- Clamp `m ∈ [0, min(3 ms, interval/2)]`.

**Estimate.** `est(series) = max(floor, q)`.
- `floor = 0.25 ms` for CPU series and `0.25 ms` for GPU. KWin's 2 ms floor is what made it overestimate by 5×
  (farnoy).
- `S_ui = T − (L_latch + m) − est_gpu − est_comp_cpu − ε_commit − est_ui`, with `ε_commit = 50 µs` (Arc-swap
  hand-off).
- `D_ui = S_ui + est_ui`; `D_comp = T − (L_latch + m) − est_gpu − est_comp_cpu`.

**Outlier handling.** A single sample over `2 × interval` (GC pause, page fault, shader compile) is recorded in
telemetry. It enters the ring capped at `2 × interval`, as Mutter caps at `3 × refresh` and Swappy at 100 ms. One
hitch then cannot pin the estimate for 64 frames longer than one ring rotation.

**GPU cost [doc + inf].**
- With `Features::TIMESTAMP_QUERY` (Vulkan, DX12, Metal, GL with `GL_ARB_timer_query`, WebGPU): write timestamps at
  the beginning of the first and end of the last render pass (`RenderPassTimestampWrites`), resolve into a buffer,
  read back with a 2–3 frame lag, and multiply by `Queue::get_timestamp_period()` ("Returns zero if timestamp queries
  are unsupported").
  - This measures pure execution (`gpu_exec`). `submit → done` from `on_submitted_work_done` measures queueing plus
    execution, which is what the deadline needs.
  - Use the callback series for scheduling and the timestamp series for diagnosis ("GPU busy" vs "GPU queued
    behind another window or process").
- `TIMESTAMP_QUERY_INSIDE_PASSES` / `_INSIDE_ENCODERS` are "not Apple GPUs" and "generally not available on
  tile-based rasterization GPUs" [doc, wgpu-types 30.0.1]. Per-pass breakdowns are therefore a desktop
  discrete/integrated-only diagnostic.
- No CPU↔GPU timestamp calibration exists in the wgpu `Queue` API [doc: not listed]. To place GPU execution on the
  trace timeline, anchor it at `done_cpu − gpu_exec`.

**UI cost.**
- The UI estimate is the hard one, because cost depends on what changed.
  - `ui_input` (frames with discrete input) is used by the urgent check.
  - `ui_other` (animation, rAF, async results) is used by normal scheduling.
- When a frame contains a JS listener, the JS part is also recorded separately (`js`), since the 4 ms soft cap
  (proposal §2) applies to it.
- A frame whose cost exceeds `est_ui` by more than 2× marks the next 8 frames "provisional": they use
  `max(q, last)`. Opening a heavy panel then doesn't miss twice in a row.

**Cold start and resets.**
- Seeds (proposal §2 budgets, with KWin's and Mutter's 1 ms constants):
  `est_ui = 2 ms, est_comp_cpu = 1 ms, est_gpu = 2 ms, m = 1 ms`.
- Latch seeds by platform:

  | Platform | `L_latch` seed | Why |
  |---|---|---|
  | macOS (CAMetalDisplayLink) | `0` | `targetTimestamp` *is* the present deadline [doc] |
  | Windows independent flip / zero-copy Wayland | `1 ms` | |
  | Composed (DWM, Mutter/KWin composited) | `3 ms` | KWin/Mutter compose about 2–4 ms before vblank (`gtk-qt-wayland.md` §3.5); Weston's default repaint window is 7 ms |

- Until the first `PresentFeedback` arrives there is no vsync phase. Use Mutter's fallback: dispatch now, or
  `last_dispatch + interval`. Draw `Immediate` and let the FIFO/waitable block.
- Reset the phase model (not the costs) on `OutputChanged`. Reset all series on GPU device loss or a backend change.

**Recovery time after a fault.** The ring flushes a cost spike in 64 frames (0.53 s at 120 Hz). The margin decays by
1/10 per hit frame, so 90% is gone after `ln(0.1)/ln(0.9) ≈ 22` frames (0.18 s at 120 Hz). §6 tests these numbers.

---

## 4. Clock sources per OS and their accuracy

| OS | Pacing signal (when to start) | Feedback (when it was shown) | Timebase | Accuracy / caveats | Fallback |
|---|---|---|---|---|---|
| **Windows 10/11** | DXGI frame-latency **waitable** per swapchain (wgpu DX12 waits on it in acquire; `windows.md` §3) | `IDXGISwapChain::GetFrameStatistics` (flip model): `PresentCount`, `PresentRefreshCount`, `SyncRefreshCount`, `SyncQPCTime` [doc]; `GetLastPresentCount` [mem] | QPC | "not reliable in many multiple monitor scenarios"; "may be necessary to call DwmFlush … Hardware Flip Queue" [doc] | `DwmFlush` thread (GPUI) with a < 1 ms early-return check → `Sleep(interval)` [src] |
| **Windows 11 (22000+)** | `DCompositionWaitForCompositorClock` (multi-handle, display-independent; boost for DRR) [doc] | `DCompositionGetStatistics/TargetStatistics` (`presentTime`, `vblankDuration`; "incomplete … iflip"); `IDCompositionDevice::GetFrameStatistics.nextEstimatedFrameTime`; composition swapchain stats with **target time** (22000.194+) [doc] | QPC | Returns at once if the display is off [doc]; Zed went back to DwmFlush (`browsers-flutter-latency.md` §4.4) | waitable object |
| **macOS 14+** | `CAMetalDisplayLink` `Update.targetTimestamp` (present deadline) and `targetPresentationTimestamp`; `preferredFrameLatency ∈ {1, 2}` [doc] | `MTLDrawable.addPresentedHandler` → `presentedTime` (0 = dropped) [doc] | `mach_absolute_time` (= CACurrentMediaTime = NSEvent.timestamp) | "final latency may be bigger … windowed modes on macOS" [doc]; present-at-time APIs assert with it [doc] | `NSView.displayLink` (CADisplayLink, macOS 14); CVDisplayLink (creation deprecated 15.0; free-running on some multi-display setups) |
| **Linux Wayland** | `wl_surface.frame` (ms, undefined base; none when invisible) [doc]; Mutter defers callbacks "to the frame's deadline" (`gtk-qt-wayland.md` §3.5) | `wp_presentation.feedback` presented (ns, `refresh`, MSC, flags `hw_clock`/`hw_completion`/`zero_copy`/`vsync`/`variable_rate`) or `discarded` [doc]; `VK_EXT_present_timing` via Mesa WSI | `clock_id` (normally `CLOCK_MONOTONIC`) | Trust `hw_clock` + `hw_completion` timestamps; without them the time is a compositor estimate [inf] | the predicted presentation time ("frame time we received plus one refresh interval", GTK's fallback, `gtk-qt-wayland.md` §1.1) |
| **Linux X11** | Present `PresentCompleteNotify` UST/MSC [mem] | same | UST | winit only for now (proposal §8.2) | timer |
| **Vulkan anywhere** | `VK_EXT_present_timing` `targetTime` (absolute, or relative "minimum duration the previously presented image should be visible"; nearest-refresh-cycle flag) [src] | `vkGetPastPresentationTimingEXT` per stage, up to `…FIRST_PIXEL_VISIBLE` [src]; `VK_KHR_present_wait2` (per-surface wait, implementation-defined completion) [src] | `VK_TIME_DOMAIN_*` (calibrate via `VK_KHR_calibrated_timestamps`) | New in 1.4.335; NVIDIA dev drivers, Mesa pending (2025-12) [doc]. wgpu 30.0.1 exposes none of it [doc: not on `SurfaceTexture`/`Queue`] | `VK_GOOGLE_display_timing` (2017) |
| **Android** (reference only) | Choreographer `postVsyncCallback` with FrameTimelines (API 33) [doc] | SurfaceFlinger / Swappy stats [doc] | `CLOCK_MONOTONIC` | per-timeline deadline + expected present | Swappy |
| **Headless / tests / no feedback** | synthetic phase-locked timer: `tick_n = phase + n × interval`, phase from the last trusted feedback (Chromium `DelayBasedBeginFrameSource`; GTK's 16.667 ms default) | none (feedback = `T`) | process monotonic | timer jitter (below) | — |

**Waking just in time.**

| OS | Primitive | Precision (documented) | Notes |
|---|---|---|---|
| Windows | `CreateWaitableTimerExW(CREATE_WAITABLE_TIMER_HIGH_RESOLUTION)` (1803+), waited together with input/event handles in `MsgWaitForMultipleObjectsEx` | "short expiration delays on the order of a few milliseconds are unacceptable" → high-res [doc]; no µs figure published | Do not rely on `timeBeginPeriod` (per-process since 2004; no guarantee when occluded on Win11) [doc]. Wake about 200 µs early and spin-check `QueryPerformanceCounter` only for the final draw deadline [inf] |
| macOS | `mach_wait_until(abs)` on the compositor thread with `THREAD_TIME_CONSTRAINT_POLICY` (TN2169 example: computation 5 ms, constraint 10 ms) | "consistently missing your scheduled time by more than 500 microseconds, that should be considered an error" [doc TN2169]; "Timers are not accurate across a sleep and wake cycle" | Real-time class only for the compositor thread, never the UI thread ("Don't create large numbers of real time threads") |
| Linux | `timerfd_create(CLOCK_MONOTONIC)` + `timerfd_settime(TFD_TIMER_ABSTIME)` in the compositor's epoll (Mutter does this [src]); `prctl(PR_SET_TIMERSLACK, 1)` on the compositor thread | high-res timers: "microsecond accuracy is typical" [doc time(7)]; default slack 50 µs on poll/epoll/nanosleep [doc]; farnoy measured 51 µs p99 timerfd wakeup deviation [src2] | Not Qt-style ms timers ("round every duration up to the next millisecond") [src2] |

**Measuring the clock source itself [inf].** Record `wake_error = actual_wake − armed_time` as a histogram per OS.
The scheduler adds `p95(wake_error)` to `m`'s floor, so a coarse timer turns into a slightly earlier start instead of
missed frames.

---

## 5. Latency instrumentation

### 5.1 Input timestamps per OS

| OS | Event | Timestamp | Resolution / base | Use |
|---|---|---|---|---|
| Windows | `WM_POINTER*` | `POINTER_INFO.PerformanceCount` (QPC "when the pointer message was received"; hardware scan time when the digitizer reports it) [doc] | QPC ticks | `t_os` |
| Windows | `WM_KEYDOWN`, `WM_MOUSE*` (legacy), `WM_CHAR`, IME | `GetMessageTime()` (ms since boot, "placed in the thread's message queue", wraps) [doc] | 1 ms (tick count; [mem] 10–16 ms granularity unless the timer resolution is raised) | unwrap → `t_os`, flag `coarse` |
| Windows | Raw Input | none in `RAWINPUT` [mem] | — | stamp at `GetRawInputData` (QPC) |
| macOS | `NSEvent` | `timestamp`, "seconds since system startup" [doc] (same base as `mach_absolute_time`) | sub-µs double | `t_os` |
| Wayland | `wl_pointer`/`wl_keyboard`/`wl_touch` | `time` ms, undefined base [doc]; `zwp_input_timestamps_v1` gives ns "in the same clock domain" [doc] | ms → ns with the extension | prefer the extension; else unwrap ms against `CLOCK_MONOTONIC` |
| Linux evdev (headless / own backend) | `input_event.time` "the time at which the event happened" [doc]; clock set by `EVIOCSCLOCKID` [doc] | µs | `t_os` |

Every event also gets `t_recv`, our clock when our thread read it. `t_recv − t_os` is the OS-to-app segment (budget
≤ 1–2 ms, proposal §2). Timestamps are validated with Chromium's 0–60 s rule (§2.1).

### 5.2 Sequence ids through the pipeline

`InputSeq(u64)` is allocated per OS event at `t_recv`. Coalesced moves keep their own seq inside the history and
terminate as `MergedInto(seq_head)`, like Chromium's `…ToTermination` stages (`browsers-flutter-latency.md` §1.4).

**Stage records** (one `LatencyRecord` per `InputSeq`, fixed-size, in a ring):

| Stage | Timestamp | Recorded by |
|---|---|---|
| `os` | `t_os` | platform |
| `recv` | event read from the OS queue | UI thread |
| `dispatch_end` | native hit test + defaults done | UI thread |
| `js_end` | listeners + microtasks drained (absent if none) | UI thread |
| `ui_frame_start` | phases begin (urgent or `BeginUiFrame`) | UI thread |
| `commit` | Commit sent (`commit_id` linked) | UI thread |
| `commit_applied` | compositor applied it | compositor |
| `draw_start` / `submit` | encode start, `queue.submit` | compositor |
| `gpu_done` | `on_submitted_work_done` | compositor |
| `present` | present call | compositor |
| `presented` | feedback timestamp (`frame_id` linked), plus flags (`zero_copy`, `vsync`, iflip) | compositor |

- **Linking.** A transaction carries the set of seqs it consumed as `(min_seq, max_seq, count)` plus up to 4
  explicit discrete seqs. A Commit carries the union over its transactions and `earliest_input_t_os`. Viz uses the
  same "earliest input generation time" idea for deadline selection
  (`DisplayScheduler::CanDrawForPreviousFrame` → `GetEarliestInputGenerationTimeOfDamagedSurfaces`) [src]. A frame
  carries the union of its commit plus its own fast-commit seqs (scroll, camera).
- **Termination.** Every record ends in exactly one terminal state: `Presented(frame_id)`, `MergedInto(seq)`,
  `NoVisualChange` (handler ran, nothing dirtied), `Dropped(reason)` (window hidden, superseded commit). §6 checks
  this as an invariant.
- **Derived segments** (Chrome EventLatency-style names):
  - `OsToRecv`, `RecvToDispatchEnd`, `DispatchToJsEnd`, `HandledToFrameStart` (queueing: the JIT wait),
    `FrameStartToCommit`, `CommitToDraw`, `DrawToSubmit`, `SubmitToGpuDone`, `GpuDoneToPresent`,
    `PresentToPresented`;
  - totals `InputToPresented` and `InputToPresentedFrames = (presented − t_os) / interval`.

### 5.3 In-app histograms

- **Buckets.** Log-linear HDR-style: 2 significant digits, from 10 µs to 10 s, about 1.5 KB per histogram [inf].
  Lock-free per-thread recording, merged on read.
- **Keys:** `(segment, input_class ∈ {key, click, wheel, pointer_move, ime, other}, window)`. There are two windows:
  "last 10 s" and "session".
- **Reported:** p50, p95, p99 and max per key. Frame counters per window:
  - `presented`, `ui_missed`, `comp_missed`, `comp_throttled`, `no_damage`, `feedback_timeout`, `comp_contention`;
  - `idle_wakeups/s`;
  - **hitch time ratio** (ms/s, Apple's thresholds 10/25/50 [doc]) for animation and scroll;
  - `wake_error` p95.
- **Surfaces:**
  - a diagnostics panel;
  - a debug HUD drawn by the compositor itself, so it costs no UI frame;
  - an `rpc`/MCP method returning the snapshot as JSON (an automation surface; proposal §9.5);
  - diagnostic-bundle and scenario headers, which name the pin shas per the cadatomic CLAUDE.md.

### 5.4 Trace export

- **Perfetto / Chrome JSON** [doc] https://perfetto.dev/docs/getting-started/other-formats ,
  https://perfetto.dev/docs/instrumentation/track-events
  - Perfetto opens the Chrome JSON trace-event format and supports its `s`/`t`/`f` flow events "as connecting arrows
    between slices". Durations must nest ("Overlapping, non-nested events are out of spec").
  - Native TrackEvent has flows (`Flow::ProcessScoped(id)` → `TerminatingFlow`), counter tracks with units, and
    custom clocks via `ClockSnapshot` (user clock ids 64–127).
  - **Plan [inf]:** a `tessel-trace` exporter writes Chrome JSON first (no dependency, easy in CI artifacts), and
    Perfetto protobuf later.
    - One track per thread (UI, compositor, GPU-as-async track per window).
    - Slices for phases.
    - A flow per `InputSeq` from `recv` to `presented`.
    - Counters for `est_*`, `m`, `L_latch`.
    - A `ClockSnapshot` mapping our `Ticks` to `BOOTTIME` [mem: Perfetto's default trace clock] so it can be merged
      with system traces (ftrace, ETW conversions).
- **Tracy** (v0.14.1; manual on `master`) [doc] https://github.com/wolfpld/tracy
  - `FrameMarkNamed(name)` per window ("a new set of frames for each unique name"); `FrameMarkStart/End` for
    discontinuous UI frames.
  - `TracyPlot` for the estimates.
  - `FrameImage` for thumbnails (dimensions "must be divisible by 4", `offset` = frames of lag).
  - Rust: the `tracy-client` crate behind a cargo feature [inf].
- **Windows ETW for PresentMon correlation** [src, PresentMon `9db97248`]
  - PresentMon listens to `Microsoft-Windows-DxgKrnl`, `-DXGI`, `-Dwm-Core`, `-Win32k` (input events
    `InputDeviceRead_Stop`, `RetrieveInputMessage_Info`), `-Kernel-Process`, `-D3D9`, `-Direct3D12`, and
    **`Intel-PresentMon` `{ECAA4712-4644-442F-B94C-A32F6CF8A499}`**.
  - The latter accepts app events `AppInputSample_Info{FrameId, InputType}`, `AppSimulationStart/End`,
    `AppRenderSubmitStart/End`, `AppPresentStart/End`, `AppSleepStart/End` (all `{FrameId}`).
  - `--track_app_timing` "requires application and/or driver instrumentation using Intel-PresentMon provider".
  - **Plan [inf]:** emit these from tessel with our `frame_id`. PresentMon's `MsClickToPhotonLatency`,
    `MsAllInputToPhotonLatency`, `InstrumentedLatency`, `DisplayLatency` and `PresentMode` ("Hardware: Independent
    Flip", "Composed: Flip", "Composed: Copy with GPU GDI", …) then line up with our records. The proposal's §8.2 gate
    ("never Copy with GPU GDI") becomes a scripted check.
- **macOS:** `os_signpost` intervals per phase show up in Instruments' Points of Interest [mem]. Use the Hitches /
  Animation Hitches templates for render-server-side hitches [doc: hitches article].
- **Linux:** Perfetto with ftrace (`sched`, `drm` vblank events) plus our track events [mem].

### 5.5 External validation rigs and procedures

**Rig A: microcontroller + light sensor (primary).**
- Hardware: a Teensy (USB 1000 Hz polling; thume used a Teensy LC because "its USB library uses 1000 Hz polling,
  while most USB devices default to 125 Hz" [src2]), a light sensor or photodiode in a light-tight holder on the
  panel, and the Open-Source-LDAT firmware (as farnoy used it) or thume's firmware
  (https://thume.ca/2020/05/20/making-a-latency-tester/).
- **App side [inf]:** a `latency-probe` tour/window, built only in diagnostic builds:
  - a 64×64 px patch at a fixed screen position;
  - modes `native` (key toggles the patch in the native handler), `js` (a Svelte handler toggles it), `scroll` (a
    wheel event moves a high-contrast edge under the sensor) and `viewport` (camera pan).
  - Each mode exercises one path of the proposal.
- **Procedure [inf, using lessons from thume and Marco Nett]:**
  1. Fix and record: OS build, compositor and mode (PresentMon `PresentMode`; Wayland `zero_copy` flag; macOS
     windowed or fullscreen), panel model, refresh, VRR on/off, sensor position (thume: top vs bottom of one panel
     differed by about 17 ms: 35.3 vs 52.9 ms), tessel build sha and the cadatomic pin shas.
  2. Randomise the inter-press interval (thume: a fixed 300 ms "mainly leads to incorrectly low variances"). Use
     300 ms + U(0, 50) ms or more.
  3. At least 300 samples per configuration (Marco Nett used 300 clicks per configuration,
     `gtk-qt-wayland.md` §3.5). Report median, p95, max, and the threshold or quality figure (thume's `q`).
  4. Record in-app `LatencyRecord`s in the same run. Join them to rig samples by order (or by the firmware sending
     a serial sync pulse that the app logs [inf]). Report `photon − presented` (the OS compositor + scanout + panel
     share) separately from `presented − t_os`.
  5. Repeat with the app launched by exec and by LaunchServices on macOS (one refresh of difference reported,
     `browsers-flutter-latency.md` §4.2).
- **Commercial alternative:** NVIDIA LDAT pairs a microphone (click) with a luminance sensor; the panel response is
  included [src2: TechPowerUp, igorslab]. It is gated behind an NVIDIA agreement.

**Rig B: high-speed camera (sanity check, not regression).**
- 240 fps gives 4.17 ms per frame; 960 fps gives about 1.04 ms.
- "Is It Snappy" (Chad Austin, 2017, iPhone 240 fps camera, frame scrubbing) [src2].
- Dan Luu used 240/1000 fps cameras (`browsers-flutter-latency.md` §4.2).
- Use it to cross-check Rig A once per OS and panel, and to catch cases where the light-sensor patch differs from what
  users look at (caret, glyph).

**Typometer** (Pavel Fatin, 2015) generates OS input events and screen-captures the result. It measures "processing
latency", excluding keyboard and panel [doc, pavelfatin.com/typing-with-pleasure]. Useful as a software-only cross-check
on Windows and X11. It is blind to compositor scanout.

### 5.6 CI: frames in virtual time against milliseconds on hardware

| Layer | Runs where | Asserts | Basis |
|---|---|---|---|
| **Scheduler model tests** (§6) | every `check.sh --fast` lane touching the scheduler | latency **in frames** and invariants under a virtual clock: e.g. a key at any phase φ ∈ [0, 1) of the interval presents at vblank ≤ +1 (native) and ≤ +2 (JS), given seeded cost distributions within budget; 0 wakeups when idle; ≤ 1 in flight | deterministic, seed-reproducible |
| **Headless kittest-style scenarios** with SimDisplay | `--batch` / daily | same frame counts through the real UI-thread pipeline (QuickJS, layout) with SimDisplay + lavapipe | deterministic frames; real CPU costs only reported |
| **Live app under Xvfb / lavapipe** (`scripts/xrun.sh`) | daily check | in-app histograms exported as an artifact; **report only** with wide alarms (p95 `FrameStartToCommit` > 2× baseline) | noisy; no display feedback |
| **Hardware rig** (Rig A) | manual, per release candidate and per OS, on the user's machines | median/p95 against the proposal §2 targets (p50 12–18 ms, p99 ≤ 30 ms at 120 Hz) | external truth |

The daily-check server has no GPU display path, so "real milliseconds" never gate CI [inf]. They gate releases.

---

## 6. Deterministic testing of the scheduler

### 6.1 Prior art

- **Zed `TestScheduler`** [src, `ZED/crates/scheduler/src/test_scheduler.rs`, `clock.rs`].
  - `TestScheduler::once(f)` uses seed 0. `many(n, f)` runs seeds `SEED..SEED+ITERATIONS` (env vars) and prints
    "Failing Seed: N".
  - `TestSchedulerConfig::default()`: `randomize_order: true`, `allow_parking: false`, `timeout_ticks: 1..=1000`,
    `capture_pending_traces` (env `PENDING_TRACES`).
  - `step()` first expires timers whose `expiration <= clock.now()`. It then picks a runnable at random, weighted by
    `priority.weight()`, at most one per session (one foreground task per session).
  - `block_until` polls the future, steps a random number of times, and when nothing is runnable: "In
    deterministic mode (parking forbidden), instantly jump to the next timer" (`advance_clock_to_next_timer`).
  - `advance_clock(duration)` is explicit.
  - `TestClock` starts at UTC `2025-07-01T23:59:58Z`, `advance()` moves both monotonic and UTC.
  - `end_test()` panics on recorded non-determinism.
- **GPUI test platform** [src]. `platform_window.simulate_frame_request(RequestFrameOptions { require_presentation,
  force_render, .. })` drives frames by hand in `window.rs` tests. A test sets
  `inactive_frame_interval: Some(Duration::from_secs(3600))` to observe throttling.
- **Chromium** [src]. `SchedulerTest` runs on `TestMockTimeTaskRunner`. `AdvanceFrame()` = advance the mock clock by
  one `DefaultInterval()` and send a BeginFrame. `FakeCompositorTimingHistory` sets estimates. Expectations are on
  the ordered list of client actions.
- **Chromium virtual time** [src, `CR/third_party/blink/public/devtools_protocol/domains/Emulation.pdl`,
  `HeadlessExperimental.pdl`].
  - `Emulation.setVirtualTimePolicy{policy, budget, maxVirtualTimeTaskStarvationCount, initialVirtualTime}`.
    - `advance`: "If the scheduler runs out of immediate work, the virtual time base may fast forward to allow the
      next delayed task (if any) to run".
    - `pause`; `pauseIfNetworkFetchesPending`.
    - The budget raises `virtualTimeBudgetExpired`.
    - Starvation count: "the maximum number of tasks that can be run before virtual is forced forwards to prevent
      deadlock".
  - `HeadlessExperimental.beginFrame{frameTimeTicks, interval (default about 16.666 ms), noDisplayUpdates,
    screenshot} → {hasDamage}`. It "Requires that the target was created with enabled BeginFrameControl", for use
    with `--run-all-compositor-stages-before-draw`.

### 6.2 tessel design [inf]

**Separation.** `tessel-frame` (a headless crate, no wgpu/winit, wasm-buildable like `cadatomic-core`) holds:
- `FrameClock` (§2.3), `UiFrameState` (§2.4), `Estimator` (§3.2), `InputRateTracker`, `LatencyRecorder`;
- traits `Clock { fn now(&self) -> Ticks }` and `Timers`, injected. The platform layer only translates OS events to
  `Signal`s and `Action`s to OS calls.

**Simulator (`tessel-frame::sim`)** is a discrete-event loop over a priority queue of `(Ticks, seq, Event)`, driven
by a seeded `StdRng`:

| Sim component | Models | Parameters |
|---|---|---|
| `SimDisplay` | vblanks: `phase + n·interval`; latch at `vblank − latch_offset`; modes FRR / VRR(min, max) / DRR boost; composed (+1 frame) vs zero-copy; output switch mid-run | refresh, phase, jitter σ, latch offset, compositor mode |
| `SimPresentEngine` | swapchain of depth 1–2; present → shown at the first latch after `submit + gpu`; feedback after `shown + feedback_delay` | depth, feedback delay |
| `SimGpu` | per-frame cost from a distribution (lognormal + rare spikes); queue shared across windows; `on_submitted_work_done` and timestamp results | mean, σ, spike rate/size, contention |
| `SimUi` | phase costs per frame class, JS handler costs, GC pauses, long tasks | as `SimGpu` |
| `SimInput` | Poisson or scripted discrete events at arbitrary phase; continuous streams at 125/1000/8000 Hz; OS→recv delay | rates, scripts |
| `SimTimer` | wake error: `armed + U(0, slack) + rare_late` | slack (50 µs Linux default), late probability |
| `SimVisibility` | hide/occlude/show; Wayland "no callbacks when invisible" | schedule |

**Fault injection** (each a builder flag, combinable, seeded):

| Fault | Expectation |
|---|---|
| slow JS: one 40 ms handler | compositor keeps presenting compositor-side updates every vblank (two-deadline rule); `ui_missed` counted; estimate recovers within 64 frames |
| sustained slow UI: est > interval | switch to pipelined; UI at half rate, compositor at full; back to normal after 10 frames of `1.5×` fit |
| slow GPU spike | one `comp_missed`; `m` jumps by the overshoot; decays by 90% within about 22 hit frames |
| dropped feedback (`p = 0.1`) and `discarded` | no wedge (`feedback_timeout` path); estimates unaffected beyond the missing samples |
| reordered or late feedback | out-of-order feedback ignored by `frame_id` |
| timer late by 2 ms | late tick dropped (no double draw); `wake_error` histogram reflects it |
| display off (wait returns at once) | fallback to sleep-paced ticks (GPUI rule); no busy loop (`wakeups/s` bounded) |
| output change 60 → 144 Hz mid-drag | phase reset; first frame `Immediate`; no frame presented twice |
| window hidden mid-commit | commit kept; one frame on show; `Dropped(Hidden)` for interim inputs that only had visual effect |
| two windows, 60 Hz and 144 Hz, shared GPU | each meets its own deadlines when the combined cost fits; contention counted otherwise |
| device lost | estimates reset; forced redraw after ≤ 3 failed draws |

**Invariants**, checked after every event in every seed:
1. `in_flight ≤ 1` per window; at most one draw per target vblank.
2. The UI never starts a frame while a commit is unapplied.
3. Idle with nothing dirty ⇒ no armed timers and no clock subscription (0 wakeups).
4. Every `InputSeq` reaches exactly one terminal state.
5. Presented frames are monotone in `T`; no present targets a past latch.
6. A JS commit with revision `r` never overwrites a native-owned property whose revision is greater than `r`
   (proposal §2 correctness).
7. Deadline-mode transitions only follow §2.3's table.
8. Estimates stay within `[floor, 2 × interval]`.

**Metrics as test outputs.** Each run yields the §5.3 histograms in *frames* (virtual ms are exact, so frames are
exact). Tests assert percentiles, for example `p99(InputToPresentedFrames | native key) ≤ 1.0` with costs inside
budget. They also check snapshot "frame timelines": a text log per vblank (`v=12 T=… S_ui=… D_comp=… ui=Sent
draw=Regular in_flight=1`), compared to golden files only for hand-written scripted scenarios. Seeded property runs
are never compared to goldens.

**Reproduction.** `TESSEL_SEED=<n> TESSEL_ITERATIONS=<k>`, after Zed's `SEED`/`ITERATIONS`. A failing seed prints a
timeline. `TESSEL_TRACE=<dir>` writes the Chrome JSON trace of the failing run.

**Scenario harness hook.** The live-app headless mode exposes a CDP-like
`tessel.begin_frame{frame_time, interval, no_display_updates} → {has_damage}` and a virtual-time policy
`{advance | pause, budget}`. Scenarios and tours then step frames deterministically, with SimDisplay in place of the
OS clock, without changing scheduler code (Chromium BeginFrameControl). This is not a stub for `wait-idle`. It is a
real barrier: `wait-idle` = virtual time advanced until no UI dirt, no armed animation, `in_flight = 0`
(cadatomic CLAUDE.md: "wait-idle is a real barrier").

---

## 7. Open questions (with recommendations)

1. **Who owns the vsync wait on Windows:** waitable objects (per window) or the compositor clock (one heartbeat)?
   *Recommendation:* waitables for pacing plus the compositor clock only for DRR boost and stats. Measure both with
   PresentMon on a 60 + 144 Hz two-monitor setup before choosing (Zed went back to DwmFlush).
2. **Presentation feedback through wgpu.** wgpu 30.0.1 has none. *Recommendation:* the platform layer gets it
   directly: `wp_presentation` on our own `wl_surface`, `MTLDrawable` presented handler via wgpu-hal `as_hal`, DXGI
   swapchain stats via hal. Track `VK_EXT_present_timing` for a later wgpu proposal. Needs a check of the wgpu-hal
   API surface before committing.
3. **Composition swapchain on Windows 11** (present at target time plus per-present stats) against DXGI flip. It is
   the cleanest API but takes us off wgpu's swapchain. *Recommendation:* defer to the "own platform backend" phase
   (proposal §8.1). Prototype one viewport surface first.
4. **Estimator constants** (64 samples, p90, floor 0.25 ms, margin cap `min(3 ms, interval/2)`). *Recommendation:*
   adopt them as defaults and tune with Rig A plus the simulator. Never ship a floor ≥ 1 ms (farnoy).
5. **Keep-alive presenting under VRR costs power.** *Recommendation:* only on battery-insensitive conditions or while
   input is high-rate (GPUI rule); never for animation-free idle windows.

---

## 8. Sources (all read 2026-10-25)

Chromium (`521441860f5056571081713ee25da002549bc63a`):
- `cc/scheduler/scheduler_state_machine.h`, `.cc`; `cc/scheduler/scheduler.cc`, `scheduler.h`; `cc/scheduler/scheduler_settings.h`
- `cc/metrics/compositor_timing_history.cc`, `.h`
- `components/viz/common/frame_sinks/begin_frame_args.h`, `begin_frame_source.cc`
- `components/viz/service/display/display_scheduler.cc`, `display.cc`
- `cc/scheduler/scheduler_unittest.cc`, `cc/test/scheduler_test_common.h`
- `ui/events/base_event_utils.cc`
- `third_party/blink/public/devtools_protocol/domains/Emulation.pdl`, `HeadlessExperimental.pdl`

KWin (`f41c984c…`):
- `src/core/renderjournal.cpp`, `src/core/renderloop.cpp`, `src/core/renderloop_p.h`, `src/backends/drm/drm_commit_thread.cpp`

Mutter (`2b9c0787…`):
- `clutter/clutter/clutter-frame-clock.c`, `clutter/clutter/clutter-main.c`

Android:
- https://developer.android.com/ndk/reference/group/choreographer
- https://developer.android.com/games/sdk/frame-pacing (updated 2026-02-26)
- gamesdk `044fd03c…` `games-frame-pacing/common/SwappyCommon.{h,cpp}`

Apple (developer.apple.com documentation JSON, current):
- `quartzcore/cametaldisplaylink` (+ `preferredframelatency`, `update/targettimestamp`, `update/targetpresentationtimestamp`), `cametaldisplaylinkdelegate/metaldisplaylink(_:needsupdate:)`
- `corevideo/cvdisplaylinkcreatewithactivecgdisplays(_:)`
- `metal/mtldrawable/presentedtime`, `addpresentedhandler(_:)`, `quartzcore/cametallayer/maximumdrawablecount`
- `appkit/nsview/displaylink(target:selector:)`, `appkit/nsscreen/displaylink(target:selector:)`, `appkit/nsevent/timestamp`
- `kernel/1462446-mach_absolute_time`, `quartzcore/cacurrentmediatime()`, `foundation/processinfo/systemuptime`
- `xcode/understanding-hitches-in-your-app`, `xcode/understanding-user-interface-responsiveness`
- TN2169 https://developer.apple.com/library/archive/technotes/tn2169/_index.html

Microsoft Learn:
- compositor-clock (2025-03-13), `DCompositionWaitForCompositorClock` (2025-10-31), `COMPOSITION_TARGET_STATS`
- `DXGI_FRAME_STATISTICS`, `IDXGISwapChain::GetFrameStatistics` (2024-11-20)
- composition swapchain guide (2025-03-25)
- `CreateWaitableTimerExW`, `timeBeginPeriod`, `POINTER_INFO`, `GetMessageTime` (2025-07-01)

Wayland / Linux:
- wayland.xml (`wl_surface.frame`, input `time`); wayland-protocols `b9edf665…` `input-timestamps-unstable-v1.xml`
- Linux `Documentation/input/input.rst`, `include/uapi/linux/input.h` (master)
- man7 `PR_SET_TIMERSLACK(2const)`, `time(7)`

Vulkan-Docs `a8b088a3…`:
- `proposals/VK_EXT_present_timing.adoc`, `proposals/VK_KHR_present_wait2.adoc`
- `appendices/VK_KHR_present_wait.adoc`, `VK_KHR_present_wait2.adoc`, `VK_GOOGLE_display_timing.adoc`
- Khronos blog 2025-12-04 https://www.khronos.org/blog/vk-ext-present-timing-the-journey-to-state-of-the-art-frame-pacing-in-vulkan

wgpu 30.0.1 (docs.rs): `wgpu_types::Features`, `wgpu::SurfaceTexture`, `wgpu::Queue`.

Zed `a5c4ca8b…`:
- `crates/gpui/src/window.rs`, `crates/gpui/src/platform.rs`
- `crates/gpui_windows/src/vsync.rs`, `crates/gpui_macos/src/display_link.rs`
- `crates/scheduler/src/{test_scheduler.rs, clock.rs}`

PresentMon `9db97248…`: `README-ConsoleApplication.md`, `PresentData/PresentMonTraceSession.cpp`, `PresentData/ETW/Intel_PresentMon.h`.

Perfetto: https://perfetto.dev/docs/instrumentation/track-events , https://perfetto.dev/docs/getting-started/other-formats .
Tracy manual (`master`, v0.14.1): https://github.com/wolfpld/tracy/blob/master/manual/tracy.tex .

Latency rigs and field data:
- https://thume.ca/2020/05/20/making-a-latency-tester/
- https://farnoy.dev/posts/linux-latency and https://www.phoronix.com/news/KDE-KWin-Gaming-Latency-2026 (2026-06-10)
- https://github.com/S4N-T0S/Open-Source-LDAT
- https://pavelfatin.com/typing-with-pleasure/ (2015-12-20)
- https://www.chadaustin.me/2017/04/announcing-is-it-snappy/
- https://www.techpowerup.com/review/nvidia-reflex-review-test-ldat-v2/single-page.html
- https://gpuopen.com/learn/integrating-amd-radeon-anti-lag-2-sdk-in-your-game/ (Anti-Lag 2 inserts its delay "just before the user controls … are sampled"; RDNA1+, Adrenalin 24.6.1/24.7.1) [src2 via search snippet]
