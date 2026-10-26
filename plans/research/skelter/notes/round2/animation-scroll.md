# Animation engine, springs and scroll physics for tessel (research notes, 2026-10-25)

Scope: what Svelte 5.57.2 needs from the platform for `transition:`/`in:`/`out:`, `animate:` and
`svelte/motion`; how browsers split animations between the main and compositor threads; spring
parameterisations; per-OS scroll physics; frame-rate-independent sampling. Written for tessel's
compositor thread (cadatomic `docs/research/2026-10-25-own-gui-toolkit.md` §5.3 "tick compositor
animations at the predicted present time", §9.2 "`element.animate()` runs natively on the
compositor").

Research only: nothing was built. Temporary clones were deleted afterwards.

**Grades.**
- **[doc]**: stated in a spec or vendor documentation.
- **[src]**: read in source at the pinned commit given.
- **[inf]**: inferred from the cited evidence.
- **[meas]**: computed here by simulating the cited source formula (a Python model of the JS, not
  a browser run).
- **[recalled]**: from prior knowledge, not re-fetched in this session; treat as unverified until
  checked.

## Sources and pins

| Source | Pin |
|---|---|
| Svelte | tag `svelte@5.57.2` = `10fdca7d705081cae17bf7d609d46ee6eb237d17` (`packages/svelte/package.json` says 5.57.2) |
| Chromium | `main` at `7d1956da5035c82329166aad2144a28664ec3c66` (raw.githubusercontent.com/chromium/chromium) |
| WebKit | `main` at `663727f0e17c6445295a8158c7979657d9f7f87e` |
| Firefox | `mozilla-firefox/firefox` `main` at `efd2d1e6f77570ae6bc5294731a68101e45c81a3` (`modules/libpref/init/StaticPrefList.yaml`) |
| AOSP | `platform/frameworks/base` `main` at `1cdfff555f4a21f71ccc978290e2e212e2f8b168` (`OverScroller.java`, `ViewConfiguration.java`) |
| Flutter | `master` at `66b3ad90177ccbd2963836a5556076b370622fe4` |
| GTK | `main` at `c36202f632cd179f033637f7acead89367002e90` |
| WinUI (microsoft-ui-xaml) | `main` at `f980ab26e378db4afe4264c4f00c5bf6cc11ec34` (`controls/dev/ScrollPresenter/`) |
| Wayland | `wayland.xml` from gitlab.freedesktop.org `main` (fetched 2026-10-25; wl_pointer version 11) |
| svelters | local `/home/user/svelters` at `1d6656c2c416b49f76902de1c31ef59a20eb26bd` |

Specs:
- W3C Web Animations Level 1, WD 2023-06-05: https://www.w3.org/TR/web-animations-1/
- CSS Transitions 1: https://www.w3.org/TR/css-transitions-1/
- CSS Scroll Snap 1, CSS Scroll Anchoring 1, CSS Overscroll Behavior 1 (drafts.csswg.org).

---

## 1. How Svelte 5.57.2 uses the platform

### 1.1 Transitions (`in:`, `out:`, `transition:`): `internal/client/dom/elements/transitions.js`

**The compiler emits `$.transition(flags, element, get_fn, get_params)` [src].** This registers a
`TransitionManager` `{in(), out(fn), stop(), is_global}` on the active effect's `nodes.t`
(lines 193–326). `pause_effect` (`reactivity/effects.js` 616–641) collects the managers of a paused
subtree, calls `transition.out(check)` on each, and destroys the effect when all have reported.
`resume_children` calls `transition.in()` (lines 696–735).

**`animate(element, options, counterpart, t2, on_begin, on_finish)`** (lines 338–510) [src] does
the work. `t2` is 1 for an intro and 0 for an outro.

**Deferred options.** `options` may be a function (crossfade). It is then called in a microtask
with `{direction}`, and `animate` returns a synchronous facade with `abort`, `deactivate`, `reset`
and `t` (lines 342–366).

**No duration and no delay** means `on_begin()` and `on_finish()` run synchronously, with no
animation (lines 370–380).

**Otherwise, in a microtask** (#18421: transitions created in one batch measure the DOM first,
still before paint, #14732):
1. **Intro with no counterpart.** It calls `tick(0, 1)` once if `tick` is given. If `css` is given,
   it builds `css_to_keyframe(css(0, 1))` and pushes it twice as the keyframes.
2. **The delay animation.** It calls
   `element.animate(keyframes, { duration: delay, fill: 'forwards' })`. The keyframes are `[]` for
   an outro or a reversal. This is a dummy animation that holds the first frame during the delay
   and acts as a timer, and it runs even when the delay is 0.
3. **`animation.onfinish`** does the rest:
   - `animation.cancel()` (removes the dummy), then `on_begin()` (which dispatches
     `introstart`/`outrostart`).
   - **Reversal.** `t1 = counterpart?.t() ?? 1 - t2`, then `counterpart?.abort()`.
     `delta = t2 - t1` and `duration = options.duration * |delta|`. An interrupted intro→outro
     therefore runs from the current `t`, for a proportionally shorter duration.
   - **Keyframes.** `n = ceil(duration / (1000/60))`. For `i` in `0..=n`:
     `t = t1 + delta * easing(i/n)`, and the keyframe is `css_to_keyframe(css(t, 1 - t))`.
     **The easing is baked into evenly spaced keyframes sampled at 60 Hz.** No `offset`s, no
     per-keyframe `easing`, and no effect `easing` (WAAPI's default is linear).
   - If any keyframe has `overflow: hidden`, it sets `element.style.overflow = 'hidden'` (Safari <18
     workaround).
   - **The `t` getter** becomes `t1 + delta * easing(animation.currentTime / duration)`. It reads
     `Animation.currentTime` live.
   - **With `tick`**, `loop(() => { if (animation.playState !== 'running') return false; tick(t, 1 - t); return true })`
     runs one rAF task per frame. It reads `playState` and `currentTime`.
   - It calls `animation = element.animate(keyframes, { duration, fill: 'forwards' })`, and that
     animation's `onfinish` calls `tick?.(t2, 1 - t2)` and then `on_finish()`.

**`abort()`** calls `animation.cancel()`, then `animation.effect = null` (a Chromium leak
workaround) and `animation.onfinish = noop` (#13681). `deactivate()` turns `on_finish` into a no-op.
`reset()` calls `tick(1, 0)` for an outro.

**Manager `in()`:**
- It sets `element.inert = inert` (the value it saved).
- A non-intro transition aborts the outro, and `out()` sets `element.inert = true`.
- `introend` aborts the intro (to free it) and restores `style.overflow`.
- `get_options()` caches `current_options` while a transition runs, "so reversible transitions
  reverse smoothly" (lines 220–229).

**`css_to_keyframe` (lines 51–63)** splits on `;` and `:`, trims, and camelCases the properties
(`float` → `cssFloat`, `--x` is kept). It **stops at the first malformed part** (`break`).
Keyframe values are CSS *strings*.

### 1.2 Built-in transitions (`svelte/transition/index.js`) [src]

| fn | default duration / easing | reads `getComputedStyle` | `css` emits |
|---|---|---|---|
| `fade` | 400 / linear | `opacity` | `opacity` |
| `blur` | 400 / cubicInOut | `opacity`, `filter` | `opacity`, `filter: <f> blur(<u*amount><unit>)` |
| `fly` | 400 / cubicOut | `opacity`, `transform` | `transform: <computed> translate(<x unit>, <y unit>)` (any CSS unit: px, %, rem, vh), `opacity` |
| `slide` | 400 / cubicOut | `display`, `opacity`, `height`/`width`, padding/margin/border-width on the axis | `overflow: hidden; opacity; height; padding-*; margin-*; border-*-width; min-height: 0` (**layout properties**) |
| `scale` | 400 / cubicOut | `opacity`, `transform` | `transform: <computed> scale(s)`, `opacity` |
| `draw` | 800 or `len/speed` / cubicInOut | `strokeLinecap`, `strokeWidth`; calls `node.getTotalLength()` | `stroke-dasharray`, `stroke-dashoffset` |
| `crossfade` | `sqrt(d)*30` / cubicOut | `transform`, `opacity`; calls `getBoundingClientRect()` on both nodes | `opacity`, `transform-origin: top left`, `transform: <computed> translate() scale()` |

**`transform: <computed>` is the `getComputedStyle().transform` string**, which in browsers is
serialised as `matrix(...)`/`matrix3d(...)`. A native transform parser must accept matrix
functions as well as `translate`/`scale`.

### 1.3 `animate:` (FLIP): `transitions.js` `animation()` (lines 84–180) and `animate/index.js`

**Keyed `{#each}` drives the manager `nodes.a = {measure, apply, fix, unfix}`** [src]
(`dom/blocks/each.js` 465–505, 630–662):
- **`measure`** stores `from = getBoundingClientRect()`. It runs for every non-offscreen item
  before reconciliation, and for the items being destroyed.
- **`fix`** runs on items being destroyed:
  - It returns early if `element.getAnimations().length` is non-zero.
  - Otherwise it reads `getComputedStyle(element)` `position`, `width` and `height` and pins the
    element out of flow: `style.position = 'absolute'`, the width and height set, and
    `style.transform += translate(from - to)`.
  - `unfix` restores the four inline styles.
- **`apply`** runs in a microtask after reconciliation:
  - `animation?.abort()`, then `to = getBoundingClientRect()`.
  - If any edge changed, `animate(element, get_fn()(element, {from, to}, params), undefined, 1, noop, abort)`.
  - The `animate:` directive therefore runs through the same `animate()` path as an intro with no
    counterpart. It runs the same keyframe and `onfinish` sequence.

**`flip`** (default duration `sqrt(d)*120`, cubicOut):
- It reads `getComputedStyle(node).transform` and `.transformOrigin` (px, divided by
  `node.clientWidth`/`clientHeight`).
- It reads `currentCSSZoom`, or else walks `getComputedStyle(p).zoom` up the parents.
- It emits `transform: <computed> translate(x px, y px) scale(sx, sy)`.

### 1.4 `svelte/motion` (`motion/spring.js`, `motion/tweened.js`) [src]

**Pure JS on `loop()`.** `internal/client/loop.js` keeps one `raf.tasks` set and one
`requestAnimationFrame(run_tasks)` while tasks exist. `run_tasks` uses `raf.now()`
(= `performance.now()`), *not* the rAF argument (#14541). `Tween`/`Spring` write `$state`
(`set(this.#current, …)`) once per frame. **No platform animation API is used.**

**`Tween.set`:**
- Defaults: `delay = 0`, `duration = 400`, `easing = linear`, `interpolate = get_interpolator`
  (numbers, arrays, objects, Dates; anything else snaps to `b`).
- The start time is `raf.now() + delay`. On the first frame after the start it captures `prev`,
  calls `duration(prev, value)` when `duration` is a function, and aborts the previous task.
- It returns the task's promise.

**`Spring`:** see §3.1.

`prefersReducedMotion` is `new MediaQuery('(prefers-reduced-motion: reduce)')` (`motion/index.js`)
[src].

### 1.5 The minimal platform surface Svelte 5.57.2 touches

Derived from §1.1–1.4 [inf, from the source lines above].

**`Element.animate(keyframes, options)`:**
- `keyframes`: an array of `{camelCaseProp: cssString}` objects, possibly `[]`, never with
  `offset`/`easing`/`composite`.
- `options`: `{ duration: number(ms), fill: 'forwards' }` only.

**On the returned `Animation`:**
- `onfinish` (setter; the event handler). Svelte never uses the `finished` promise, the `finish`
  event via `addEventListener`, `ready`, `pause`, `play`, `reverse`, `playbackRate`,
  `updatePlaybackRate`, `persist`, `commitStyles` or `startTime`.
- `cancel()`.
- `currentTime` (getter, ms).
- `playState` (getter; compared to `'running'`).
- `effect` (setter to `null`).

**Elsewhere on the element:**
- `Element.getAnimations()`: only `.length`.
- `getComputedStyle(el)`: `opacity`, `transform`, `transformOrigin`, `filter`, `display`,
  `position`, `width`, `height`, `zoom`, `padding*`/`margin*`/`border*Width`, `strokeLinecap`,
  `strokeWidth`.
- `getBoundingClientRect()`, `clientWidth`, `clientHeight`, `currentCSSZoom`.
- `el.style.{overflow, position, width, height, transform}` read and write.
- `el.inert`.
- `el.dispatchEvent(new CustomEvent('introstart' | 'introend' | 'outrostart' | 'outroend'))`.
- `SVGGeometryElement.getTotalLength()` (for `draw`).

**Timing semantics that must hold:**
- A **0-duration** animation (the default 0 delay) must **finish asynchronously, not
  synchronously**. Svelte sets `onfinish` after `animate()` returns. The spec queues finish
  notification steps as a microtask or task, and events go to the "pending animation event queue"
  dispatched by *update animations and send events* [doc, WA-1 §4.4.x finish notification steps].
- **`fill: 'forwards'`** must hold the last frame after finish until `cancel()`. While the delay
  dummy runs, the first keyframe must already be applied.
- **`currentTime` must advance in real time** (or the slowed time), clamped by the active
  duration, and be readable synchronously from JS at any time.
- **`cancel()` must remove the effect at once** (the next style read sees the underlying value),
  and `onfinish` must not fire afterwards.
- **One element may carry several animations at once:**
  - the delay dummy plus the main one, for a moment;
  - an intro being cancelled while an outro starts;
  - `animate:` plus an outro.

  The later one wins per property (composite `replace`, ordered by composite order, which is
  creation order for script animations) [doc, WA-1 §5.4 "effect stack"].

**Web Animations features Svelte does not use** (still worth having for hand-written JS and
devtools):
- the `finished` and `ready` promises;
- `pause`, `play`, `reverse`, `playbackRate`;
- `iterations`, `direction`, `endDelay`;
- `composite: add/accumulate`;
- the `replaceState`/`remove` event;
- `commitStyles`.

### 1.6 What svelters already does

**`packages/native-runtime/src/motion.js` (TR1/TR2) [src, svelters `1d6656c2`]:**
- `defineFacade` gives runtime nodes `animate()` → `NativeAnimation`, plus `getAnimations()`,
  `style` (a small `CSSStyleDeclaration` over the channels), `inert`, `dispatchEvent` and
  `getBoundingClientRect` (from ABI v2.8 `bounds`).
- **`parseFrame` maps keyframes onto four channels:**
  - opacity;
  - transform as `tx ty sx sy ox oy` (px translate and scale only; rotate, skew, matrix and %
    are dropped with a warning);
  - clip `w h` (only when `height`/`width` come with `overflow:hidden`; that is how `slide` is
    approximated);
  - inert.
- **`startNative`** sends `{id, duration, delay, frames}` once as JSON (`OP_ANIMATE = 11`). The
  backend reports `animationend`. Otherwise a JS player ticks once per `raf` frame.
- **A 0-duration animation** (Svelte's delay dummy) finishes through `queue_task`, which matches
  the async requirement above.
- **`finish()` cannot be done natively**: the backend cannot jump, so it cancels and holds the
  frame in JS.
- **`currentTime` is computed in JS** from `raf.now()`, so it can drift from the native clock under
  the `slow` factor.

**Aliases.** The build replaces `svelte/transition` and `svelte/animate` with native ports
(`plans/milestones/TR1-transitions.md` L287–298, `src/animate.js`). The reason: QuickJS has no
`getComputedStyle`. `computed(node)` answers a fixed style: the inline opacity, `transform: none`,
the size from `bounds`, and zero padding, margin and border. The TR1 milestone says the facade's
`getComputedStyle`-free `fix` is a no-op (L336).

**The svelters port of `flip` has a bug [src, inf].** `animate.js` sets
`sx = to.width ? to.width / to.width : 1`, which is always 1. Svelte's original is
`node.clientWidth / to.width / zoom`. This is harmless only while no transform or zoom applies.

**`crates/svelters-egui/src/anim.rs` (TR2) [src]:**
- The `Animator` keeps per-node `Running{spec, start, finished, current}`.
- Its clock is egui input time times `slow`.
- Evenly spaced keyframes are interpolated linearly per channel.
- The newest animation wins per channel. A finished animation fills forwards until cancelled.
- Under reduced motion, every animation jumps to its end.
- `running_roots` drives frame demand.
- **It runs on the UI frame.** There is no compositor thread, no predicted present time, and no
  `currentTime` read-back.

**`crates/svelters-style/src/transition.rs` (CSS transitions) [src]:**
- After each restyle, `restyled(key, before, after, now, instant)` starts a transition for every
  changed property whose `transition` entry has a positive duration or delay. It starts from the
  value **painted now** (a running transition's sample, else the old computed value).
- `view`/`sample` overlay the interpolated values, and `tick` drops finished ones.
- **Interpolated:** colours (premultiplied sRGB), `Length{px, percent}`, numbers, insets, font
  weight, shadow lists (padded with transparent zero shadows) and gradients with the same number
  of stops. Everything else is discrete. A 0s duration with a delay switches at the end of the
  delay.
- **Easing:** `TimingFunction::{CubicBezier, Steps}`, with Newton and bisection; the input `t` is
  clamped to [0, 1].
- **Gaps** against CSS Transitions 1 and Web Animations [inf]:
  - no `transform` value type, so transforms don't transition;
  - no reversing-shortening factor: a reversed transition restarts with the full duration
    (CSS Transitions 1 §3.1, "reversing-adjusted start value", "reversing shortening factor"
    [doc]);
  - no `transitionrun`/`transitionstart`/`transitionend`/`transitioncancel` events (grep found none
    in svelters crates);
  - no `linear()` easing (CSS Easing 2);
  - no exposure as `CSSTransition` objects in `getAnimations()`.

**Scrolling (`crates/svelters-egui/src/interact/scroll.rs`, `native/mod.rs`) [src]:**
- There are three `Kinetic` modes:
  - `Os`: the OS sends momentum (macOS, Windows precision touchpad).
  - `Own`: Linux. It starts on the trackpad `TouchPhase::End` if the speed is over 30 pt/s, and
    decays by `0.95` per 16 ms (`vel *= 0.95^(dt/0.016)`), stopping below 0.5 pt per frame.
  - `None`: mouse wheels.
- **Chaining:** the innermost scrollable takes the wheel first. A scrollable keeps the whole delta
  if it moved ("browsers do not split a wheel step") and passes it on at an edge when
  `chain = true`.
- **Gaps:** no wheel animation curve, no overscroll or rubber band, no snap, no anchoring.

---

## 2. A native animation engine for tessel

### 2.1 What browsers do (evidence)

**Chromium cc [doc]** (`docs/how_cc_works.md` §Animation, Chromium `7d1956da`):
- cc runs "keyframe based animations of transform lists, opacity, and filter lists which directly
  manipulate those values on the relevant TransformNode / EffectNode in the property tree
  (identified by ElementId)".
- An `Animation` has `KeyframeEffect`s, and each of those has `KeyframeModel`s, one per property.
- cc also runs its own animations: "a scroll animation for smooth scrolling".
- `LayerTreeHostImpl` calls `NeedsTickAnimations` and `TickAnimations` every frame. Ticking
  "generate[s] animation events".
- BeginMainFrame carries "a time (for animation purposes) as well as any scroll deltas that have
  been applied on the compositor thread … that Blink doesn't know about".

**cc `KeyframeModel` run states [src]** (`ui/gfx/animation/keyframe/keyframe_model.h`):
`WAITING_FOR_TARGET_AVAILABILITY, WAITING_FOR_DELETION, STARTING, RUNNING, PAUSED, PAUSED_EXCLUSIVE,
FINISHED, ABORTED, ABORTED_BUT_NEEDS_COMPLETION`. The last one is for "impl-only scroll offset
KeyframeModels that need to be completed on the main thread".

**Animation events back to the main thread [src]** (`cc/animation/animation_events.h`):
`Type { kStarted, kFinished, kAborted, kTakeOver, kTimeUpdated }`. `kTakeOver` is "for continuing a
scroll offset animation on the main thread".

**Start-time synchronisation [src]** (Blink `core/animation/pending_animations.cc`):
- Animations that start in one frame form a *compositor group*.
- Main-thread animations in that group wait for the compositor's actual start time:
  "A composited animation needs to continue waiting, otherwise the start time on the compositor and
  main-thread will be misaligned".
- `NotifyCompositorAnimationStarted(monotonic_animation_start_time, group)` then sets their start
  times.
- **Rule for tessel:** the start time is chosen by whoever ticks first (the compositor), and the
  UI side adopts it.

**Which properties go to the compositor [src]** (Blink `core/animation/compositor_animations.cc`):
- The accelerated set is `backdrop-filter, filter, opacity, rotate, scale, transform, translate`,
  plus `background-color` and `clip-path` (through paint worklets, behind flags), plus custom
  properties.
- Rejections: `kUnsupportedCSSProperty`, `kEffectHasNonReplaceCompositeMode` (non-replace
  composite on a non-neutral keyframe), `kEffectHasUnsupportedTimingParameters`,
  `kTargetHasInvalidCompositingState`, `kMixedKeyframeValueTypes`, `kTargetHasIncompatibleAnimations`
  and `kAnimationHasNoVisibleChange`.

**Firefox OMTA [doc]** (dbaron, "compositor animations", 2015,
https://dbaron.org/log/20150916-compositor-animations):
- Transform and opacity animations run on the compositor thread. Exceptions: 3-D in preserve-3d,
  SVG transforms, and a transform animation that also animates geometry.
- "we stop updating style on the main thread as the animation progresses".
- If the page flushes style (e.g. `getComputedStyle()`), "we recompute the current style on the
  main thread".
- Known bug: inherited values of an animating property are wrong in children
  (https://bugzilla.mozilla.org/show_bug.cgi?id=1039799).

**HTML and Web Animations ordering [doc]** (WA-1):
- *Update animations and send events(doc, now)* updates timelines, removes replaced animations,
  performs a microtask checkpoint, then sorts and dispatches the queued animation events.
- In the HTML "update the rendering" steps it runs **before** "run the animation frame callbacks"
  [recalled, HTML §8.1.7.3].
- `document.timeline.currentTime` "does not change between update calls" [doc].
- **Rule for tessel:** within one UI frame, every JS read of `currentTime` and every style read
  sees the same timeline time.

### 2.2 Proposed design

**Timelines.**
- **One `DocumentTimeline` per window** (a window is a document root in tessel). Its time is the
  **frame time**, frozen for the UI frame: `T_ui`.
- `T_ui` = the predicted present time of the frame being built.
  - Today it is `latch − margin − estimates` at the start of the UI frame plus the pipeline
    estimate, i.e. the compositor's `targetTimestamp` for the frame this commit will land in.
  - macOS: `CAMetalDisplayLink.Update.targetPresentationTimestamp`, "the time the system estimates
    until the display of the next frame. Update your animations based on the time difference
    between this timestamp and the previous" [doc].
  - `CADisplayLink.targetTimestamp`: "when the next frame displays" [doc].
  - Wayland: `wp_presentation` feedback plus the refresh period.
  - DXGI: frame statistics.
- The compositor samples at **`T_c`, the predicted present time of the frame it is drawing**. On a
  normal frame `T_c == T_ui`. On a compositor-only frame (the UI missed its deadline),
  `T_c = T_ui_prev + n·interval`.
- **`performance.now()` stays real time.** `document.timeline.currentTime` and the rAF argument
  are `T_ui` (a predicted present time, monotonic, possibly slightly in the future).
  - Svelte's `loop` uses `performance.now()` rather than the rAF argument (§1.4). Its `Tween` and
    `Spring` therefore lag the native clock by the pipeline depth (≈1 frame).
  - The fix is a `svelters:motion` native module, or making `raf.now()` return `T_ui` while a rAF
    callback runs [inf].
- **Slow-motion and the devtools rate** scale the timeline rate, as svelters' `slow` does. With
  reduced motion, every animation finishes at its first sample.

**Data model** (Web-Animations shaped, so JS sees standard objects):
```text
Animation { id, timeline, effect: KeyframeEffect?, start_time: Option<Time>, hold_time: Option<Time>,
            playback_rate: f64 (=1), pending: Play|Pause|None, replace_state, onfinish, oncancel,
            finished/ready promises (JS side) }
KeyframeEffect { target: NodeId (+pseudo/part), timing: EffectTiming, frames: per-property tracks }
EffectTiming { delay, end_delay, fill: none|forwards|backwards|both|auto, iteration_start, iterations,
               duration: ms|auto, direction, easing: TimingFunction }
PropertyTrack { prop: PropId, keyframes: [(offset, value: ComputedValue, easing, composite)],
                class: Compositor | Paint | Layout }
```
- Keyframe offsets left out are spaced evenly, by the WA-1 rule "compute missing keyframe offsets":
  first 0, last 1, the rest interpolated linearly between known offsets [doc].
- **Parse once.** Keyframe CSS strings are parsed and computed once, at `animate()` time, against
  the element's style (resolving `%`, `em`, `rem`, `vh` and `var()`), into `ComputedValue`s.
  Nothing is parsed per frame. Svelte's keyframe arrays are short (≤ about 60 per second of
  duration).
- **Property classes:**
  - **Compositor:** opacity, transform, translate/rotate/scale, and a non-layout clip rect (the
    rect clip tessel already has).
  - **Paint:** `filter: blur()` and the colour filters, background-color, box-shadow,
    stroke-dashoffset/dasharray, colour.
  - **Layout:** width, height, min/max, padding, margin, border-width, inset, font-size.

**Composition** (WA-1 §5.4 effect stack, replace only at first):
- For each `(node, prop)`, the underlying value is the computed style, with CSS transitions
  applied (see below).
- On top of that, apply the active script animations in **composite order**: creation order for
  `animate()`, after CSS animations and transitions [doc, WA-1 §5.4.2].
- With `replace`, the topmost effect that is in play or filling wins.
- `add`/`accumulate` come later, only for transform lists and numbers.

**Interpolation (the CSS Values 4 "Combining values" rules) [doc, recalled]:**
- **Numbers, lengths and percentages:** linear. A mix of px and % stays a calc-pair, as
  svelters-style's `Length{px, percent}` does.
- **Colours:** premultiplied in the interpolation colour space. CSS Color 4 sets **Oklab** as the
  default for `color-mix()` and gradients, but sRGB for transitions and animations of legacy
  colours. svelters' premultiplied-sRGB rule matches legacy colours.
- **Transform lists:**
  - If both lists have the same function types in the same order, interpolate per function, as is.
  - Otherwise, or when either is a matrix, decompose to the CSS Transforms 1/2 2-D or 3-D
    *decomposed matrix* (translate, scale, skew, quaternion), interpolate the parts (slerp for the
    quaternion), and recompose.
  - `none` ⇄ list uses identity functions of the other list's types.
  - Svelte's `fly`/`scale`/`flip` write `<matrix-from-getComputedStyle> translate() scale()`. Every
    keyframe has the same function list, so the fast per-function path applies after one matrix
    parse.
- **Filters:** per function with the same list, else discrete.
- **Discrete properties** flip at progress 0.5 (WA-1 "discrete animation type"). The `visibility`
  rule (visible wins during the interpolation) applies.

**Execution split.**
1. **Compositor tracks.** When an animation is created, or when a node's property is promoted,
   the UI thread copies the parsed tracks to the compositor in the next Commit (§5.4). The
   compositor writes the sampled opacity, transform and clip **into the property tree node**
   (cc's TransformNode/EffectNode model), never into the display list. It marks only that layer
   damaged, and draws. No UI work happens per frame.
   - Promotion needs: replace composite, and every keyframe of that property of one value type.
     Other conditions fall back, mirroring Blink's dispositions:
     - the property is not in the compositor set;
     - the node has no own layer and cannot get one cheaply;
     - a parent or other incompatible animation targets the same property;
     - a timeline other than the document timeline (scroll timelines can later run on the
       compositor, as in Chromium).
2. **Paint tracks.** The UI thread samples them at `T_ui` in UI phase 2 ("animate") and re-records
   that node's display list (phase 6), with no style or layout.
   - **Option:** parameterise display-list items by animated paint values (colour, blur radius,
     dash offset), so the compositor can sample them too. Chromium's paint-worklet background-color
     and clip-path animations are this idea [src: compositor_animations.cc kBackgroundColor /
     kClipPath cases].
3. **Layout tracks.** The UI thread runs style, layout and record every frame for the subtree up to
   the nearest layout boundary. This is the "opt-in" UI-side animation in §5.2 phase 2. Svelte's
   `slide` lands here; it is honest, but costs layout per frame.
   - **Recommendation:** keep svelters' clip approximation as an explicit opt-in
     `slide({native: 'clip'})`, and default to real layout so the content below moves [inf].

**Finish and read-back (the main-thread sync).**
- **The compositor owns the presentation value; the UI thread owns the model.** Both evaluate the
  same pure function `sample(track, t)`, so the UI thread never asks the compositor for values.
- `getComputedStyle()`, `getBoundingClientRect()` (transform-aware variants) and `currentTime`
  evaluate the animation **on the UI thread at `T_ui`**. This is exactly what Firefox does ("we
  recompute the current style on the main thread") [doc]. It is cheap, because the tracks are
  already parsed.
- Keeping them identical needs:
  - one `start_time` that both sides agree on (Blink's compositor group and
    `NotifyCompositorAnimationStarted`);
  - one time quantisation, the predicted present times, never the wall clock.
- **Events:**
  - The compositor reports `Started{id, start_time}`, `Finished{id}` and `Aborted{id}` (cc's
    `AnimationEvent::Type`) through the reply mailbox.
  - The UI thread queues them into the per-window *pending animation event queue*, and dispatches
    `finish`/`cancel`/`remove` plus `onfinish` in the "update animations and send events" step of
    the next UI frame, before rAF callbacks.
  - `finished` resolves in that step's microtask checkpoint.
  - **Never dispatch from the compositor's timing.** The JS-visible finish happens on the first UI
    frame whose `T_ui ≥ end_time`. That is the earliest frame in which the presented value is
    final, so `onfinish`'s `tick(t2)` and `on_finish` never run before the last frame is on screen.
  - Wake the UI thread at `end_time` even when nothing else is dirty: a timer on the compositor's
    reply. This keeps `pause_effect` destroying outro'd nodes on time.
- **Fill.** Filling animations stay on the compositor (holding the end value costs nothing) until
  `cancel()`. Svelte cancels on `introend`. Outros are destroyed with the node.

**Interruption and retargeting.**
- **Script animations:** `cancel()` is a mailbox message that removes the track. Because the UI
  model removes it at once, the next style read is right. The compositor removes it on the next
  Commit. Svelte's reversal (§1.1) builds a fresh animation from the counterpart's `t()`, which is
  computed from `currentTime` on the UI side, so it needs no read-back.
- **CSS transitions** (CSS Transitions 1 §3) start from the *current value* sampled at `T_ui`.
  Implement the **reversing shortening factor**: when the new end equals the running transition's
  start, `duration *= shortening` and `start = current`.
- **Springs (§3):** keep `(x, v)` continuous. Retargeting sets a new equilibrium only.
- **Scroll animations:** retarget à la Chromium's `ScrollOffsetAnimationCurve::UpdateTarget` (§4.3).

**How CSS transitions map onto the engine.**
- A CSS transition becomes an `Animation` with a 2-keyframe effect (`from` = the current value,
  `to` = the new computed value), `easing` = the `transition-timing-function`, `delay`, `fill:
  backwards` (CSS Transitions 2 models a `CSSTransition` with backwards fill during its delay
  [recalled]), and composite order below script animations.
- `svelters-style::Transitions` becomes the **trigger**:
  - It still diffs before and after styles.
  - Instead of storing `Running` and overlaying in `view`, it creates and cancels engine animations.
  - `Transitions::sample` disappears; paint and layout read the engine's composed values.
- CSS transitions of opacity and transform are then promoted to the compositor like any other
  animation. Today they are UI-sampled per frame.
- Expose transitions through `getAnimations()` and fire the `transition*` events.

**Frame demand.**
- The compositor requests frames while any compositor track is unfinished, or any filling track
  changed in the last commit.
- The UI thread requests frames only while paint or layout tracks run, or rAF or motion loops
  exist. This corresponds to egui's `running_roots`, per window.

---

## 3. Springs

### 3.1 Svelte's `Spring` (5.57.2, `motion/spring.js`) [src]

**Options:**
- `stiffness` default 0.15, clamped to [0, 1];
- `damping` default 0.8, clamped to [0, 1];
- `precision` default 0.01;
- `set(v, {instant, preserveMomentum: ms})`;
- `Spring.of(fn)`;
- the deprecated `spring()` store, which has `{hard, soft}`.

**Per frame** (`loop`):
```text
elapsed = min(now - last_time, 1000/30)        // clamp at 33 ms
dt = elapsed * 60 / 1000                       // in 60 Hz frames
v = (x - x_prev) / (dt || 1/60)                 // velocity from the last two samples
a = (stiffness*(target - x) - damping*v) * inv_mass
d = (v + a) * dt
settled if |d| < precision && |target - x| < precision → x = target
x_prev = x; x += d
```
- It recurses into arrays, objects and Dates.
- `preserveMomentum: ms` sets `inv_mass = 0` and then recovers it by `1000/(ms·60)` **per tick**.

**Analysis [inf + meas]:**
- At 60 Hz (`dt = 1`) the update is the linear recurrence
  `e[n+1] = (2 − c − k)·e[n] − (1 − c)·e[n−1]`, where `e = x − target`.
- **`damping` is the per-tick velocity loss**: velocity retention is `1 − c` per tick.
- **The scheme depends on the frame rate.** `d = (v + a)·dt` adds `a` to the velocity per tick
  whatever `dt` is. At 120 Hz the per-tick stiffness halves while the per-tick damping stays, so
  the damping per second doubles.
- Simulating the JS formula:

  | spring | overshoot at 60 Hz | at 120 Hz | at 144 Hz | at 240 Hz |
  |---|---|---|---|---|
  | `{stiffness: 0.1, damping: 0.25}` | 23.4 % | 9.6 % | 6.5 % | 0.7 % |
  | default `{0.15, 0.8}` | 0 | 0 | 0 | 0 |

  The default spring settles in 0.38 s at 60 Hz and 0.39 s at 120 Hz, because it is overdamped.
- **Svelte springs bounce less on ProMotion and 144 Hz displays. Their 60 Hz behaviour is the
  designed one** (it is what the docs' examples were tuned on) [inf].

**Native port (frame-rate independent, matches Svelte at exactly 60 Hz) [inf]:**
1. **Poles at 60 Hz:** `r₁,₂ = roots of r² − (2−c−k)·r + (1−c)`.
2. **Continuous equivalents:** `s = 60·ln r` (complex logarithm for complex roots).
   - Then `ω₀² = s₁·s₂` and `2ζω₀ = −(s₁+s₂)`.
   - Default `{0.15, 0.8}`: `r = 0.80, 0.25` → `s = −13.4, −83.2 /s`, i.e. `ω₀ = 33.4 rad/s`,
     `ζ = 1.45`.
   - `{0.1, 0.25}`: `r = 0.825 ± 0.263i` → `ω₀ = 20.5`, `ζ = 0.42`.
3. **Evaluate the closed form** of `ẍ = −ω₀²(x−T) − 2ζω₀ẋ` at any `t`:
   - under-, critically or over-damped cases, as in Flutter's `SpringSimulation`
     (`physics/spring_simulation.dart` `_CriticalSolution`, `_OverdampedSolution`,
     `_UnderdampedSolution` [src]);
   - initial state `(x₀, v₀)` from the current state.
   - The settle test keeps Svelte's semantics: `|x−T| < precision && |ẋ·(1/60)| < precision`.
4. **Retarget** by restarting the closed form from the current `(x, v)` with the new `T`, so
   velocity is continuous. `preserveMomentum: ms` is a ramp of the force over `ms` real
   milliseconds: `inv_mass(t) = min(1, t/ms)`. Integrate that phase numerically (RK4 at 1 ms) or
   approximate it as coasting with `v` constant, then switch on the spring.
5. **Run it** in a `svelters:motion` module: Rust owns the state, and JS reads `current` from a
   shared slot updated in UI phase 2. A spring that targets a compositor property (a
   `style:transform` bound to `spring.current`) can be handed to the compositor as a
   spring-curve track. That is an extension: the value is JS state, so it needs a
   "bound-to-property" declaration.

### 3.2 Apple's parameterisations

- **SwiftUI `Animation.spring(response: 0.5, dampingFraction: 0.825, blendDuration: 0)`** [doc,
  developer.apple.com/documentation/swiftui/animation/spring(response:dampingfraction:blendduration:)]:
  - `response` is "the stiffness of the spring, defined as an approximate duration in seconds";
  - `dampingFraction` is "the amount of drag … as a fraction of an estimate of amount needed to
    produce critical damping";
  - "each animation will be replaced by their successor, preserving velocity from one animation to
    the next".
- **SwiftUI `Spring(duration: 0.5, bounce: 0.0)`** [doc]:
  - `bounce` 0 is critically damped, (0, 1] underdamped (1 is undamped), [−1, 0) overdamped;
  - duration "is approximately equal to the settling duration, but for springs with very large
    bounce values, will be the duration of the period of oscillation".
- **`CASpringAnimation(perceptualDuration:bounce:)`** exists in QuartzCore [doc, declaration only].
- **The conversion,** mass 1. Flutter's `SpringDescription.withDurationAndBounce` mirrors it in
  source [src]:
  - `stiffness = (2π / duration)²`;
  - `ζ = 1 − bounce` for `bounce ≥ 0`, else `ζ = 1 / (1 + bounce)`;
  - `damping = 2ζ·√(m·k)`.
- **For response/dampingFraction** the same holds with `duration = response`:
  `k = (2π/response)²` and `c = 4π·ζ/response` [inf: `response` is the undamped period; the
  WWDC 2018 "Designing Fluid Interfaces" sample code is cited by third parties, but its slides
  were not re-read].
- **Critically damped retargeting (ζ = 1)** is `x(t) = T + (A + B·t)·e^{−ω₀t}`, with `A = x₀ − T`
  and `B = v₀ + ω₀·A`. Retarget by recomputing `A` and `B` from the current `(x, v)` and the new
  `T`. There is no overshoot when `v` points at `T`.
  - GTK's overshoot snap-back uses exactly this form with "friction" 20 (`ω₀ = 10 /s`)
    (`gtk/gtkkineticscrolling.c`: `f(x) = exp(-mx/2)·(c1 + c2·t)`, `OVERSHOOT_FRICTION 20`) [src].
- **Recommendation:**
  - The native API takes `{duration, bounce}`, the Apple/Flutter shape, as the canonical form.
  - It also takes Svelte's `{stiffness, damping, precision}` through the 60 Hz pole mapping in §3.1.
  - It also takes `{mass, stiffness, damping}`.
  - All three become `(ω₀, ζ)` internally.

---

## 4. Scroll physics per OS

### 4.1 Input deltas

**Windows mouse wheel:**
- `WHEEL_DELTA` is 120 per detent; high-resolution wheels send fractions [recalled, Win32
  WM_MOUSEWHEEL docs].
- Lines per detent come from `SPI_GETWHEELSCROLLLINES`, whose default is 3; `WHEEL_PAGESCROLL`
  means a page. Characters come from `SPI_GETWHEELSCROLLCHARS`.
- Chromium: `scroll_delta *= kScrollbarPixelsPerLine` with `kScrollbarPixelsPerLine = 100.0f / 3.0f`,
  so **3 lines = 100 px per detent** (`ui/events/blink/web_input_event_builders_win.cc` 303–335)
  [src].

**Windows precision touchpad:**
- Chromium attaches a DirectManipulation viewport with `TRANSLATION_X|Y|_INERTIA|RAILS_X|Y|SCALING`
  and calls `SetContact` [src, `content/browser/renderer_host/direct_manipulation_helper_win.cc`
  82–89, 264].
- It maps the viewport status `RUNNING` to scroll, and `INERTIA` to fling ("finger leave touchpad
  content still updating by inertia") (`direct_manipulation_event_handler_win.cc` 128–171) [src].
- **The OS produces the momentum.** `DM_POINTERHITTEST` arrives when "pointer input is first
  detected" [doc, learn.microsoft.com/windows/win32/inputmsg/dm-pointerhittest].
- Legacy apps get WM_MOUSEWHEEL with small deltas [recalled].

**macOS:**
- `NSEvent.hasPreciseScrollingDeltas` says whether `scrollingDeltaX/Y` are points [doc].
- `phase` covers the fingers on the pad, and **`momentumPhase` the OS-generated inertia** ("a stream
  of scroll events that dissipate over time") [doc,
  developer.apple.com/documentation/appkit/nsevent/momentumphase].
- Chromium: a continuous device uses `kCGScrollWheelEventPointDeltaAxis*` px as they come. A notched
  wheel gives `deltaX/Y × kScrollbarPixelsPerCocoaTick (40.0)`
  (`components/input/web_input_event_builders_mac.mm` 600–613;
  `ui/events/cocoa/cocoa_event_utils.h`) [src].
- **macOS accelerates wheel deltas itself.** `PointDelta` and `NSEvent.delta` include acceleration
  (the Chromium comment there) [src].

**Wayland** (wl_pointer v11) [doc, wayland.xml]:
- `axis_source`: `wheel (0)`, `finger (1)`, `continuous (2)`, `wheel_tilt (3, since 6)`.
- "If the source is … finger, a wl_pointer.axis_stop event will be sent when the user lifts the
  finger. … This enables the client to implement kinetic scrolling." For the other sources, "clients
  must not rely on receiving an axis_stop".
- `axis_value120` (since 8): each multiple of 120 is one detent; `axis_discrete` is deprecated
  since 8.
- `axis_relative_direction` (since 9) reports natural scrolling. Use it for sliders that must
  follow the fingers.
- **Accumulate everything up to `wl_pointer.frame`.**
- `axis` values are surface units, "a value of 10 per mouse wheel click by convention"
  (Chromium `ui/ozone/platform/wayland/host/wayland_pointer.cc` 188–193) [src].

**libinput:** "The source information lets a caller decide when to implement kinetic scrolling."
For finger sources the caller computes the velocity and starts kinetic motion on lift, and cancels
it "when the pointer leaves the current widget or when a key is pressed" [doc,
wayland.freedesktop.org/libinput/doc/latest/scrolling.html].

**Chromium on Linux:**
- `MouseWheelEvent::kWheelDelta = 120` on Windows, Fuchsia and Linux (53 elsewhere) (`ui/events/event.cc`
  667–679) [src]. So **1 detent = 120 px** on Linux [inf].
- Wayland fling velocity is a least-squares regression over the recent `axis` frames ("matches
  libgestures's RegressScrollVelocity()"). No fling if the last frame is older than
  `kFlingStartTimeoutMs = 200` (`wayland_event_source.cc` 126, 1093–1130) [src].

**GTK 4:**
- A wheel detent is `pow(page_size, 2.0/3.0)` px, so a 600 px view scrolls 71 px per detent
  (`get_wheel_detent_scroll_step`). Surface (touchpad) units are `× MAGIC_SCROLL_FACTOR 2.5`
  (`gtkscrolledwindow.c` 201, 1207–1229, 1394–1430) [src].
- **GTK 4 does not animate wheel steps;** it sets the adjustment directly [src].
- Kinetic velocity is computed over the last `SCROLL_CAPTURE_THRESHOLD_MS 150` of scroll history
  (`gtkeventcontrollerscroll.c` 66) [src].

### 4.2 Fling / momentum curves

**iOS `UIScrollView`:**
- `decelerationRate` defaults to `.normal` [doc].
- Its raw values are **0.998 (normal) and 0.99 (fast) per millisecond** of velocity retention.
  Flutter cites this: "Taken from UIScrollView.decelerationRate (.normal = 0.998) 0.998^1000 =
  ~0.135" (`widgets/scroll_simulation.dart` 50–53) [src; the raw value is not in Apple's text].
- **Model:** `v(t) = v₀·d^t` (t in ms) and `x(t) = x₀ + v₀·(d^t − 1)/ln d`, so the travel is
  `−v₀/ln d ≈ v₀·0.4995 s` for normal. Flutter's `FrictionSimulation(0.135, …)` is this model with
  per-second drag (`physics/friction_simulation.dart` 118–133) [src].
- The commonly quoted projection is `v₀/1000 · d/(1−d)` [recalled, WWDC 2018 session].

**Android `OverScroller`/`SplineOverScroller`** (AOSP `1cdfff55`) [src]:
- **Constants:**
  - `DECELERATION_RATE = ln 0.78 / ln 0.9` (≈ 2.358);
  - `INFLEXION = 0.35`;
  - `START_TENSION 0.5`, `END_TENSION 1.0`;
  - a 100-sample spline table;
  - `mFlingFriction = ViewConfiguration.getScrollFriction()` = `SCROLL_FRICTION 0.015`;
  - `mPhysicalCoeff = 9.80665 · 39.37 · ppi · 0.84` (`ppi = density·160`);
  - `GRAVITY 2000` for overfling.
- **Formulas:**
  - `l = ln(0.35·|v| / (friction·physCoeff))`;
  - duration `= 1000·exp(l/(DECEL−1))` ms;
  - distance `= friction·physCoeff·exp(DECEL/(DECEL−1)·l)`;
  - position along the spline table.
- **ViewConfiguration:** `MINIMUM_FLING_VELOCITY 50`, `MAXIMUM_FLING_VELOCITY 8000` (dp/s),
  `OVERFLING_DISTANCE 6`, `TOUCH_SLOP 8`, `SCROLL_BAR_FADE_DURATION 250`.
- Flutter's `ClampingScrollSimulation` ports the same spline as `1 − (1−t)^DECEL` with friction
  0.015 (`scroll_simulation.dart` 169–259) [src].

**Chromium desktop fling** (`ui/events/gestures/blink/web_gesture_curve_impl.cc`) [src]:
- **Curve choice:**
  - `kSyntheticAutoscroll` gives a fixed velocity.
  - ChromeCast and mobile use `MobileScroller` (the Android port).
  - Otherwise, when `kExperimentalFlingAnimation` is on (**enabled by default on Windows and
    Linux**, off elsewhere; `ui/base/ui_base_features.cc` 205–211), it uses `PhysicsBasedFlingCurve`.
  - The fallback is `FlingCurve`.
- **`PhysicsBasedFlingCurve`** (`physics_based_fling_curve.cc`):
  - a cubic-bezier curve from `(0.2, 1.0)` to `(0.55, 1.0)`, whose first control point is rescaled
    to match the initial slope to the release velocity;
  - duration `min(2.0 s, v/2300 px/s²)`;
  - distance from `v²/2a`, with the physical deceleration `2.7559e-5 in/ms² × ppi`;
  - distance clamped to `3 ×` the viewport (`kDefaultBoundsMultiplier`) × the boost multiplier.
- **`FlingCurve`** (the older one): `x(t) = α·e^{−γt} − β·t − α` with `α = −5707.62`, `β = 172`,
  `γ = 3.7`. The velocity is `−αγ·e^{−γt} − β`, and the curve ends where the velocity reaches 0.
- **macOS Chromium** uses the OS momentum events (see 4.1); Windows precision touchpads use the
  DirectManipulation inertia.

**Windows InteractionTracker / WinUI `ScrollPresenter`:**
- `PositionInertiaDecayRate`: "Range is from 0 to 1. … The closer to 1, the faster InteractionTracker
  will slow to a stop" [doc,
  learn.microsoft.com/uwp/api/windows.ui.composition.interactions.interactiontracker.positioninertiadecayrate].
- `ScrollPresenter` defaults: `c_scrollPresenterDefaultInertiaDecayRate = 0.95f` for position and
  0.985 for zoom (`ScrollPresenter.cpp` 31, 1113–1117) [src].
- **Reading [inf]:** 0.95 means 95 % of the velocity is lost per second, i.e. `v(t) = v₀·0.05^t`,
  with τ ≈ 0.33 s against iOS's 0.5 s.
- **Programmatic offset change** (ScrollTo with animation): duration
  `clamp(distance_px × 5 ms, 50, 1000) ms` (`ScrollPresenter.h` 69–71, `.cpp` 3243–3268) [src].

**GTK 4 kinetic** (`gtkkineticscrolling.c`, `gtkscrolledwindow.c` 181–201) [src]:
- deceleration `x = c1 + c2·e^{−4t}` (`DECELERATION_FRICTION 4`, τ = 0.25 s);
- overshoot critically damped with `OVERSHOOT_FRICTION 20`, at most `MAX_OVERSHOOT_DISTANCE 100` px;
- velocity accumulation for repeated flicks: floor 0.33, ceiling 1.0, at most ×6.

**Firefox APZ** (StaticPrefList.yaml, `efd2d1e6`) [src]:
- `apz.fling_friction 0.002` (per ms, i.e. 0.998 per ms like iOS);
- `apz.fling_min_velocity_threshold 0.5`, `apz.fling_stopped_threshold 0.01`;
- fling curve bezier `(0, 0, 1, 1)` (linear, off);
- overscroll spring `stiffness 200`, `damping 1.1`.

**svelters today:** 0.95 per 16 ms = 0.0405 per second retained, τ ≈ 0.31 s; it stops below 0.5 pt
per frame. That is close to GTK's `e^{−4t}` and to Windows' 0.95 per second [inf].

### 4.3 Wheel smooth-scroll animation

**Chromium** (`cc/animation/scroll_offset_animation_curve.cc`, `_factory.cc`) [src]:
- **Mouse wheel** uses `kInverseDelta` duration:
  - 12 frames (200 ms) for deltas ≤ 120 px, falling linearly to 6 frames (100 ms) at ≥ 480 px;
  - easing `cubic-bezier(0.42, 0, 0.58, 1)` (ease-in-out).
- **Keyboard** uses `kConstant` 9 frames = 150 ms.
- **Programmatic** (`scrollTo({behavior: 'smooth'})`) uses `kDeltaBased`: `sqrt(|Δ|)` frames, at
  most 1.5 s on desktop, with the bezier `(0.4, 0, 0, 1)` (`cc/base/features.cc`
  `PROGRAMMATIC_SCROLL_ANIMATION_CURVE(0.4,0,0,1,1500)`; Android `(0.42,0,0.58,1,700)`).
- **Retargeting** (`UpdateTarget`) keeps the velocity: the new segment's ease-in-out has its first
  control point scaled by the current slope (`EaseInOutWithInitialSlope`). The duration is the
  minimum of the segment duration and `2.5 × remaining/velocity` (`VelocityBasedDurationBound`, which
  "minimizes the 'rubber-band' bouncing").
- These animations run **on the compositor** as cc scroll-offset `KeyframeModel`s, with `kTakeOver`
  when the main thread must continue them [src, how_cc_works + animation_events.h].
- **Windows "scrolling personality"** (percent-based scrolling plus an impulse curve, contributed
  by Microsoft) was removed around Chromium 132–134 for battery regressions [doc-ish, press and
  forum reports: windowsreport.com,
  forum.vivaldi.net/topic/106460/chrome-flags-windows-scrolling-personality-gone]. The current tree
  keeps only a comment that mentions "Impulse-style".
- Line step `kPixelsPerLineStep = 40`. Page step `max(0.875·len, len − 40)`
  (`cc/input/scroll_utils.h`) [src].

**Firefox:**
- `general.smoothScroll.mouseWheel.durationMinMS 50` / `MaxMS 200`;
- lines, pages and pixels 150/150 ms;
- `currentVelocityWeighting 0.25`, `stopDecelerationWeighting 0.4`;
- the MSD (spring) physics is nightly-only (`msdPhysics.motionBeginSpringConstant 1250`,
  `regularSpringConstant 1000`) [src].

**WebKit `ScrollAnimationSmooth`:** speed 1000 px/s, at most 200 ms, ease-in-out, retargeting with
ease-out (`ScrollAnimationSmooth.cpp` 43–44, 64–106) [src].

**GTK 4:** no wheel animation. The adjustment animation (200 ms) is used for keyboard and page
jumps [src].

**macOS AppKit:** no wheel animation. OS acceleration plus momentum events give smoothness [inf
from 4.1].

### 4.4 Overscroll and rubber-banding

**macOS:**
- WebKit calls AppKit's private elasticity functions: `_NSElasticDeltaForReboundDelta`,
  `_NSHyperbolicReboundDeltaForElasticDelta` and friends (`platform/mac/ScrollingEffectsController.mm`
  49–56, 151–154, 414–426) [src].
- Constants it keeps: `scrollVelocityZeroingTimeout 100 ms`, `rubberbandDirectionLockStretchRatio 1`,
  `rubberbandMinimumRequiredDeltaBeforeStretch 10` px, `gestureAxisDecayTimeConstant 120 ms`.
- Older WebKit (macOS ≤ 10.7) had its own: `rubberbandStiffness 20`, `rubberbandAmplitude 0.31`,
  `rubberbandPeriod 1.6` [doc via search snippet of
  opensource.apple.com WebCore-7537.73.13 `ScrollElasticityController.mm`; not re-read, the page
  returned 404 on fetch].
- **iOS-style resistance curve** (community reverse-engineering, widely used):
  `f(x) = (1 − 1/(x·c/d + 1))·d` with `c = 0.55` and `d` the viewport dimension [recalled].
- **Flutter `BouncingScrollPhysics`** (its iOS emulation) [src]:
  - drag friction `0.52·(1 − overscrollFraction)²`;
  - fling-into-edge transfers at most 5000 px/s into a spring of `mass 0.5, stiffness 100, ratio 1.1`;
  - the rubber band's half-life is 0.07 s.

**Android 12+:** stretch overscroll (`EdgeEffect`). Before that, the glow; `OVERSCROLL_DISTANCE 0`
[src: ViewConfiguration].

**Windows:** no rubber band for the mouse. Touch and touchpad get a small DirectManipulation
boundary feedback [recalled].

**GTK:** at most 100 px of overshoot with the critically damped return. There is an undershoot
shadow of 40 px (`UNDERSHOOT_SIZE`) [src].

**Firefox APZ:** overscroll spring with stiffness 200 and damping 1.1 [src].

### 4.5 Anchoring, snapping, chaining

**Scroll anchoring** (CSS Scroll Anchoring 1, `overflow-anchor: auto | none`) [recalled]:
- Before layout, choose an anchor node in the scroller: the first fully or partly visible
  eligible box in DOM order.
- After layout, adjust the offset by its displacement.
- The adjustment is suppressed when changes to `top`/`position`/`transform` etc. on the anchor or
  scroller cause it, and the spec's suppression triggers apply.
- **tessel:** do it in layout (phase 4–5), natively. JS sees a `scroll` event.

**Scroll snap** (CSS Scroll Snap 1: `scroll-snap-type: x|y|both mandatory|proximity`,
`scroll-snap-align`, `scroll-snap-stop: always`, `scroll-padding`/`scroll-margin`) [recalled]:
- Chromium's snap fling (`cc/input/snap_fling_curve.cc`) models the remaining distance as a
  geometric sequence:
  - ratio 0.9 (scalar 40) or 0.92 (scalar 25), per 16 ms frame;
  - at most 5 s;
  - decay factor capped at 0.96 / 0.99 [src].
- **Prediction:** snap to the point nearest the fling's predicted end (the fling curve's final
  offset), then animate to it with the retargeting curve.

**Chaining** (CSS Overscroll Behavior 1: `overscroll-behavior: auto | contain | none`) [recalled]:
- **Latching:** browsers latch a wheel or touchpad gesture to the innermost scroller that can move
  in the gesture's initial direction, until the gesture ends. They do not chain mid-gesture.
  Chromium "calculates the scroll chain from where the touch started and finds the most inner one
  that can scroll in that direction. From that point on until the finger is released only that
  scroller will" [doc-ish, W3C public-pointer-events list 2019,
  lists.w3.org/Archives/Public/public-pointer-events/2019JulSep/0070.html].
- **Momentum and fling** go to the latched scroller; chaining to the parent happens on a new
  gesture.
- **Discrete wheel ticks** (no phase) are latched within a short timeout. Chromium used timer-based
  latching for wheels (codereview.chromium.org/2625453002) [doc-ish].
- **The gap in svelters today:** it chains per event, with no latching. A momentum stream that hits
  an edge then scrolls the outer panel, which macOS and browsers avoid [inf].

### 4.6 Recommended per-OS defaults for tessel

All curves are evaluated in closed form at the compositor's predicted present time (§5). The
`ScrollNode` offset is compositor-owned (fast commit, §5.1).

| | Windows | macOS | Linux (Wayland/X11) |
|---|---|---|---|
| Wheel detent → px | `lines(SPI_GETWHEELSCROLLLINES, 3) × 100/3 px` (Chromium); `WHEEL_PAGESCROLL` → page step; fractional WHEEL_DELTA accumulated | non-precise: `deltaY × 40 px` (Chromium/WebKit), keeping the OS acceleration | `value120/120` detents × **GTK's `page^(2/3)`** (native GTK feel, ≈60–90 px), or 3 × line height; prefer the GTK rule inside GTK-styled themes |
| Wheel animation | Chromium inverse-delta: 200 ms → 100 ms over 120–480 px, ease-in-out (0.42,0,0.58,1), velocity-preserving retarget, `2.5 × remaining/v` bound | none (apply at once; the OS accelerates) | Chromium curve as on Windows (Chromium and Firefox do this on Linux); `none` when "GTK-faithful" is chosen (GTK 4 jumps) |
| Precise touchpad deltas | DirectManipulation viewport (Chromium's configuration) or WM_POINTER; 1:1 px | `scrollingDelta` points 1:1 when `hasPreciseScrollingDeltas` | `axis` with `axis_source=finger`, accumulated per `frame`; 1:1 (`× 2.5` if matching GTK's `MAGIC_SCROLL_FACTOR`) |
| Momentum | **OS:** DirectManipulation `INERTIA` updates (no own fling) | **OS:** `momentumPhase` events (no own fling); ignore momentum events once latched elsewhere | **own** fling on `axis_stop`: velocity by least-squares over the last 150 ms (GTK window, Chromium regression); drop if the last event is older than 200 ms. Curve: Chromium `PhysicsBasedFlingCurve` (on by default on Linux), or the iOS-like exponential `v·0.998^ms` capped at a 3× viewport distance; stop below 0.5 px/frame |
| Fling, touch screen | DirectManipulation | (n/a) | Android spline (`OverScroller` constants), as Chromium mobile does |
| Overscroll | none for the mouse; touch: small bounded overshoot, critically damped return | rubber band: resistance `(1 − 1/(x·0.55/d + 1))·d`; return as a critically damped spring with ω₀ ≈ 10–20 /s (GTK 20/2, Flutter ratio 1.1); momentum into the edge transfers ≤ 5000 px/s | none by default; GTK-style ≤ 100 px overshoot with friction 20 only for touch |
| Keyboard / page | 150 ms constant ease-in-out (Chromium); page = `max(0.875·len, len − 40)` | 150 ms, or instant to match AppKit | 200 ms (GTK adjustment animation) or 150 ms |
| Programmatic smooth | `sqrt(Δ)/60 s`, ≤ 1.5 s, bezier (0.4,0,0,1) (Chromium desktop); WinUI uses `clamp(5 ms/px, 50, 1000)` | same | same |
| Latching | per gesture; wheel without phase: latch until 300 ms idle [inf, no source for the number] | per `phase` began→ended, including momentum | per `axis_stop`-terminated sequence; wheel as on Windows |
| Reduced motion | all smooth curves → instant; flings kept (they are direct manipulation) [inf] | same | same |

---

## 5. Frame-rate-independent sampling and UI/compositor agreement

1. **Sample pure functions of time, never integrate per frame.** Every curve is a function `f(t)`
   evaluated at the frame's **predicted present time**:
   - keyframes and easing;
   - the scroll animation bezier;
   - the fling formula;
   - the spring closed form;
   - the CSS transition.

   Integrating per frame is how Svelte's JS spring and svelters' `0.95^(dt/16ms)` fling work (the
   latter is at least `dt`-scaled). Pure functions make 60, 120 and VRR rates identical, and let
   the UI thread and the compositor compute the same value independently [inf].
2. **Use the present time, not the callback time.**
   - macOS: `targetPresentationTimestamp` [doc]. Its delta to the previous one is the frame step.
   - Android 13+: `Choreographer.FrameData.getPreferredFrameTimeline().getExpectedPresentationTimeNanos()`
     [recalled].
   - Wayland: `wp_presentation` `presented` times plus the refresh.
   - Windows: DXGI `GetFrameStatistics`, or the composition clock.
   - **Chromium** ticks animations at `BeginFrameArgs.frame_time`, a frame *begin* time, and gets
     away with it because its pipeline latency is constant [recalled]. tessel's §5.3 already chooses
     the latch-relative present time; use that one number for both threads.
3. **Agree on one start time per animation.**
   - The UI thread creates the animation as *pending*.
   - The first compositor frame that includes the commit picks `start = T_c(first presented frame)`
     and reports it (cc `kStarted`; Blink waits on the compositor group).
   - Until the report arrives, UI-side reads use `start = T_ui` of the creating frame. That is the
     same number if the commit lands in the predicted frame, so in practice the report only
     confirms or corrects it.
   - For non-promoted (UI-sampled) animations, `start = T_ui`.
4. **Read-back for JS** (`getComputedStyle`, `currentTime`, `getBoundingClientRect` with
   transforms) evaluates the animation on the UI thread at the current `T_ui`, frozen for the
   frame as WA-1 requires. It never synchronously asks the compositor (Firefox model) [doc].
   - A forced style flush in the middle of a task uses the same `T_ui`. Values may then lag what
     is on screen by at most the pipeline depth (one frame), which browsers accept too.
5. **Events** that matter to JS (`finish`, `transitionend`, `animationend`, scroll `scrollend`)
   are dispatched in the UI frame whose `T_ui` passes the end time, in "update animations and send
   events" order, before rAF callbacks. Compositor-only frames never dispatch.
6. **Native-owned values and JS writes** (§5.4 sequence numbers):
   - The compositor owns the scroll offset during wheel, fling and smooth animations. JS
     `scrollTop` reads the last value the compositor reported, applied at the start of each UI
     frame (BeginMainFrame "applies any compositor scroll deltas").
   - A JS `scrollTo` cancels the compositor animation (`kTakeOver` or abort), with the revision
     check.
   - An animated `transform` written by JS through `style.transform` changes the underlying value
     only. The animation stays on top (composite replace), as in browsers.
7. **Clamp time jumps.**
   - After an idle gap, a newly started animation starts at the current `T`, not at the last
     sample time: svelters' `Animator::start` resets `last_real` for this reason.
   - Svelte's spring clamps `elapsed` to 33 ms. A closed form needs no clamp, but cap a fling's
     elapsed time if the window was occluded (resume where it was) [inf].

---

## 6. Open questions (for the toolkit doc's §14)

1. **`slide` and other layout transitions:** run real layout per frame (default, correct) or keep
   svelters' clip approximation? Recommendation: real layout, with the clip as opt-in.
2. **Should the Svelte JS `Spring`/`Tween` be aliased to a native `svelters:motion`?** Doing so
   fixes the frame-rate dependence (§3.1) and the `performance.now()` lag. Recommendation: yes,
   with the 60 Hz pole mapping, so designs tuned in browsers look the same.
3. **The Linux wheel:** GTK-faithful (no animation, `page^(2/3)`) or browser-like (animated, 120
   px/detent)? Recommendation: browser-like by default (cadatomic is an IDE closer to VS Code and
   Chromium than to GTK apps), with a theme switch.
4. **Wheel latching timeout:** no primary source was found for a number. Measure it in Chromium
   (`scripts/xrun.sh`) or choose 300 ms.
