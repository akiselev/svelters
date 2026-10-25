# Lane MOTION-CSS: native transitions, FLIP, springs, CSS precompilation

Owns the compiler's `native_transitions`/`native_motion`/`css_precompiled` capabilities
(`crates/svelters-compiler/**` for these features only), `packages/native-runtime/src/motion.js` and a new
`svelters:motion` module, and the presto side in `crates/presto-core/src/motion/**` **as a new module of your
own crate `crates/presto-motion/**`** (the consolidation moves it if needed). Implement: `$n.transition_native`
for svelte/transition's fade/fly/slide/scale/blur/draw from their parameters (formulas exact; sampled at 60 Hz
equal to Svelte's JS within 1e-6); `$n.flip_native` for svelte/animate's flip (measure from presto layout; fix
svelters' scale bug `sx = to.width / to.width`); `svelters:motion` Spring/Tween with svelte/motion's API on
presto-anim's closed form; the Web Animations subset Svelte uses (`Element.animate`, `onfinish`, `cancel`,
`currentTime`, `playState`, `effect = null`, `getAnimations`) natively for presto; CSS precompilation
(binary sheets, interned utility sets, rule-candidate hints from the scoped-CSS analysis, static matches;
dev keeps the runtime path). Tests (written, not run): transition sampling equality, FLIP, spring mapping,
CSS precompiled vs runtime style results equal.
