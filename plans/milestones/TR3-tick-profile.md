# TR3: per-frame `tick` against descriptors (SV14, in-process half)

TRACKS row TR3: profile what a JS-driven transition costs per frame next to keyframe descriptors
(`SVL_STYLE_ANIMATE`, TR2). The isolate half waits for ISOLATES. This lane builds **measurement and
a recommendation**, no new fast path: a `tick(t)` function is arbitrary JS and has no keyframes to
send.

## 1. What was added

| Part | Where |
| --- | --- |
| Three fixtures: 40 labels fading out over 400 ms, `out:fade\|global` (native), the same with the native animator off (TR1's JS player), and an `out:` transition with a `tick(t)` that writes the opacity | `fixtures/motion-egui/src/Prof{Native,Js,Tick}.svelte` |
| The profile test (kittest, virtual 1/60 s clock): per steady frame of the outro, wall time, host time in JS and in ops (`HostConfig::timing`), and ops. Asserts on op counts and on JS time ordering; prints the table (`--nocapture`) | `crates/svelters-egui/tests/tick_profile.rs` |
| `TestApp::from_source_with(path, step_dt, cfg)` | `runner/kittest.rs` |
| Debug switch `globalThis.__svelters_no_native_animation = true`: the runtime plays animations itself even on a backend that animates | `motion.js` (`startNative`) |

## 2. Numbers (one run, WSL/thinkpad class machine, **debug build**, per steady frame)

| 40 labels, 400 ms fade-out | frames | wall µs | host JS µs | host ops µs | ops |
| --- | --- | --- | --- | --- | --- |
| native keyframes (TR2) | 23 | 11179 | 0 | 0 | 0.0 |
| JS player (TR1 path) | 36 | 10721 | 1363 | 260 | 39.8 |
| `tick(t)` | 23 | 16366 | 3541 | 499 | 40.0 |

- **Native:** nothing runs in JS or crosses the ABI between the start and the end. One
  `animationend` per node at the end (40 events in total).
- **JS player:** one style op per animated node per frame, about 1.6 ms of JS and ops per frame for
  40 nodes in a debug build.
- **`tick(t)`:** the same ops plus the transition's own JS: about 4 ms per frame, 2.5 times the JS
  player, because Svelte's `loop` calls `tick` and the facade's `style` setter parses and composes
  per call.
- Wall time is dominated by egui laying out and painting 40 labels in a debug build (about 11 ms),
  so the wall column mostly shows noise between variants; the JS and ops columns are the cost the
  animation path itself adds. Release builds are roughly an order of magnitude cheaper in all
  columns; the ratios hold because they are op counts.
- The JS player took 36 frames for 400 ms against 23: the JS clock (`performance.now()` in scripted
  mode) advances less per virtual frame than egui's clock. This only affects the test clock (TR2
  open item), not real time.

**Power.** egui repaints every frame while any animation runs, in all three variants, so the frame
count is the same; the differences in energy are the JS and ops columns (CPU time per frame), which
is zero on the native path. With the native path the app wakes for frames only (no JS), and between
frames of a finished animation it is idle (TR2: `animate_demand`).

## 3. Recommendation

- Keep `css` (keyframes) as the default for the built-in transitions and for anything expressible as
  opacity, translate, scale or clip: `fade`, `fly`, `scale`, `slide`, `flip`. These cost no JS per
  frame on egui and need no change in user code.
- `tick(t)` stays on the JS path: no descriptor can stand for arbitrary JS. For the common case of a
  `tick` that only sets `opacity`/`transform`, the recommended fix is in user code (return `css`
  instead); a dev warning could say so once per transition (not done: scope).
- Do not optimise the JS player further for egui; it is only the fallback for backends without the
  op. If GTK/Qt/AppKit/WinUI get native animators, they would use the same op.

## 4. Not done

- Isolate half of SV14 (waits for ISOLATES / SV15).
- No release-build numbers and no real power measurement (RAPL or battery drain); the proxy is CPU
  time per frame.
- No dev warning for `tick`-only opacity transitions.
