// Writes fixtures/svelte-goldens.json: what svelte@5.57.2's own JS computes, for presto-motion's
// fidelity tests (tests/svelte_fidelity.rs). Run from the repo root after `pnpm install`:
//   node crates/presto-motion/tests/gen-svelte-goldens.mjs
// The DOM reads (getComputedStyle, getTotalLength, clientWidth) are stubbed with each case's values.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(here, '../../../packages/native-runtime/package.json'));
const svelte = dirname(require.resolve('svelte/package.json'));
const load = (p) => import(pathToFileURL(join(svelte, p)).href);
const transitions = await load('src/transition/index.js');
const easings = await load('src/easing/index.js');
const { flip } = await load('src/animate/index.js');

const NUM = /-?\d+(?:\.\d+)?(?:e[-+]?\d+)?/g;
const numbers = (css) => (css.match(NUM) ?? []).map(Number);

const BASE = {
  opacity: '1',
  transform: 'none',
  filter: 'none',
  display: 'block',
  width: '200px',
  height: '120px',
  paddingTop: '4px',
  paddingBottom: '6px',
  paddingLeft: '3px',
  paddingRight: '5px',
  marginTop: '2px',
  marginBottom: '8px',
  marginLeft: '1px',
  marginRight: '7px',
  borderTopWidth: '1px',
  borderBottomWidth: '2px',
  borderLeftWidth: '3px',
  borderRightWidth: '4px',
  strokeLinecap: 'butt',
  strokeWidth: '2px',
};

function node(computed, totalLength) {
  const style = { ...BASE, ...computed };
  globalThis.getComputedStyle = () => style;
  return { getTotalLength: () => totalLength ?? 0 };
}

// [kind, params (JSON-able, easing by name), computed overrides, totalLength, runs [t1, t2]]
const CASES = [
  ['fade', {}, {}, null, [[0, 1], [1, 0]]],
  ['fade', { duration: 250, easing: 'quadOut' }, { opacity: '0.8' }, null, [[0, 1], [0.37, 0]]],
  ['fly', { x: 120, y: -40 }, {}, null, [[0, 1], [1, 0]]],
  ['fly', { x: '30px', y: 10, opacity: 0.3, easing: 'elasticOut', duration: 600 }, { opacity: '0.9' }, null, [[0, 1], [0.6, 0]]],
  ['fly', { y: 25, easing: 'backInOut' }, { transform: 'matrix(1, 0, 0, 1, 10, 5)' }, null, [[0, 1]]],
  ['slide', {}, {}, null, [[0, 1], [1, 0]]],
  ['slide', { axis: 'x', easing: 'bounceOut', duration: 300 }, { opacity: '0.5' }, null, [[0, 1], [0.2, 1]]],
  ['scale', { start: 0.5 }, {}, null, [[0, 1], [1, 0]]],
  ['scale', { start: 0.2, opacity: 0.4, easing: 'expoInOut' }, { transform: 'matrix(2, 0, 0, 2, 0, 0)' }, null, [[0, 1]]],
  ['blur', {}, {}, null, [[0, 1], [1, 0]]],
  ['blur', { amount: 12, opacity: 0.25, easing: 'circInOut', duration: 500 }, { opacity: '0.7' }, null, [[0, 1]]],
  ['draw', {}, { strokeLinecap: 'round' }, 300, [[0, 1], [1, 0]]],
  ['draw', { speed: 2, easing: 'sineInOut' }, {}, 480, [[0, 1]]],
];

const transition_cases = [];
for (const [kind, params, computed, totalLength, runs] of CASES) {
  const p = { ...params };
  if (p.easing) p.easing = easings[p.easing];
  const cfg = transitions[kind](node(computed, totalLength), p);
  const easing = cfg.easing ?? easings.linear;
  for (const [t1, t2] of runs) {
    // transitions.js `animate`: duration scaled by |t2 - t1|, n = ceil(duration / (1000 / 60))
    const delta = t2 - t1;
    const duration = cfg.duration * Math.abs(delta);
    const n = Math.ceil(duration / (1000 / 60));
    const frames = [];
    for (let i = 0; i <= n; i += 1) {
      const t = t1 + delta * easing(i / n);
      frames.push({ t, numbers: numbers(cfg.css(t, 1 - t)) });
    }
    transition_cases.push({
      kind,
      params,
      computed: { ...BASE, ...computed, totalLength },
      t1,
      t2,
      duration,
      config: { delay: cfg.delay, duration: cfg.duration },
      frames,
    });
  }
}

// every easing on a 1/200 grid
const easing_samples = {};
for (const [name, fn] of Object.entries(easings)) {
  easing_samples[name] = Array.from({ length: 201 }, (_, i) => fn(i / 200));
}

// flip: from, to, clientWidth/Height, transformOrigin, transform, params
const FLIPS = [
  [{ left: 0, top: 0, width: 100, height: 40 }, { left: 30, top: 200, width: 100, height: 40 }, [100, 40], '50px 20px', 'none', {}],
  [{ left: 10, top: 20, width: 80, height: 30 }, { left: 140, top: -60, width: 160, height: 60 }, [160, 60], '80px 30px', 'none', { duration: 300, easing: 'quintOut' }],
  // a 2x scaled ancestor: the layout size is half the measured rect (svelters' old port had sx = 1)
  [{ left: 0, top: 0, width: 200, height: 80 }, { left: 0, top: 120, width: 200, height: 80 }, [100, 40], '0px 0px', 'matrix(1, 0, 0, 1, 3, 4)', { delay: 50 }],
];
const flip_cases = [];
for (const [from, to, [cw, ch], origin, transform, params] of FLIPS) {
  const p = { ...params };
  if (p.easing) p.easing = easings[p.easing];
  globalThis.getComputedStyle = () => ({ transform, transformOrigin: origin, zoom: '1' });
  const el = { clientWidth: cw, clientHeight: ch, parentElement: null };
  const cfg = flip(el, { from, to }, p);
  const easing = cfg.easing;
  const n = Math.ceil(cfg.duration / (1000 / 60));
  const frames = [];
  for (let i = 0; i <= n; i += 1) {
    const t = easing(i / n);
    frames.push({ t, numbers: numbers(cfg.css(t, 1 - t)) });
  }
  flip_cases.push({ from, to, clientWidth: cw, clientHeight: ch, origin, transform, params, delay: cfg.delay, duration: cfg.duration, frames });
}

// svelte/motion's tick_spring at exactly 60 Hz (dt = 1, inv_mass = 1), from rest, until it settles
function spring_frames({ stiffness, damping, precision }, x0, target) {
  const out = [x0];
  let last = x0;
  let cur = x0;
  for (let i = 0; i < 10000; i++) {
    const delta = target - cur;
    const velocity = (cur - last) / 1;
    const spring = stiffness * delta;
    const damper = damping * velocity;
    const acceleration = (spring - damper) * 1;
    const d = (velocity + acceleration) * 1;
    if (Math.abs(d) < precision && Math.abs(delta) < precision) {
      out.push(target);
      break;
    }
    last = cur;
    cur = cur + d;
    out.push(cur);
  }
  return out;
}
const SPRINGS = [
  [{ stiffness: 0.15, damping: 0.8, precision: 0.01 }, 0, 100],
  [{ stiffness: 0.1, damping: 0.25, precision: 0.01 }, 0, 100],
  [{ stiffness: 0.3, damping: 0.9, precision: 0.001 }, 50, -20],
  [{ stiffness: 0.05, damping: 0.6, precision: 0.01 }, 10, 400],
  [{ stiffness: 0.5, damping: 1, precision: 0.01 }, 0, 1],
];
const spring_cases = SPRINGS.map(([params, from, to]) => ({ params, from, to, frames: spring_frames(params, from, to) }));

const out = { svelte: '5.57.2', transitions: transition_cases, easings: easing_samples, flips: flip_cases, springs: spring_cases };
writeFileSync(join(here, 'fixtures/svelte-goldens.json'), `${JSON.stringify(out)}\n`);
console.log(`wrote ${transition_cases.length} transition runs, ${Object.keys(easing_samples).length} easings, ${flip_cases.length} flips, ${spring_cases.length} springs`);
