# WG3s: scroll conformance (pins 9 and 10), the format and the macOS references

Track: WIDGETS. The scroll half of WG3's native references (`plans/milestones/WG3-native-references.md`),
requested by the user after the visual work. Decided (coordinator, 2026-10-22): macbook-gui owns the whole
format (pins 9 and 10, the actions, the scroll observable, the `Driver` methods, the comparison metrics), the
macOS capture and the egui comparison against OW2's `Scrollable` (`interact/scroll.rs`, `Policy::scroll`,
`Kinetic`). WG3s-linux (GTK, Qt) and WG3s-win (Win32, WinUI) implement the same `Driver` methods later.

## 1. The fixture

Scroll pins run on their own fixture (a pin with `setup.scroll` gets it instead of fields `a`, `b` and button
`c`):

| Anchor | What | Size (points, logical) |
|---|---|---|
| `s` | a vertical and horizontal scroll area | viewport 320 × 240; content 960 × 2400 (rows of text, 24 pt each) |
| `t` | a vertical scroll area inside `s` | viewport 280 × 120; content 280 × 600; at content y 40 of `s` (visible at offset 0) |

Content is static text (`Row 1` … `Row 100`, each row 24 pt tall) so a scroll position is visible in
screenshots. The window is 400 × 320, its content top left at a fixed screen point. `setup.scroll` gives each
area's initial offset: `{"s": [x, y], "t": [x, y]}` (points from the content's start). A driver reports each area's largest offset (`ScrollRecord::max`: content minus viewport as the toolkit lays it out; an
`NSTextView` document adds its text inset), so `overshoot` is measured against the real edge. Offsets are reported in
points, origin at the content's start, positive towards its end (down, right), whatever the toolkit's own
convention (AppKit's flipped/unflipped document views are converted).

## 2. Actions

```json
{"wheel": "s", "lines": [0, 1]}
{"wheel": "s", "lines": [0, 1], "burst": 8, "interval_ms": 20}
{"wheel": "s", "lines": [0, 1], "shift": true}
{"gesture": "s", "deltas": [[0, 4], [0, 12], ...], "interval_ms": 16, "momentum": "none"}
{"gesture": "t", "deltas": "flick-down", "momentum": "native"}
```

- **`wheel`**: a mouse wheel with notches (discrete, not continuous). `lines: [dx, dy]` notches per event,
  positive towards the content's end (scrolling down or right; the driver maps the sign to the platform's
  wheel convention). `burst: n` sends n such events `interval_ms` apart (default 1 event); the checkpoint
  follows the last. `shift` holds Shift (vertical wheel → horizontal on macOS, Windows and GTK). The pointer
  rests over the target's centre (`t`: over `t`; `s`: over a part of `s` outside `t`).
- **`gesture`**: a trackpad (precision touchpad) two-finger scroll, continuous, in points per event, with
  phases: the first delta is *began*, the rest *changed*, then *ended* with a zero delta. `deltas` is a list
  of `[dx, dy]` (positive towards the end) or a named profile (§2.1). `interval_ms` is the time between events
  (default 16). `momentum`:
  - `"none"`: the fingers stop before lifting (no momentum on any platform);
  - `"native"`: the platform's own momentum after *ended* (macOS: the OS input stack generates it, see §5;
    Windows precision touchpads: DirectManipulation; GTK/Qt on Linux: the toolkit's kinetic scrolling);
  - `"replay:<name>"`: a recorded momentum stream (`conformance/scroll/<name>.json`, §5) sent after *ended*.
- Every action is followed by a checkpoint after the scroll has **settled**: no offset change for 300 ms
  (timeout 5 s, which the capture reports).

### 2.1 Delta profiles (`conformance/scroll/profiles.json`)

| Name | Deltas (dy per 16 ms event) | What it models |
|---|---|---|
| `drag-slow` | 20 × 2 pt | a slow drag, 40 pt, ≈ 125 pt/s |
| `flick-down` | 3, 8, 16, 28, 40, 44, 40 | a flick, 179 pt, peak ≈ 2750 pt/s |
| `flick-up` | the same negated | |
| `to-end` | 60 × 40 pt | far past the content's end (overscroll) |

## 3. Observables

- **Checkpoint state.** A scroll anchor's state is `{"offset": [x, y]}` (points, rounded to 0.5), next to the
  fields' `text`/`sel`/`focus` in the same `state` line format.
- **Samples.** During an action and until it settles, the driver samples every scroll anchor's offset each
  frame (or every 8 ms): `[[t_ms, x, y], ...]` from the action's first event. Samples go to the trace
  (`target/na/<platform>/<toolkit>-<version>/<pin>.samples.json`), not to the golden (they are noisy).
- **Motion metrics** (computed from the samples of the action's target, on the action's main axis) go into the
  golden next to the checkpoint, as `"motion": {...}`:

| Metric | Meaning | Tolerance (comparison and determinism) |
|---|---|---|
| `distance` | offset change over the action, points | ± max(2 pt, 5%) |
| `per_notch` | `wheel` only: `distance / (notches × burst)` | ± max(1 pt, 5%) |
| `peak_v` | the largest offset velocity, pt/s (over 3-sample windows) | ± 60% (sampling noise, see below) |
| `tau_ms` | after the input ends: the time constant of an exponential fit `v(t) = v0·e^(−t/τ)` to the velocity (only with ≥ 6 decaying samples; `null` otherwise) | ± 60%, compared only when both runs have one |
| `settle_ms` | from the last input event to the last offset change | ± max(150 ms, 50%) |
| `overshoot` | the largest distance past the content's start or end during the action (rubber-band), points; 0 when the offset clamps | ± max(3 pt, 25%) |
| `chained` | `gesture`/`wheel` on `t`: the distance `s` moved | ± max(2 pt, 5%) |

**Positions are the strict part; timing is indicative.** The timing metrics come from sampling the views every
8 ms (or every egui frame), so a 10 pt wheel step lands in one or two samples depending on machine load: two
captures of the same NSScrollView differed by up to 2× in `peak_v` and had τ fitted in one run and not the other,
while every offset, distance and overshoot agreed within tolerance. The timing tolerances are wide on purpose;
compare curves (the `*.samples.json` traces) by eye when they matter.

A checkpoint *equals* a reference checkpoint when every offset is within ± max(2 pt, 5% of the distance moved)
and every metric is within its tolerance. Text and focus of a scroll fixture are not compared.

## 4. The `Driver` methods

```rust
fn scroll_wheel(&mut self, target: &str, lines: [i32; 2], shift: bool, burst: u32, interval_ms: u32) -> Result<(), String>;
fn scroll_gesture(&mut self, target: &str, deltas: &[[f64; 2]], interval_ms: u32, momentum: &Momentum) -> Result<(), String>;
```

Both return after the scroll has settled. `observe` adds every scroll anchor's `offset`, and the driver keeps
the action's samples (`Driver::take_samples`), from which `run_pin` computes the metrics
(`svelters_conformance::scroll::metrics`). The default implementations return `Err("unsupported")`, which the
capture records as a skipped step, so the existing GTK/Qt/Windows/egui drivers keep compiling.

## 5. macOS (NSScrollView, captured 2026-10-22 on the macbook, macOS 15.7.5, display 2×)

`crates/svelters-conformance/tests/appkit_scroll.rs` (opt-in `NA_MACOS=1`): two `NSScrollView`s (§1) in an AppKit
window. Input through the session tap with the pointer over the target: wheel *line* events (discrete), trackpad
*continuous* events with `kCGScrollWheelEventScrollPhase` began/changed/ended. **Every posted trackpad event carries
its scripted timestamp** (`CGEventSetTimestamp`, start + i × 16 ms): AppKit computes velocity from event timestamps,
and with post-time stamps a loaded machine bunched the events and the same flick went 653 pt in one run and 261 in
the next. Offsets are sampled every 8 ms until nothing moves for 300 ms. Golden:
`conformance/golden/macos/nsscrollview-15.7.5.json`; a second capture reproduced it within §3's tolerances.

**Momentum.** macOS trackpad momentum is made by the OS input stack, not by AppKit, and a synthetic gesture gets
none. `native` and `replay:<name>` both post a recorded real flick (`conformance/scroll/macos-flick.json`: one
two-finger flick by the user on the built-in trackpad, 9 gesture events over 130 ms / 601 pt, then 69 momentum events
over 1.15 s / 2507 pt; recorded with `examples/scroll_record.rs`, a listen-only event tap) after *ended*, stamped
with its own offsets.

**Findings (NSScrollView, AppKit 2575.7):**
- **Wheel: 10 pt per notch, applied at once** (settles within a frame), the same in a fast burst (no acceleration
  for synthetic line events), Shift turns it horizontal.
- **No wheel chaining.** At the inner view's end (495 pt with the text inset), further notches move nothing, not even
  the outer view.
- **Trackpad overscroll rubber-bands** (26–157 pt past the edge) and settles back in ~300–500 ms; a gesture past the
  end stops at the real end (2175 with the text view's inset).
- **Momentum:** the recorded stream moved the outer view ~720 pt (decay τ ≈ 150 ms), much less than the stream's
  own 2507 pt: AppKit does not apply momentum deltas 1:1 [cause unverified].

## 6. egui against the references

`crates/svelters-conformance/tests/egui_scroll.rs` (kittest, macOS input model, `Native::Egui`), report
`target/na/macos/egui-scroll-gap.md`. Equal checkpoints (setup excluded): 0 of 20.
- **Wheel:** egui scrolls **40 pt per notch** (AppKit 10) and animates it (smooth scroll, ~180 ms, τ ≈ 50 ms); AppKit
  jumps at once.
- **Momentum:** egui applies the replayed momentum stream 1:1 (`Kinetic::Os`): the flick went 2122 pt to the end of
  the content where AppKit went ~720.
- **Overscroll:** egui clamps (no rubber band, overshoot 0).
- **[driver gap, not egui behaviour]** In the egui fixture the inner area `t` never receives the input: every action
  aimed at `t` scrolled `s` (`chained: 2160` on a gesture over `t`), so the chaining rows and the nested rows say
  nothing about egui yet. Likely the fixture's `scroll-area` sizes/positions (the `width`/`height` props or the box
  column) or the hit point; to fix before these rows are read.

Reading (numbers, not a decision): the macOS-relevant policy gaps are the wheel step (40 vs 10 pt, animated vs
immediate), momentum scaling, and the missing rubber band. They are `Policy::scroll` / Scrollable choices (OW2), not
the egui core.

## Not done / follow-ups

- The egui fixture's `t` (above); then a `Native::Mac` scroll row for OW2.
- WG3s-linux (GTK, Qt) and WG3s-win (Win32, WinUI): implement `Driver::scroll_wheel` / `scroll_gesture` (§4).
- The momentum scaling AppKit applies to an injected stream vs a real one (a real flick through a live NSScrollView
  with the event tap recording both would settle it).

## Handoff (2026-10-22, wind-down)

**Captured fixtures (checked in):**
- `conformance/pins/9-wheel.json`, `9-wheel-chaining.json`, `10-gesture.json`, `10-overscroll.json`: the scroll pins.
- `conformance/scroll/macos-flick.json`: one real trackpad flick with its OS momentum (the user's swipe, take 3).
- `conformance/golden/macos/nsscrollview-15.7.5.json`: NSScrollView, reproduced within tolerance by a second capture.

**Compared:** egui `scroll-area` (macOS input model) against the NSScrollView golden (`egui_scroll.rs`, a report, not
gated): 0 of 20 checkpoints equal (wheel 40 vs 10 pt and animated, momentum 1:1, no rubber band).

**Next, in order:**
1. Fix the egui fixture's inner area `t` (input aimed at it reaches `s`), then re-read the chaining/nested rows.
2. A `Native::Mac` scroll row in `Policy::scroll` (OW2): 10 pt wheel step, immediate; momentum scaling; rubber band.
3. WG3s-linux / WG3s-win drivers (§4).
4. Re-capture the macOS golden on a quiet machine before relying on the timing metrics.

**Running it:** `tools/dev/cargo-slot cargo test -p svelters-conformance --test appkit_scroll --no-run`, then
`NA_MACOS=1 [NA_UPDATE=1] [NA_PINS=wheel,…] target/build/debug/deps/appkit_scroll-<hash> --nocapture` on a Mac
nobody is using (it moves the pointer). Recording a new flick: `target/debug/examples/scroll_record <out.json> [wait_s]`
(needs Input Monitoring), then one two-finger flick.
