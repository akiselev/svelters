// MOTION-CSS: `$n.transition_native` / `flipNative` send their native descriptor in the OP_ANIMATE
// payload; `svelters:motion`'s Spring equals Svelte's at 60 Hz and keeps Svelte's 60 Hz values at
// other frame rates. Own host (the runtime remembers per host whether OP_ANIMATE was refused); the
// virtual clock is native-animator.test.mjs's.
import assert from 'node:assert/strict';
import test from 'node:test';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const $n = await import('../src/index.js');
const { raf } = await import('../src/core/internal.js');
const { flip, flipNative } = await import('../src/animate.js');
const motion = await import('../src/svelters-motion.js');
const svelteMotion = await import('svelte/internal/motion/spring');
const easing = await import('svelte/easing');

let now = 0;
let pending = [];
raf.now = () => now;
raf.tick = (fn) => pending.push(fn);
function frames(n, step = 16) {
  for (let i = 0; i < n; i++) {
    now += step;
    const due = pending;
    pending = [];
    for (const fn of due) fn(now);
  }
}
const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');
const created = (log) => [...log.matchAll(/create label #(\d+)/g)].map((m) => +m[1]);

/** Mounts `{#if show} <label transition:kind={params} /> {/if}` through `$n.transition_native`. */
function mountNative(flags, kind, params, { intro = false, show: initial = true } = {}) {
  const show = $.state(initial);
  $n.mount(
    ($$anchor, $$props) => {
      $.push($$props, true);
      const w = $n.toplevel('window', 'present');
      const box = $n.element('box');
      $n.if_block($n.slot(box, null), () => $.get(show), ($$anchor) => {
        const l = $n.element('label');
        $n.transition_native(flags, l, kind, () => params);
        $n.set(l, 'label', 'Hello');
        $n.append($$anchor, l);
      });
      $n.append($n.slot(w, null), box);
      $n.append($$anchor, w);
      $.pop();
    },
    { intro },
  );
  return { show };
}

/** Captures OP_ANIMATE payloads; the backend plays them (answers 1). */
function capture() {
  const plain = th.host.style;
  const sent = [];
  th.host.style = function (op, h, text) {
    if (op !== 11) return plain.call(this, op, h, text);
    sent.push({ h, ...JSON.parse(text) });
    return 1;
  };
  return { sent, restore: () => (th.host.style = plain) };
}

test('MOTION-CSS: transition_native sends the built-in, its parameters and t range with the frames', async () => {
  const c = capture();
  try {
    const { show } = mountNative(3, 'fly', { y: 20, duration: 200, opacity: 0.5 });
    await flush();
    const label = created(take())[0];
    $.set(show, false);
    $.flush();
    await flush();
    const start = c.sent.find((s) => !s.cancel && s.frames.length > 0);
    assert.ok(start, 'the outro was sent');
    assert.equal(start.h, label);
    assert.ok(start.native, 'with its native descriptor');
    assert.equal(start.native.kind, 'fly');
    assert.deepEqual(start.native.params, { easing: 'cubicOut', y: 20, duration: 200, opacity: 0.5 });
    assert.equal(start.native.t1, 1);
    assert.equal(start.native.t2, 0);
    assert.equal(start.native.computed.opacity, '1');
    // the frames are the built-in's, as without the descriptor
    assert.deepEqual(start.frames.at(-1).transform.slice(0, 2), [0, 20]);
    assert.equal(start.frames.at(-1).opacity, 0.5);
  } finally {
    c.restore();
  }
});

test('MOTION-CSS: a custom easing keeps the frames only', async () => {
  const c = capture();
  try {
    const { show } = mountNative(3, 'fade', { duration: 100, easing: (t) => t * t });
    await flush();
    take();
    $.set(show, false);
    $.flush();
    await flush();
    const start = c.sent.find((s) => !s.cancel && s.frames.length > 0);
    assert.ok(start);
    assert.equal(start.native, undefined);
  } finally {
    c.restore();
  }
});

/** A runtime node stand-in whose host answers `computedStyle`. */
const fakeNode = (style) => ({ h: 1, alive: () => true, host: { computedStyle: () => style } });

test('MOTION-CSS: flip divides the move by the layout size (svelters had sx = to.width / to.width)', () => {
  // measured rects twice the layout size (a 2x ancestor): Svelte's clientWidth / to.width = 0.5
  const node = fakeNode({ transform: 'none', transformOrigin: '50px 20px', width: '100px', height: '40px' });
  const from = { left: 0, top: 0, width: 200, height: 80 };
  const to = { left: 0, top: 120, width: 200, height: 80 };
  const cfg = flip(node, { from, to }, { duration: 100 });
  assert.equal(cfg.css(0, 1), 'transform:  translate(0px, -60px) scale(1, 1);');
  const native = flipNative(node, { from, to }, { duration: 100, easing: easing.quintOut });
  native.css(0, 1); // notes t1
  assert.equal(native.duration, 100);
  assert.equal(native.easing, easing.quintOut);
});

/** Steps Svelte's own Spring and svelters:motion's side by side on the virtual clock. */
async function springs(opts, from, to, step, count) {
  now = 0;
  pending = [];
  const theirs = new svelteMotion.Spring(from, opts);
  const ours = new motion.Spring(from, opts);
  theirs.set(to);
  ours.set(to);
  const a = [];
  const b = [];
  for (let i = 0; i < count; i++) {
    frames(1, step);
    a.push(theirs.current);
    b.push(ours.current);
  }
  return [a, b];
}

test('MOTION-CSS: svelters:motion Spring equals svelte/motion at 60 Hz', async () => {
  for (const opts of [{}, { stiffness: 0.1, damping: 0.25 }, { stiffness: 0.3, damping: 0.9, precision: 0.001 }]) {
    const [svelte, ours] = await springs(opts, 0, 100, 1000 / 60, 240);
    for (let i = 0; i < svelte.length; i++) {
      assert.ok(Math.abs(svelte[i] - ours[i]) < 1e-6, `${JSON.stringify(opts)} frame ${i}: svelte ${svelte[i]}, ours ${ours[i]}`);
    }
  }
});

test('MOTION-CSS: svelters:motion Spring keeps the 60 Hz values at 120 Hz', async () => {
  const opts = { stiffness: 0.1, damping: 0.25 };
  const [svelte60] = await springs(opts, 0, 100, 1000 / 60, 120);
  const [, ours120] = await springs(opts, 0, 100, 1000 / 120, 240);
  for (let i = 0; i < svelte60.length; i++) {
    const v = ours120[2 * i + 1];
    assert.ok(Math.abs(svelte60[i] - v) < 1e-6, `frame ${i}: svelte at 60 Hz ${svelte60[i]}, ours at 120 Hz ${v}`);
  }
});

test('MOTION-CSS: svelters:motion Spring springs arrays and objects, and `instant` jumps', async () => {
  now = 0;
  pending = [];
  const s = new motion.Spring({ x: 0, y: [0, 10] });
  s.set({ x: 10, y: [5, 0] });
  frames(400, 1000 / 60);
  assert.deepEqual(s.current, { x: 10, y: [5, 0] });
  await s.set({ x: 1, y: [2, 3] }, { instant: true });
  assert.deepEqual(s.current, { x: 1, y: [2, 3] });
  assert.equal(motion.prefersReducedMotion.current, false);
});
