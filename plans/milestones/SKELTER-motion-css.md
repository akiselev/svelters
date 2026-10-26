# SKELTER MOTION-CSS: native transitions, FLIP, springs, CSS precompilation

Lane MOTION-CSS of skelter batch 2 (`plans/cloud/presto-b2/LANE-motion-css.md`), branch
`ci/presto-motion-css` from `ci/presto-base` e34079e6. Session `session_011YTnnfgQ149nNJtSdVuDWS`.
Design: `plans/research/skelter/compiler-and-js.md` §3.6, §3.7.

## What landed

### `crates/skelter-motion` (new code in the wave-2 skeleton crate)

| Module | What |
|---|---|
| `easing` | svelte/easing's 31 curves by name, evaluated with `skelter_anim::easing::svelte`; `to_skelter(tol)`: `Linear` exactly, other curves as CSS `linear()` points within `tol` (adaptive subdivision) |
| `transition` | `fade fly slide scale blur draw` from their JSON parameters and the computed style (`Transition::new`); `css(t, u)` as numbers (`Frame`); `Run {t1, t2, duration, delay}` with Svelte's 60 Hz keyframe `t`s; `plan(run)` → per-channel two-keyframe tracks (slide's opacity knee as a third keyframe at eased offset), timing easing = the transition's, fill forwards; `NotNative` for what skelter can't resolve (em/vw units, a computed filter, an unknown easing) so the host plays the frames |
| `flip` | Svelte's `flip` from rects + layout size + origin; **the scale bug is fixed** (`sx = clientWidth / to.width / zoom`, svelters had `to.width / to.width`) |
| `spring` | svelte/motion's spring as a motion on `skelter_anim::spring::Spring::svelte` (start velocity from Svelte's last two values): exact at 60 Hz, frame-rate independent; Svelte's settle frame; `retarget` keeps position and last step; `physical()` and `velocity()` for a compositor spring |
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

- Capabilities in lane COMPILER's shape (its message, 2026-10-25): `Spec.capabilities: Vec<String>`,
  `semantics::capability::{.., NATIVE_TRANSITIONS, NATIVE_MOTION, CSS_PRECOMPILED}` (raw strings for
  `cx1_codes`), `PlatformSemantics::capabilities()` / `has_capability()`; my copy of their hunks is a
  subset of theirs (no `spec_hash`, ids, a11y), so the consolidation keeps theirs and adds my three constants. No checked-in spec has capabilities, so every
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

- `skelter-motion/tests/svelte_fidelity.rs` against `fixtures/svelte-goldens.json`, which
  `tests/gen-svelte-goldens.mjs` writes by running svelte@5.57.2's own transition, easing, animate and
  spring code: every easing (1e-12), every built-in at Svelte's 60 Hz keyframe times (1e-6), the
  planned tracks at those times (1e-6 relative, f32 values), flip incl. the scale case, the spring
  closed form vs Svelte's integrator per frame and its settle frame.
- `skelter-motion/tests/motion.rs`: units, custom easing, slide knee, op descriptor/fallback/flip,
  WAAPI state machine, spring retarget vs Svelte, physical equivalent.
- `native-runtime/test/motion-native.test.mjs`: descriptor in the payload, custom easing, flip fix,
  `svelters:motion` Spring = svelte/motion's Spring at 60 Hz, the same values at 120 Hz, objects/instant.
- `svelters-compiler/tests/motion_native.rs`, `tests/css_precompiled.rs`; `svelters-style/tests/binary.rs`
  (precompiled vs parsed sheets give equal computed styles on a tree; interning; anon layers; hints
  and utilities round trip; garbage refused).

Run locally once to fix CI's first report (filtered): skelter-motion's tests, svelters-style `binary`, the compiler's `motion_native`/`css_precompiled`/`cx1_codes`, svelters-compiler and svelters-modules suites, native-runtime's JS suite. CI results: see the end.

## Shims and cross-lane edits

- Merged `ci/presto-base` ac7cc8c4 (wave-1 APIs): easings and the spring now use skelter-anim's.
  The computed `transform` of the built-ins is parsed with `skelter_anim::css` (`transition::parse_transform`);
  skelter-motion has no shims left.
- Cross-lane edits (all additive): `svelters-compiler` semantics/codegen/transform/compile and the
  CSS pruner (COMPILER lane works in the same crate; I told it the exact `capabilities` field text by
  message), `svelters-modules` source/mod.rs (sheet module), `svelters-style` (binary, utility),
  `svelters-abi` (one constant), `packages/native-runtime` (style.js, index.js, package.json aliases).

## Not done / follow-ups

1. **skelter-side wiring** belongs to SVELTERS-SKELTER / CORE: handle `SVL_STYLE_ANIMATE` with
   `skelter_motion::op::AnimateOp` + `waapi::Animations` (deliver `poll` as `animationend`), and
   `SVL_STYLE_SHEET_BIN` with `binary::decode(from_base64(..))` into the style engine; use the hints
   (needs a template-node → element-offset map from COMPILER's templates) and `preload_utilities`.
2. **Compositor easing** (done, coordinator's decision 2026-10-25): `skelter_base::anim::Easing::Svelte(SvelteEasing)`
   (the 31 named curves and `SVELTE_EASINGS` moved to skelter-base), evaluated by
   `skelter_anim::easing::svelte::eval`; skelter-motion re-exports the enum and plans tracks with it, so
   they are exact. skelter-compositor had no exhaustive match on `Easing`; skelter-anim's `eval_with` has
   the new arm. Tests: skelter-anim `svelte_variant_evaluates_every_named_curve`, skelter-motion
   `compositor_easings_are_exact`.
3. A spring that only feeds `style:` is not yet handed to the compositor (JS evaluates the closed form
   per frame; it is frame-rate independent already).
4. `svelte/motion` imports in `.svelte.js` modules and plain JS are not rewritten (only components);
   the loader could alias it per capability (SVELTERS-SKELTER owns the `"egui"` → capability checks).
5. `flip_native` still reads `from`/`to` through `bounds` in JS; a native measure-before/after needs a
   skelter op (snapshot rects at `measure()`), left for when skelter's layout exists.

## Questions for the coordinator

None open (the easing variant was decided yes and is done).

## CI

`3f223c27` (merged `ci/presto-base` 4d94409a): js 40227639042, linux-gtk 40227639098, linux-qt 40227639088,
macos-appkit 40227639111, windows-winui 40227639122 **green**; skelter 40227639129 red only in other lanes'
targets (`skelter-compositor --test engine`, `--test tree_damage`, `skelter-core --lib`, `skelter-widgets --lib`);
skelter-motion's tests pass there.
