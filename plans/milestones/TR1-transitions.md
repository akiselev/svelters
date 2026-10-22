# TR1: transitions and animations on native elements

`transition:`, `in:`, `out:` and `animate:` on native elements. Before this lane they were compile
errors (`unsupported_feature: transitions`, `unsupported_feature: animate: directives`), and the
`transition` category of `experiments/native-compiler/conformance/expected.json` listed 33 samples.

This file is the design, written before the implementation, and then the record of what was built
(section 9 onwards). The design answers the five questions of the lane brief. Each answer gives the
choice and the reason for it.

Related: `docs/notes/svelte-feature-survey.md` §8 (the old "forbidden" rationale and the native
alternatives), `plans/architecture/backend-abi.md` (ABI v2.1 frame clock, v2.3 styling ops),
`plans/research/EGUI.md` §1.2 (egui frames), `docs/notes/native-compiler/runtime-contract.md`.

## 0. Summary

| Question | Choice |
| --- | --- |
| 1. Core | Svelte's own `transition()` / `animation()` and `pause_effect` / `resume_effect`, called by our blocks. The runtime node implements the small slice of `Element` they use (`animate`, `inert`, `style`, `dispatchEvent`, `getBoundingClientRect`). Our blocks keep outroing branches, as Svelte's `BranchManager` and each block do. |
| 2. Clock | The existing ABI v2.1 frame clock (`request_frame` / `host->frame`, JS `requestAnimationFrame`), reached through Svelte's own `raf` / `loop`. No ABI change. A frame is requested only while an animation runs. |
| 3. `css(t)` | A hybrid. Svelte's keyframes go through a small WAAPI-style player in JS. Each frame's declarations map onto four **visual channels** (opacity, transform, clip, inert), which every backend implements natively through `host.style` ops. Unknown properties are dropped, with one warning per property. `svelte/transition` and `svelte/animate` resolve to native ports with the same API and CSS. |
| 4. `animate:` | Needs geometry. ABI v2.8 adds an optional `bounds` member: the layout rect in toplevel coordinates, or the natural size when the widget has not been laid out. FLIP and crossfade use it, and so does `slide`'s height. Without it, `animate:` is a no-op. |
| 5. Driving | JS calls one style op per changed channel per element per frame (about 1.5 µs of host and ABI time each). Native interpolation from keyframes is not needed yet. The ABI leaves room for it (section 5). |

ABI: **v2.8, additive.** Style ops `SVL_STYLE_TRANSFORM` (8), `SVL_STYLE_CLIP` (9) and
`SVL_STYLE_INERT` (10) are new codes on the existing optional `style` member. One optional backend
member, `bounds`, is appended last.

## 1. The core: reuse Svelte's transition runtime

### 1.1 What Svelte does

- **Registration.** `$.transition(flags, element, get_fn, get_params)`
  (`internal/client/dom/elements/transitions.js`) registers a `TransitionManager` (`in`, `out(fn)`,
  `stop`) on the running branch effect (`effect.nodes.t`). If the transition is an intro and
  `should_intro` is set, it starts the intro in an effect, but only when it is `|global` or when
  its block has already run (`REACTION_RAN`). Transparent blocks (else-if, snippet, dynamic
  component, boundary) are skipped while looking for that block.
- **Pausing.** A block that removes a branch calls `pause_effect(effect, done)`
  (`reactivity/effects.js`). This marks the subtree `INERT` and collects the managers: local ones
  only up to the next non-transparent block, global ones everywhere. It calls `out(check)` on each
  and destroys the effect once all have finished. With no transitions, all of this happens
  synchronously.
- **Reversal.** If the branch comes back while it is outroing, `resume_effect` calls `in()`. The
  intro starts from the outro's current `t`, through `counterpart.t()`. Blocks know which branches
  are outroing: `BranchManager` keeps `#onscreen` and `#outroing` per key, and the each block keeps
  `outrogroups`.
- **Animation.** `animate()` builds keyframes by sampling `css(t)` every 16.7 ms of the duration.
  It plays them with `element.animate(keyframes, { duration, fill: 'forwards' })`. A dummy
  animation as long as `delay` runs first, so `introstart` fires only after the delay. `tick(t)`
  runs in a `loop()` task on Svelte's `raf`. Events go out through
  `element.dispatchEvent(new CustomEvent('introstart'))`. While outroing, `element.inert = true`.

### 1.2 Choice: call Svelte's code, give it a native `Element`

The native runtime already reuses Svelte's effects, scheduler and boundaries through one adapter,
`src/core/svelte.js`, and `tools/check-svelte-internals.mjs` asserts what it takes from Svelte. Its
blocks (`if_block`, `each_block`, `key_block`, `snippet`, `component`, `await_block`) are its own,
but they are built from Svelte's `block` and `branch` effects. So Svelte's `transition()`,
`animation()`, `pause_effect` and `resume_effect` work unchanged on our effect tree, given three
things:

1. **`effect.nodes`.** Our branches get `{ start: null, end: null, a: null, t: null }`. With
   `end: null`, `destroy_effect` never touches DOM nodes.
2. **An `Element` facade on the runtime node** (`Native`, the object `bind:this` and attachments
   already get). It provides:
   - `animate(keyframes, { duration, fill })`: a native player (section 3);
   - `getAnimations()`;
   - `inert`: an accessor, section 1.4;
   - `style`: a small `CSSStyleDeclaration`. Svelte reads and writes `style.overflow`, and the
     `animate:` fix path reads `position`/`width`/`height`/`transform`. Writes go through the same
     visual channels as `css(t)`, so a `tick(t)` written for the DOM (`node.style.opacity = t`)
     also works natively;
   - `dispatchEvent(event)`: runs the node's runtime handlers for `event.type`;
   - `getBoundingClientRect()`: `host.bounds`, section 4.
3. **Block integration.** Our blocks call `pause_effect` instead of destroying a branch. They keep
   the outroing branch in place, and resume it instead of building a new one when its key comes
   back. Section 1.3 has the details.

This keeps Svelte's semantics without porting them: the local/global rules, `|global` across
transparent blocks, reversal from the current `t`, deferred transitions (crossfade returns a
function), `delay`, event order, effect tracking turned off inside the transition function
(`without_reactive_context`), and the #15604 rule that a nested outroing branch is not resurrected
when an ancestor is resumed. The adapter grows by six names, all asserted by
`check-svelte-internals.mjs`:

- `transition` and `animation` (exported by `svelte/internal/client`);
- `pause_effect` and `resume_effect` (`reactivity/effects.js`, already a hidden module);
- `set_should_intro` (`render.js`, a new hidden module);
- the flag bits `INERT`, `EFFECT_TRANSPARENT` and `REACTION_RAN`.

**Rejected: a native re-implementation of the transition manager.** It would be about 500 lines
duplicating Svelte's `transitions.js`, and Svelte's edge cases (#14732, #15604, #18421, #13681) would
have to be tracked by hand. The facade is about 150 lines, and Svelte's own code keeps tracking
upstream fixes.

**`CustomEvent`.** QuickJS has no `CustomEvent`. Svelte's `dispatch_event` calls
`new CustomEvent(type)`, so the runtime installs a minimal one (`type`, `detail`) only when it is
missing. The DOM conformance host keeps jsdom's.

### 1.3 Blocks

| Block | Change |
| --- | --- |
| `if_block` | `BranchManager` semantics. The state is a map of key (`true`/`false`) to branch, plus an `outroing` set. When the key changes: if the key's branch is still on screen (outroing), resume it; otherwise build a new branch after the current ones. Then pause every other branch that is not already outroing. The pause callback disposes the natives and drops the branch. A new optional fifth argument, `elseif`, makes the block transparent (`EFFECT_TRANSPARENT`). The compiler passes it for `{:else if}`. |
| `key_block` | The same, keyed by the key value (SameValueZero), so a key that flips back during the outro resumes its branch, as Svelte's `BranchManager` does. |
| `snippet`, `component` (dynamic `<svelte:component>`, snippet props) | Same as `key_block`, keyed by the function. The block is transparent. |
| `boundary` | The block is transparent. |
| `each_block` | Removed items are paused, not closed. They stay in the region list in place, marked outroing, and are disposed when their outro group finishes. Svelte destroys a group together. A key that comes back while outroing resumes its item. Unkeyed blocks do the same by index. The fallback branch is paused and resumed the same way. |
| `await_block` | Branch switches go through the same swap with pause. Svelte's `await` also uses `BranchManager`. |

**Insertion anchors.** An outroing region keeps its natives where they are, but is never used as
the `before` of a new insertion: `firstNative` skips outroing regions. Live natives therefore always
keep their relative order. A new branch lands after the outroing one, exactly where Svelte puts it
(before the block's end anchor). Keyed moves only reorder live items. Disposal still walks outroing
regions, so an ancestor that is torn down removes their natives too.

**Destroy while outroing.** When an ancestor is destroyed, `destroy_effect` calls `stop()` on the
managers and aborts the animation, so the pause callback never fires. The natives go with the
ancestor's scope, because item scopes are children of the block's owner scope.

### 1.4 Intro on mount, `|local`/`|global`, inert and events

- **`mount(App, { intro })`.** `intro` defaults to `true`, as in Svelte. The runtime sets Svelte's
  `should_intro` around the root component's init (`set_should_intro`), as Svelte's `_mount` does.
  On the first render, only `|global` intros play, because the enclosing block has not run yet.
  This is Svelte's rule.
- **`|local` (default) / `|global`.** These are Svelte's rules, unchanged, because our blocks are
  Svelte block effects with the same transparent flags.
- **`inert` during an outro.** It has two parts:
  1. **Runtime.** `__svelters_dispatch` and `__svelters_dispatch_sync` drop events whose node, or an
     ancestor of it in the native tree, is inert. A sync signal then gets the toolkit default. This
     is portable and needs nothing from the backend.
  2. **Backend.** `SVL_STYLE_INERT` asks the toolkit to stop input and focus:
     - GTK: `can-target` false and `focusable` false;
     - Qt: `Qt::WA_TransparentForMouseEvents` and `Qt::NoFocus`, restored afterwards;
     - DOM host: `el.inert`.

     A backend without the op only gets the runtime part.
- **Events.** `introstart`, `introend`, `outrostart` and `outroend` are runtime events. The
  compiler accepts `on<them>={fn}` on every native element (they are in no backend spec) and emits
  `$n.on(node, 'introstart', fn)`. The runtime registers these without `host.listen`, and the
  facade's `dispatchEvent` runs them. Handlers get the event object (`{ type }`), as in Svelte.

## 2. The animation clock

**Choice: the ABI v2.1 frame clock, unchanged.**

Every backend already has `request_frame`, which answers with one `host->frame` on the toolkit's
next frame:

- GTK: `add_tick_callback` on a mapped toplevel;
- Qt: `QWindow::requestUpdate`, with the frame filter catching `UpdateRequest`;
- AppKit: a `CADisplayLink`/`CVDisplayLink`, unpaused per request, with a watchdog timer;
- WinUI: `CompositionTarget.Rendering`;
- egui: `request_repaint`, plus a frame flag read in the next pass.

Each falls back to a 16 ms timer when there is no window to tick on. The host prelude implements
`requestAnimationFrame` on top of it, and `performance.now()` from the host clock.

Svelte's client build (`esm-env` `BROWSER`, which our bundles get through the `browser` condition)
schedules with `raf.tick = requestAnimationFrame` and `raf.now = performance.now`, and runs its
`loop()` tasks there. The native player registers one `loop` task per running animation. So:

- **Idle CPU stays at zero.** `loop` requests a frame only while it has tasks. When the last
  animation ends, `run_tasks` stops asking, and the backend stops ticking, because each request
  gives exactly one frame. This is tested on the log backend: no `request_frame` once the last
  animation has finished.
- **The rejected option, a host-side timer,** would bypass vsync and keep a timer source alive.

The conformance DOM host drives the same `raf` object, as Svelte's test helper
`animation-helpers.js` does: it patches `raf.tick` and `raf.now`, and `raf.tick(ms)` advances time.

## 3. What `css(t)` means natively

### 3.1 Options

1. **Parse each frame's declarations and map them to native setters.** This is generic and works
   with user `css(t)` functions. It needs a mapping table and a parser, but Svelte's own
   `css_to_keyframe` already splits the declarations.
2. **Per-backend native equivalents of each built-in** (GTK `GtkRevealer`, Qt `QPropertyAnimation`,
   ...). These are smooth and cheap, but they cover only the built-ins, give different semantics per
   backend, and lose Svelte's `t`/reversal/delay model.
3. **A hybrid.** Svelte computes the keyframes. A JS player interpolates them and maps the
   declarations onto a small set of **visual channels**. Each backend implements the channels
   natively, at compositor level where it can.

**Choice: 3.** The CSS stays the contract: user `css(t)` works, and the CSS research lane can later
own the declaration-to-native mapping. The per-frame native work is the cheapest primitive each
toolkit has.

### 3.2 Channels

| Channel | CSS it comes from | Style op | Payload |
| --- | --- | --- | --- |
| opacity | `opacity` | `SVL_STYLE_OPACITY` (4, exists since v2.3) | `"0".."1"`; `NULL` restores 1 |
| transform | `transform` (`translate[X/Y]`, `scale[X/Y]`, `none`); `transform-origin` | `SVL_STYLE_TRANSFORM` (8, new) | `"tx ty sx sy ox oy"`: px, px, factor, factor, origin as fractions of the size (default `0.5 0.5`); `NULL` = identity |
| clip | `height`/`width` with `overflow: hidden` (what `slide` emits) | `SVL_STYLE_CLIP` (9, new) | `"w h"` in px, `-1` = that axis unclipped; `NULL` = none. The element takes this much layout space and its content is cut at it. |
| inert | the facade's `inert` | `SVL_STYLE_INERT` (10, new) | `"1"` or `NULL` |

Other declarations are handled as follows:

- `overflow` is consumed by the clip mapping.
- `min-height`/`min-width: 0`, `padding-*`, `margin-*` and `border-*-width` are ignored. They are
  what `slide` emits, and the native ports emit 0 for them.
- `filter: blur()` is the reserved blur channel (section 3.4).
- Any other property is dropped, with **one dev warning per property name** (decision D2).

Values are parsed once per keyframe when the animation starts, not per frame. Each frame
interpolates numbers between the two adjacent keyframes, as WAAPI does: Svelte's keyframes are
evenly spaced and come from an already eased `t`. A channel is written only when its value changes
(`node.vis`), so a fade sends one op per frame.

**Composition with ST1 styling.** An inline `style="opacity: .5"` already goes through
`SVL_STYLE_OPACITY`. The channel layer keeps the base value from the style runtime: the animated
value wins while the animation runs, and cancel writes back the base value, or `NULL`. Transform,
clip and inert have no static source yet. When the CSS lane brings a static `transform`, it becomes
that channel's base value through the same op.

**Scope after the CSS deferral (2026-10-21).** The user has deferred the CSS engine
(`svelters-style`) and all of CSS.md's decisions (D1–D10). This lane therefore builds only on what
is in the tree today:

- the per-element style sink (`host.style` ops);
- the existing property setters;
- the ABI v2.1 frame clock.

The animated-value path is deliberately small and self-contained: one module, `src/motion.js`, with
four channels and no cascade, selectors or general CSS parsing beyond the declarations a transition
emits. A style engine could take it over later as one unit. The paragraph below records how it
would fit.

**Compatibility with `plans/research/CSS.md`** (branch `research/css`; deferred, see above). The
animated values are not a second style path:

- **The animated values are inline declarations.** An animation frame is a set of inline
  declarations at the top of the cascade. It goes through the same per-element inline sink that ST1
  uses for `style="opacity: …"` today: `host.style(OPACITY, h, v)`.
- **The new ops extend that sink.** `TRANSFORM` and `CLIP` extend the sink for the properties a
  per-frame value cannot go through rule classes for. CSS.md measures a new deduped GTK class as a
  global provider reload and a Qt sheet as a re-polish.
- **Setter backends write the setter's property.** On AppKit and WinUI, the backend's `style`
  member (CSS.md gives them one, answering `SVL_STYLE_TARGET_NONE`) writes these ops to the same
  native property their style table targets: `opacity` is `alphaValue` and `Opacity`, and
  `transform` is the layer transform and `Translation`/`Scale`. On egui, the scene node writes
  them.
- **The runtime stays toolkit-agnostic.** It only knows CSS declarations and channels.
- **With the CS1 engine,** `applyVisual` becomes "set the node's animated declarations" in the
  host's `StyleTree`. The engine then pushes them through whichever sink it uses for computed
  values, and the CSS lane's `computed(h)` replaces this lane's `computed(node)` for the built-ins.
- **With CS6 (engine tweens for CSS `transition:`),** CS6 can reuse this lane's player and
  channels. The reverse does not hold: Svelte's directives need Svelte's `t` model and events,
  which CSS transitions do not have.

D7 of CSS.md is about CSS `transition:` properties, not Svelte's directives. This lane's channels
do give Qt, AppKit and egui the primitive that D7 says they lack.

**The `style` facade.** `node.style.opacity = t`, `node.style.transform = '...'` and
`setProperty`/`removeProperty`/`cssText` write through the same mapping. A read returns the last
written value.

**Rejected: `SVL_STYLE_INLINE` (CSS text) per frame.** GTK dedupes inline styles into append-only
classes, so a new string every frame would grow the provider without bound. Qt reparses and
re-polishes the widget's stylesheet. Both are far costlier than a property write.

### 3.3 Per-backend cost

| | opacity | translate / scale | size clip | blur |
| --- | --- | --- | --- | --- |
| **GTK 4** | `gtk_widget_set_opacity`: a GSK opacity node, the cheapest possible (exists) | a per-widget `GtkCssProvider` holding `transform:` (GTK 4 CSS supports `transform`, `transform-origin` and `filter`): about 10 µs to parse, then a restyle and reallocate of that widget only | none below the widget's minimum: no per-widget clip exists without a wrapper. Approximated with a negative CSS margin (collapses the layout space) plus `scaleY` with origin top (squashes the content into it); see D3 | CSS `filter: blur()` on the same provider (GSK blur node; GL only, the cairo renderer ignores it) |
| **Qt 6 Widgets** | `QGraphicsOpacityEffect` (exists): the widget is rendered into a pixmap each frame (moderate) | a custom `QGraphicsEffect` that also draws the source pixmap through a `QTransform`: the same cost class as opacity (one effect per widget, so opacity and transform share it) | `maximumHeight`/`maximumWidth` (a layout pass per frame; the classic Qt collapse; content is squeezed or cut) | `QGraphicsBlurEffect` cannot be combined with the opacity effect; possible inside the same custom effect (pixmap blur). Deferred |
| **AppKit** | `alphaValue` (layer-backed: compositor, cheap) | `wantsLayer` + `layer.affineTransform` (compositor; AppKit may reset it on layout, so it is re-applied per frame) | `NSLayoutConstraint` on height + `clipsToBounds` (layout pass) | `layer.filters` (`CIGaussianBlur`; needs `layerUsesCoreImageFilters`) |
| **WinUI 3** | `UIElement.Opacity` (compositor) | `UIElement.Translation` / `Scale` / `CenterPoint` (compositor, cheap) | `Height` + `Clip` (layout pass) | Composition effect brush (heavy, Win2D) |
| **egui** | multiply the node subtree's opacity (`Ui::multiply_opacity`); egui repaints anyway while animating | a transformed layer (`Area` + `transform_layer_shapes`): moderate, needs a layer per animated node | `max-height`/`max-width` (taffy relayout, cheap) | none |

**Minimal common set.**
- **opacity** is the floor on all five, so `fade` is guaranteed everywhere.
- **transform** (translate and scale): GTK, Qt, AppKit and WinUI. egui follows later. This gives
  `fly`, `scale` and FLIP.
- **clip**: Qt (exact) and GTK (approximated), for `slide`. egui follows later.
- **blur**: reserved (op 11 is not assigned yet). `blur` degrades to its opacity part.
- **`draw`** is SVG-only and is not provided. Importing it is a build error, because the native
  module does not export it.

### 3.4 `svelte/transition` and `svelte/animate`

Svelte's built-ins call `getComputedStyle(node)` and `getBoundingClientRect()`, which QuickJS lacks
and which would be wrong for a native node anyway. The build aliases `svelte/transition` and
`svelte/animate` to native ports, as it already aliases `svelte` to the native `mount` facade:
`@svelters/native-runtime/transition` and `/animate`. They have Svelte's signatures, defaults and
CSS text, but read `computed(node)`:

- opacity: the node's base opacity (inline style), else 1;
- transform: `none`;
- size: `bounds(h)`.

`crossfade` and `flip` use `bounds` too. `svelte/easing` and `svelte/motion` are pure and stay
Svelte's. The conformance hooks apply the same aliases. These modules are copies of Svelte's
(MIT, attributed in `THIRD_PARTY_NOTICES.md`), with only the computed-style reads changed.

## 4. `animate:` (FLIP) and geometry

Svelte's each block calls `nodes.a.measure()` on every live item before reconciling, then
`apply()` in a microtask afterwards. `apply` reads `getBoundingClientRect()` again, and, if the rect
moved, plays `fn(node, { from, to }, params)` (`flip`: a translate and scale from the old rect).
Outroing items of an animated list are `fix()`ed: taken out of the flow with `position: absolute`.

**ABI: nothing exposes geometry today.** GTK has `compute_bounds` only as a GI method with an
out-struct, which X1 rejects. Qt's `geometry` is parent-relative. So:

```c
/* ABI v2.8, backend, appended last. NULL = no geometry (animate: and crossfade are no-ops).
   out = x, y, width, height in logical px, relative to h's toplevel. A widget that has not been
   laid out yet (just created or inserted) reports x = y = 0 and its natural size, which is what
   `slide` needs for an intro. 0 = ok, < 0 = error (last_error). */
int (*bounds)(void *self, svl_handle h, double out[4]);
```

JS gets `host.bounds(h)`, which returns `[x, y, w, h]` or `null`.

- **GTK:** `gtk_widget_compute_bounds(w, root)`, or `measure()` when the widget is not allocated.
- **Qt:** `mapTo(window(), QPoint())` plus `size()` after `QLayout::activate()` on the toplevel,
  which makes the layout synchronous, or `sizeHint()` when the widget is not laid out.
- **DOM host:** `getBoundingClientRect()`.
- **AppKit, WinUI, egui:** follow-ups. The member stays NULL.

**Layout timing (FLIP).** Qt can lay out synchronously, so `apply` sees the new rect in the same
turn. GTK allocates in the frame clock's layout phase, so `bounds` right after a move still returns
the old rect. The runtime therefore runs `apply` for GTK from the next `requestAnimationFrame`
instead of a microtask, when `host.bounds` reports a stale layout. The moved widget then shows at
its new place for one frame before the transform pulls it back. This is a known one-frame jump,
reported in the record. egui would behave the same.

**`fix()` is not done natively.** A native widget cannot leave its parent's layout. Outroing items
of an animated list keep their place until they are removed, and the live items animate around them
(decision D4). The facade's `getComputedStyle`-free `fix` is a no-op.

## 5. Driving: JS setters per frame, not native interpolation

**Estimate, from the R1 host-batching measurement.**
- 11,000 ops cost 16–18 ms of host and ABI time, about 1.5 µs per op, names already cached.
- The backend side of an opacity write is about 1 µs (GTK `set_opacity`, a property store and a
  queued redraw).
- A transform write on GTK costs about 10–20 µs (CSS parse and restyle).

A fade of 50 elements at 60 fps is 3,000 ops/s, about 7 ms/s (0.7 % of one core), plus JS
interpolation (tens of µs per frame). A fly of 50 elements is 6,000 ops/s, about 50–100 ms/s on GTK
(5–10 %), dominated by the CSS restyle, not the ABI. The ABI is not the bottleneck: batching or
native interpolation would save the 1.5 µs crossings, but not the toolkit's restyle.

**Rejected for now: native keyframe interpolation** (`host.animate(h, keyframes, duration)`, with
the backend ticking itself). It would save JS time per frame, but:

- it needs per-backend timelines, cancellation and `t()` read-back for reversal;
- it has to be kept in sync with Svelte's semantics;
- it still cannot serve `tick(t)` transitions, which run JS every frame by definition.

The channel ops are the primitive such an op would be built from, so it stays possible later, as
an optional `SVL_STYLE_*` timeline op. It is a follow-up if a profile shows JS interpolation cost.

**Measured** in this lane: per-frame cost through the QuickJS host on the log backend and on GTK
(section 11).

## 6. Compiler

- **`transition:`, `in:` and `out:`** become an `Op::Transition` placed with the bindings and
  actions, after `bind:this`, as Svelte's `after_update`:

  ```js
  $n.transition(flags, node, () => fn, () => params)
  ```

  The flags are Svelte's `TRANSITION_IN` (1), `TRANSITION_OUT` (2) and `TRANSITION_GLOBAL` (4).
  Modifiers other than `local`/`global` are `transition_modifier_invalid`. The name is a directive
  name like `use:` (`a.b.c-d` member access).
- **`animate:`** becomes `$n.animation(node, () => fn, () => params)`. Svelte's validation applies:
  `animation_invalid_placement` (only as the sole child element of a keyed each) and
  `animation_duplicate`.
- **`on{intro,outro}{start,end}`** on native elements: `$n.on(node, 'introstart', fn)`, with no spec
  lookup.
- **`{:else if}`** passes `elseif = true` to `$n.if_block`.

`$n.transition` and `$n.animation` live in `@svelters/native-runtime` (`src/transition.js`). They
ensure `effect.nodes`, then call Svelte's `$.transition` / `$.animation`.

## 7. Tests

- **Runtime unit tests** (`packages/native-runtime/test/transitions.test.mjs`, TestHost with a
  virtual `raf`):
  - intro and outro op sequences;
  - deferral (natives stay inserted until `outroend`);
  - reversal (resume, no new natives);
  - `|global` vs local;
  - each-item outro groups and resumption;
  - `inert` drops events;
  - `css` and `tick`;
  - no frame requested when idle.
- **Op-log fixture** `experiments/native-compiler/fixtures/transitions`:
  - `style #h opacity` and `style #h transform` lines;
  - played on the log backend;
  - replayed on GTK and Qt (`oplog_replay`).
- **GTK and Qt Rust headless tests**:
  - the new style ops (opacity, transform, clip, inert);
  - `bounds`;
  - idle after the last animation;
  - a QuickJS host run of a fade and a fly with real frames.
- **Conformance:** frame clock and channels in the DOM host. Remaining samples are reclassified with
  reasons.
- **Gallery page** "Motion" in `examples/`: fade, fly, scale, slide, a `tick` transition, crossfade
  and a keyed list with `animate:flip`.

## 8. Decisions for the user

- **D1 Guarantees per backend.**
  - Recommended: `fade` on all five.
  - `fly`/`scale` on GTK, Qt, AppKit and WinUI.
  - `slide` exact on Qt and approximated on GTK.
  - Everything else degrades to its opacity part, with a dev warning.
- **D2 Unsupported CSS in a custom `css(t)`.**
  - Recommended: drop it with one dev warning per property (prod: silent). A `css` function cannot
    be checked at compile time.
  - Alternative: a runtime error, which would make one bad property break a whole transition.
- **D3 GTK `slide`.**
  - Recommended: the squash approximation now, and a native clip wrapper (a GtkRevealer-like bin
    the backend interposes) as a follow-up.
  - Alternative: no clip on GTK (opacity only).
- **D4 `animate:` outroing items.**
  - Recommended: they keep their place (no `fix()`) until removed.
  - Alternative: hide them at once (`opacity 0`) so the FLIP looks cleaner.
