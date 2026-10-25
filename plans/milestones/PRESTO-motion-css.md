# PRESTO MOTION-CSS: native transitions, FLIP, springs, CSS precompilation

Lane MOTION-CSS of presto batch 2 (`plans/cloud/presto-b2/LANE-motion-css.md`), branch
`ci/presto-motion-css` from `ci/presto-base` e34079e6. Session `session_011YTnnfgQ149nNJtSdVuDWS`.
Design: `plans/research/presto/compiler-and-js.md` §3.6, §3.7.

## What landed

### `crates/presto-motion` (new code in the wave-2 skeleton crate)

| Module | What |
|---|---|
| `easing` | svelte/easing's 31 curves in `f64`, same operations as the JS; `to_presto(tol)`: `Linear` exactly, other curves as CSS `linear()` points within `tol` (adaptive subdivision) |
| `transition` | `fade fly slide scale blur draw` from their JSON parameters and the computed style (`Transition::new`); `css(t, u)` as numbers (`Frame`); `Run {t1, t2, duration, delay}` with Svelte's 60 Hz keyframe `t`s; `plan(run)` → per-channel two-keyframe tracks (slide's opacity knee as a third keyframe at eased offset), timing easing = the transition's, fill forwards; `NotNative` for what presto can't resolve (em/vw units, a computed filter, an unknown easing) so the host plays the frames |
| `flip` | Svelte's `flip` from rects + layout size + origin; **the scale bug is fixed** (`sx = clientWidth / to.width / zoom`, svelters had `to.width / to.width`) |
| `shim::spring` | svelte/motion's spring in closed form: the 60 Hz recurrence `e[n+1] = (2-k-c) e[n] - (1-c) e[n-1]` solved (complex, real, repeated, damping = 1 cases), evaluated at `n = 60 s`; exact at 60 Hz, frame-rate independent; Svelte's settle frame; `retarget` keeps position and last step; `physical()` gives the continuous (stiffness, damping) and `velocity()` the matching start velocity |
| `shim::css` | the computed `transform` parser the built-ins need (matrix, translate, scale, rotate, skew) |
| `waapi` | the Web Animations slice Svelte uses: `Animation` (play/pause/cancel/finish, `currentTime` get/set, `playState`, `effect = null`, fill forwards, progress), `Animations<N>` per document (`of_node` = `getAnimations`, `poll` raises each finish once → `animationend`, `next_finish` for the clock) |
| `op` | the `OP_ANIMATE` payload: channel frames plus the optional `native` descriptor; `plan()` prefers the descriptor, falls back to the frames |

### JS (`packages/native-runtime`)

- `$n.transition_native(flags, node, kind, getParams)` and `$n.flip_native(node, getParams)`: the same
  built-ins as `$n.transition` / `$n.animation`; their `css` is wrapped (`motion.js describeNative`) so
  the next `animate()` sends `native: { kind, params, computed, t1, t2[, flip] }` in the OP_ANIMATE
  payload next to the frames. A custom easing (not svelte/easing's, by identity) sends no descriptor.
- `transition.js`: `draw` added (path length from `node.getTotalLength()` when the node has it);
  `nativeTransition(kind)`. `animate.js`: `flip` fixed (layout size from the computed width/height,
  origin from `transformOrigin`), `flipNative`.
- `svelters:motion` (`src/svelters-motion.js`, also `@svelters/native-runtime/motion`): `Spring` in
  closed form (same math as the Rust shim; arrays, objects, Dates; `instant`; `preserveMomentum` runs
  Svelte's integrator since the inverse-mass ramp has no closed form), `Tween`/`tweened`/`spring` are
  Svelte's own (aliases `svelte/internal/motion/{spring,tweened}`), `prefersReducedMotion.current`.
- Aliases added: `svelte/easing` (so app code and the runtime share one module: easing names by
  identity), `svelters:motion`. `tools/check-svelte-internals.mjs` §7 guards the motion internals.
- `style.js`: `registerStyles(id, { bin })` → `SVL_STYLE_SHEET_BIN` (12) on an engine target.

### Compiler

- `Spec.capabilities` (`#[serde(default)]` set) + `Spec::has_capability`, `PlatformSemantics::has_capability`
  (default false; SpecBackend and Overlay answer). No checked-in spec has capabilities, so every
  existing backend's output is unchanged (conformance and op-log goldens untouched).
- `native_transitions`: a `transition:`/`in:`/`out:` whose name is an import of fade/fly/slide/scale/
  blur/draw from `svelte/transition` (renamed imports too) → `$n.transition_native(flags, n, "fade", ...)`;
  `animate:` of svelte/animate's `flip` → `$n.flip_native(n, ...)`. Anything else keeps the JS path.
- `native_motion`: `import ... from 'svelte/motion'` in a component's scripts → `"svelters:motion"`.
- `css_precompiled` (release only, `scoped_css` on): the pruner keeps (complex selector, element)
  matches and entirely-global selectors (`SelectorMatches` on `CssOutput`, Svelte's output itself
  unchanged); `scoped_css::precompiled` parses the scoped code with the engine, maps each engine
  rule to its Svelte rule through the sheet's source map, and writes `svelters_style::binary` bytes:
  per element the candidate rules and the `always` ones (compound type/class/id selectors over its
  static tag, classes + scope class and id; never rules under `@media`), global rules (unmapped or
  `:global`), and the resolved utility rules of static classes (when utilities are on).
  `ScopedCss::precompiled` / `precompiled_base64`. The loader (`svelters-modules`) emits
  `registerStyles(id, { bin })` for it. Dev keeps the text path.

### svelters-style

- `binary`: `encode`/`decode` of `Precompiled { sheet, hints, global_rules, utilities }`; interned
  declaration blocks (shared `Arc`); selectors and media re-parsed from text on load; anonymous layers
  renamed fresh per load; `compound_matches`; base64 helpers.
- `Stylist::preload_utilities` (resolved utility rules without the vocabulary lookup; `ensure_utilities`
  now shares `insert_utilities` with it).

### ABI

- `svelters-abi`: `SVL_STYLE_SHEET_BIN = 12`; `SVL_STYLE_ANIMATE`'s doc names the optional `native` field.

## Tests (written; CI runs them)

- `presto-motion/tests/svelte_fidelity.rs` against `fixtures/svelte-goldens.json`, which
  `tests/gen-svelte-goldens.mjs` writes by running svelte@5.57.2's own transition, easing, animate and
  spring code: every easing (1e-12), every built-in at Svelte's 60 Hz keyframe times (1e-6), the
  planned tracks at those times (1e-6 relative, f32 values), flip incl. the scale case, the spring
  closed form vs Svelte's integrator per frame and its settle frame.
- `presto-motion/tests/motion.rs`: units, custom easing, slide knee, op descriptor/fallback/flip,
  WAAPI state machine, spring retarget vs Svelte, physical equivalent.
- `native-runtime/test/motion-native.test.mjs`: descriptor in the payload, custom easing, flip fix,
  `svelters:motion` Spring = svelte/motion's Spring at 60 Hz, the same values at 120 Hz, objects/instant.
- `svelters-compiler/tests/motion_native.rs`, `tests/css_precompiled.rs`; `svelters-style/tests/binary.rs`
  (precompiled vs parsed sheets give equal computed styles on a tree; interning; anon layers; hints
  and utilities round trip; garbage refused).

Not run locally: any of the above (cargo check only, per the batch rule). CI results: see the end.

## Shims and cross-lane edits

- `presto-motion::shim::spring` stands in for `presto_anim::spring`'s Svelte mapping and
  `shim::css` for `presto_anim::css` (both FRAME-ANIM's; empty on `ci/presto-base`). The consolidation
  should keep one: the 60 Hz derivation and the golden test transfer as they are.
- Cross-lane edits (all additive): `svelters-compiler` semantics/codegen/transform/compile and the
  CSS pruner (COMPILER lane works in the same crate; I told it the exact `capabilities` field text by
  message), `svelters-modules` source/mod.rs (sheet module), `svelters-style` (binary, utility),
  `svelters-abi` (one constant), `packages/native-runtime` (style.js, index.js, package.json aliases).

## Not done / follow-ups

1. **presto-side wiring** belongs to SVELTERS-PRESTO / CORE: handle `SVL_STYLE_ANIMATE` with
   `presto_motion::op::AnimateOp` + `waapi::Animations` (deliver `poll` as `animationend`), and
   `SVL_STYLE_SHEET_BIN` with `binary::decode(from_base64(..))` into the style engine; use the hints
   (needs a template-node → element-offset map from COMPILER's templates) and `preload_utilities`.
2. **Compositor easing**: presto-base's `Easing` has no svelte/easing curves, so tracks carry `linear()`
   points within 1e-4. Question below.
3. A spring that only feeds `style:` is not yet handed to the compositor (JS evaluates the closed form
   per frame; it is frame-rate independent already).
4. `svelte/motion` imports in `.svelte.js` modules and plain JS are not rewritten (only components);
   the loader could alias it per capability (SVELTERS-PRESTO owns the `"egui"` → capability checks).
5. `flip_native` still reads `from`/`to` through `bounds` in JS; a native measure-before/after needs a
   presto op (snapshot rects at `measure()`), left for when presto's layout exists.

## Questions for the coordinator

1. **Add `Easing::Svelte(name)` (or `Easing::Fn(u8)`) to `presto-base::anim::Easing`**, evaluated by
   `presto_anim::easing::eval` with `presto_motion::easing::SvelteEasing::eval`'s formulas?
   Recommendation: yes, additive (FRAME-ANIM adds one match arm); then `to_presto` returns it and
   compositor tracks are exact instead of within 1e-4.
2. **Keep the closed-form spring in presto-anim or presto-motion?** Recommendation: presto-anim
   (`spring::svelte`), moving `shim::spring` there at consolidation with its golden test.
