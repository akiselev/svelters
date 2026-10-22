# TR2: the native animator (SV13 + SV37)

TR1 plays Svelte's keyframes from JS: one `style` op per channel per frame. TR2 sends an
animation's keyframes and timing **once** to a backend that can animate (egui), which samples them
every frame itself, reports the end, and keeps no JS in the frame loop. Related:
`plans/milestones/TR1-transitions.md` (channels, `bounds`, facade), `plans/research/CADATOMIC.md`
(SV13, SV37), `plans/research/TRACKS.md` §2.1.

## 1. What was built

| Part | Where |
| --- | --- |
| `SVL_STYLE_ANIMATE` (11), ABI v2.9, additive on the optional `style` member | `svelters-abi` |
| `Animator`: parse, sample, compose over the static visual, clock, slow-motion, reduced motion | `crates/svelters-egui/src/anim.rs` |
| `animationend` signal on `Widget` (argument: the animation id) | `kinds/builtin.rs`, regenerated `egui.json` and `egui.d.ts` |
| Op, step, event, demand | `ops.rs` (`animate_op`), `frame.rs` (`animate_step`, `animate_demand`) |
| Exiting nodes: not pickable, out of the a11y tree | `scene/hit.rs` (`exiting`), `walk.rs` (`paint_visual`) |
| JS player uses the native path when the backend answers 1 | `packages/native-runtime/src/motion.js` (`startNative`) |
| Test API: `EguiBackend::set_motion`, `motion`, `animations_running`; `TestApp::from_source_stepped` | `backend.rs`, `runner/kittest.rs` |
| Tests | `anim.rs` unit tests, `tests/animator.rs` (kittest), `fixtures/motion-egui/src/Flip.svelte` |

## 2. The protocol

`host.style(11, h, json)` with `{"id":n,"duration":ms,"delay":ms,"frames":[{"opacity":o,
"transform":[tx,ty,sx,sy,ox,oy],"clip":[w,h]},..]}` starts an animation (the frames are Svelte's
evenly spaced keyframes, already parsed by TR1's `parseFrame`); `{"id":n,"cancel":true}` cancels it.
The backend **answers 1** when it plays the animation. Any other answer or an error (GTK, Qt, the
test host: they do not know the op) makes the runtime remember "refused" per host and run TR1's JS
player. So no backend has to change.

The scene samples the keyframes as the JS player did (two adjacent frames, linear; a channel only one
frame sets holds, or switches at the middle; a free clip axis steps), composes them over the node's
static visual (what `visual_op` set; the animation wins a channel, the newest animation wins among
animations) and writes the result through `set_visual`. A finished animation holds its last frame
(`fill: forwards`) until JS cancels it, as Svelte expects.

**`outro_done`.** When an animation finishes the scene queues `animationend(id)` with
`post_event` (no JS runs in the frame) and asks for a checkpoint. The runtime listens to it itself,
outside the current scope, and calls the animation's `onfinish`, which is Svelte's own `outroend`
path (pause_effect's callback destroys the branch). `dispatch` delivers `animationend` to an inert
node (an outroing one is inert; every other event is dropped).

## 3. Decisions in the lane

- **D1 Clock: egui input time.** `ctx.input(|i| i.time)`. kittest advances it by its `step_dt` per
  frame, so it is a virtual clock; `from_source_stepped(path, 1.0/60.0)` makes an animation take the
  same number of frames everywhere. (TK1's `performance.now()` virtual clock stays JS's; the two
  agree only in real time. `currentTime`, which Svelte reads to reverse an animation, uses JS's
  clock; under kittest the two diverge, and a reversal there is approximate: accepted, coordinator
  decision 2026-10-22.)
- **D2 Gap after idle.** An animator with nothing running ignores the time that passed; the next
  step after a start only sets the baseline.
- **D3 Reduced motion** is a backend setting (`set_motion`): every animation jumps to its last
  keyframe and ends at the next frame (events and lifecycle stay; only the motion goes). TR1's JS
  player honours the same idea through `setReducedMotion(on)` (or `globalThis.__svelters_reduced_motion`),
  tested in `transitions.test.mjs`. Reading the OS preference stays with the embedder (cadatomic or
  the runner calls `set_motion`); egui 0.37 and winit expose no such preference, so the eframe
  runner does not call it (coordinator decision, 2026-10-22).
- **D4 Slow-motion** is a factor on the animation clock (`slow: 0.1` plays at a tenth of the
  speed), for debugging.
- **D5 Demand per root and viewport (SV37).** `animate_demand` asks for a frame only of the
  viewport of each root with a running animation, plus the logic viewport (which steps the
  animator). Finished animations ask for nothing, so the app goes idle.
- **D6 Exiting nodes.** The node is inert (TR1's `SVL_STYLE_INERT`): its events were already
  dropped; now `hit_test` skips it and its descendants and its AccessKit node is hidden.
  Keyboard focus (Tab) is **not** removed yet (open).
- **D7 FLIP.** `animate:flip` already read `bounds` (TR1 gave egui a synchronous `bounds`); with the
  native player its translate/scale keyframes are sent once. `tests/animator.rs` covers it.
- **D8 ABI number.** The op is 11, "v2.9". STATUS has no allocation line yet; if CS3/CS4 or LISTS
  took 11 or v2.9 first, renumber here (one constant, one doc line).

## 4. Not done

- The `performance.now()`/JS clock and the egui clock are separate (D1).
- Reduced motion is not read from the OS (see D3).
- Tab focus reaching an exiting node: fixed in TR2b (`TR2b-focus-exiting.md`).
- Implicit CSS `transition:` declarations (soft dependency on STYLE CS5e).
- SV14 (profile per-frame `tick`) is not started: `tick(t)` transitions have no keyframes and stay
  on the JS path.
- Nothing was run on GTK, Qt, AppKit or WinUI: they refuse the op and keep TR1's player.
