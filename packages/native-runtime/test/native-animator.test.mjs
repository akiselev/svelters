// TR2: native animation on a backend that plays keyframes itself (src/motion.js `startNative`). This file
// has its own host because the runtime remembers per host whether the backend refused
// SVL_STYLE_ANIMATE. The harness (virtual raf, TestHost) is transitions.test.mjs's.
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
const opacities = (log) => [...log.matchAll(/style #\d+ opacity (\S+)/g)].map((m) => m[1]);
const created = (log) => [...log.matchAll(/create label #(\d+)/g)].map((m) => +m[1]);

test('TR2: a backend that animates natively gets the keyframes once, no per-frame ops, and ends on animationend', async () => {
  const plain = th.host.style;
  const sent = [];
  th.host.style = function (op, h, text) {
    if (op !== 11) return plain.call(this, op, h, text);
    sent.push({ h, ...JSON.parse(text) });
    return 1; // the backend plays it
  };
  try {
    const { show, ev } = mountIf(BOTH, fade, { duration: 100 });
    await flush();
    const label = created(take())[0];
    $.set(show, false);
    $.flush();
    await flush();
    const start = sent.find((s) => !s.cancel);
    assert.ok(start, 'the keyframes were sent');
    assert.equal(start.h, label);
    assert.equal(start.duration, 100);
    assert.ok(start.frames.length >= 2 && start.frames[0].opacity === 1 && start.frames.at(-1).opacity === 0);
    take();
    assert.equal(frames(10), 0, 'no JS frame loop');
    assert.equal(opacities(take()).length, 0, 'no per-frame opacity ops');
    assert.equal(ev.outroend, 0, 'JS does not end it by its clock');
    $n.dispatch(label, 'animationend', [start.id]); // inert node: the end is still delivered
    await flush();
    assert.equal(ev.outroend, 1);
    assert.match(take(), /remove #\d+ #\d+|destroy #\d+/, 'removed when the backend reported the end');
  } finally {
    th.host.style = plain;
  }
});
