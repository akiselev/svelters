// TR1: transitions on native elements (src/motion.js, src/transition.js, the blocks' pause/resume)
// against the TestHost, with a virtual frame clock: Svelte's `raf` is patched as Svelte's own test
// helper (animation-helpers.js) does, so frames and time only advance when the test says so.
import assert from 'node:assert/strict';
import test from 'node:test';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const $n = await import('../src/index.js');
const { raf } = await import('../src/core/internal.js');
const { fade, fly, slide } = await import('../src/transition.js');

// ---- the virtual clock ----------------------------------------------------------------------
let now = 0;
let pending = [];
raf.now = () => now;
raf.tick = (fn) => pending.push(fn);
/** Runs `n` frames of 16 ms; returns how many frames had a callback waiting. */
function frames(n) {
  let ran = 0;
  for (let i = 0; i < n; i++) {
    now += 16;
    const due = pending;
    pending = [];
    if (due.length) ran++;
    for (const fn of due) fn(now);
  }
  return ran;
}
/** Frames until nothing asks for another (bounded). */
function settle(max = 200) {
  for (let i = 0; i < max && pending.length; i++) frames(1);
  assert.equal(pending.length, 0, 'the clock went idle');
}

const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');

/**
 * Mounts `{#if show} <label transition={fn(params)} /> {/if}` in a window; returns the `show`
 * signal and the outroend/introend counters.
 */
function mountIf(flags, fn, params, { intro = false, show: initial = true } = {}) {
  const show = $.state(initial);
  const ev = { introend: 0, outroend: 0, introstart: 0, outrostart: 0 };
  $n.mount(
    ($$anchor, $$props) => {
      $.push($$props, true);
      const w = $n.toplevel('window', 'present');
      const box = $n.element('box');
      $n.if_block($n.slot(box, null), () => $.get(show), ($$anchor) => {
        const l = $n.element('label');
        for (const k of Object.keys(ev)) $n.on(l, k, () => ev[k]++);
        $n.transition(flags, l, () => fn, () => params);
        $n.set(l, 'label', 'Hello');
        $n.append($$anchor, l);
      });
      $n.append($n.slot(w, null), box);
      $n.append($$anchor, w);
      $.pop();
    },
    { intro },
  );
  return { show, ev };
}

const IN = 1;
const OUT = 2;
const BOTH = IN | OUT;
const GLOBAL = 4;

const opacities = (log) => [...log.matchAll(/style #\d+ opacity (\S+)/g)].map((m) => m[1]);
const created = (log) => [...log.matchAll(/create label #(\d+)/g)].map((m) => +m[1]);

test('an outro keeps the element until it ends, then removes it; the clock goes idle', async () => {
  const { show, ev } = mountIf(BOTH, fade, { duration: 100 });
  await flush();
  const first = take();
  assert.equal(created(first).length, 1);
  assert.equal(opacities(first).length, 0, 'a local intro does not play on the first render');

  $.set(show, false);
  $.flush();
  await flush();
  let log = take();
  assert.doesNotMatch(log, /remove|destroy/, 'still on screen while it fades out');
  assert.match(log, /style #\d+ inert "1"/, 'an outroing element is inert');
  frames(3);
  await flush();
  log = take();
  assert.ok(opacities(log).length >= 2, `per-frame opacity ops: ${log}`);
  assert.doesNotMatch(log, /remove|destroy/);
  settle();
  await flush();
  log = take();
  assert.match(log, /remove #\d+ #\d+|destroy #\d+/, 'removed when the outro ended');
  assert.equal(ev.outrostart, 1);
  assert.equal(ev.outroend, 1);
  assert.equal(frames(5), 0, 'no frame requested once nothing animates');
});

test('the intro plays on a later render: opacity rises to the base value', async () => {
  const { show, ev } = mountIf(BOTH, fade, { duration: 100 }, { show: false });
  await flush();
  take();
  $.set(show, true);
  $.flush();
  await flush();
  settle();
  await flush();
  const ops = opacities(take());
  assert.equal(ops[0], '"0"', `starts transparent: ${ops}`);
  assert.equal(ops.at(-1), 'null', 'ends at the element own opacity (none set: null)');
  assert.equal(ev.introend, 1);
});

test('reversal: toggling back during the outro resumes the same element', async () => {
  const { show, ev } = mountIf(BOTH, fade, { duration: 160 });
  await flush();
  take();
  $.set(show, false);
  $.flush();
  frames(4);
  $.set(show, true);
  $.flush();
  await flush();
  settle();
  await flush();
  const log = take();
  assert.equal(created(log).length, 0, `no new element: ${log}`);
  assert.doesNotMatch(log, /remove|destroy/);
  assert.match(log, /style #\d+ inert null/, 'inert is lifted again');
  assert.equal(ev.outroend, 0);
  assert.equal(ev.introend, 1);
});

test('|global intros play on mount with intro: true; local ones do not', async () => {
  mountIf(IN | GLOBAL, fade, { duration: 50 }, { intro: true });
  await flush();
  assert.equal(opacities(take())[0], '"0"');
  settle();
  await flush();
  take();
  mountIf(IN, fade, { duration: 50 }, { intro: true });
  await flush();
  assert.equal(opacities(take()).length, 0);
});

test('fly sends the transform channel; slide the clip channel (from host bounds)', async () => {
  th.host.bounds = () => [0, 0, 120, 40];
  try {
    let r = mountIf(OUT, fly, { duration: 64, y: 20 });
    await flush();
    take();
    $.set(r.show, false);
    $.flush();
    frames(2);
    let log = take();
    assert.match(log, /style #\d+ transform "0 [\d.]+ 1 1 0.5 0.5"/, log);
    settle();
    await flush();
    log = take();
    assert.match(log, /style #\d+ transform null|remove|destroy/);

    r = mountIf(OUT, slide, { duration: 64 });
    await flush();
    take();
    $.set(r.show, false);
    $.flush();
    frames(2);
    log = take();
    assert.match(log, /style #\d+ clip "-1 [\d.]+"/, log);
    settle();
  } finally {
    delete th.host.bounds;
  }
});

test('a backend without a channel: the channel is dropped, the outro still ends and removes', async () => {
  const style = th.host.style;
  th.host.style = (op, h, text) => {
    if (op >= 8) throw new Error(`unknown style op ${op}`);
    return style.call(th.host, op, h, text);
  };
  try {
    const { show, ev } = mountIf(OUT, fly, { duration: 64, x: 30 });
    await flush();
    take();
    $.set(show, false);
    $.flush();
    settle();
    await flush();
    assert.match(take(), /remove #\d+ #\d+|destroy #\d+/);
    assert.equal(ev.outroend, 1);
  } finally {
    th.host.style = style;
  }
});

test('a tick transition runs JS per frame and ends', async () => {
  const ts = [];
  const custom = () => ({ duration: 64, tick: (t) => ts.push(t) });
  const { show, ev } = mountIf(OUT, custom, {});
  await flush();
  take();
  $.set(show, false);
  $.flush();
  settle();
  await flush();
  assert.ok(ts.length >= 3, `tick ran per frame: ${ts}`);
  assert.equal(ts.at(-1), 0, 'an outro ends at t = 0');
  assert.match(take(), /remove #\d+ #\d+|destroy #\d+/);
  assert.equal(ev.outroend, 1);
});

test('TR2: reduced motion ends the JS-played outro at once, with the last frame', async () => {
  const { setReducedMotion } = $n;
  setReducedMotion(true);
  try {
    const { show, ev } = mountIf(BOTH, fade, { duration: 400 });
    await flush();
    take();
    $.set(show, false);
    $.flush();
    await flush();
    frames(2);
    await flush();
    assert.equal(ev.outroend, 1, 'ended within two frames of a 400 ms outro');
    assert.match(take(), /remove #\d+ #\d+|destroy #\d+/);
  } finally {
    setReducedMotion(false);
  }
});
