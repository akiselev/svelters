// R2: `bind:` lowered onto observation (src/bind-observe.js) against the Node TestHost. A native change
// is modelled as the TestHost's `nativeChange` (store, compare with the last value, `changed` line) and
// the `__svelters_changed` batch the host delivers (R1, src/observe.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

function harness() {
  const th = createTestHost();
  globalThis.__svelters_host = th.host;
  let seen = 0;
  const take = () => {
    const out = th.lines.slice(seen);
    seen = th.lines.length;
    return out;
  };
  /** The toolkit changes `prop` of `h` to `v` and announces it. */
  const native = (h, prop, v) => {
    const batch = th.nativeChange(h, prop, v);
    take();
    if (batch) globalThis.__svelters_changed(batch);
    $.flush();
  };
  const event = (h, signal, args = []) => {
    globalThis.__svelters_dispatch(h, signal, args);
    $.flush();
  };
  return { th, take, native, event };
}

function mountInWindow(body) {
  return $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    body($n.slot(w, null), w);
    $n.append($$anchor, w);
    $.pop();
  });
}

const timers = () => new Promise((r) => setTimeout(r, 5));

test('bind_prop: observes the prop, native changes reach state, echoes and redundant writes are no-ops', () => {
  const t = harness();
  const v = $.state('Ann');
  const unmount = mountInWindow((c) => {
    const e = $n.element('entry');
    $n.bind_prop(e, 'text', () => $.get(v), (x) => $.set(v, x), 'changed', 0);
    $n.append(c, e);
  });
  assert.deepEqual(t.take().filter((l) => l.includes('#2')), ['create entry #2', 'set #2 text "Ann"', 'observe #2 text on', 'insert #1 #2 before=end slot=default']);
  t.native(2, 'text', 'Zed');
  assert.equal($.get(v), 'Zed');
  assert.deepEqual(t.take(), [], 'the native value is not written back');
  t.event(2, 'changed', ['ignored']);
  assert.equal($.get(v), 'Zed', 'the signal plan is not used when the host observes');
  $.set(v, 'Bob');
  $.flush();
  assert.deepEqual(t.take(), ['set #2 text "Bob"']);
  // our own write announced back (the echo): same value, no state write, no op
  t.native(2, 'text', 'Bob');
  assert.equal($.get(v), 'Bob');
  unmount();
  const ops = t.take();
  assert.ok(ops.indexOf('observe #2 text off') >= 0 && ops.indexOf('observe #2 text off') < ops.indexOf('destroy #2'), ops.join('\n'));
});

test('bind_prop: falls back to the signal plan when the host cannot observe the prop', () => {
  const t = harness();
  t.th.host.observe = () => false; // a backend without the mechanism (Qt before R3, AppKit/WinUI before R4)
  const v = $.state('');
  mountInWindow((c) => {
    const e = $n.element('entry');
    $n.bind_prop(e, 'text', () => $.get(v), (x) => $.set(v, x), 'changed', 0);
    $n.append(c, e);
  });
  assert.ok(t.take().includes('listen #2 changed on'));
  t.event(2, 'changed', ['abc']);
  assert.equal($.get(v), 'abc');
});

test('bind_readonly: value at creation, after the first layout pass, then every observed change', async () => {
  const t = harness();
  const pos = $.state(-1);
  t.th.host.getProp = ((get) => (h, p) => (p === 'cursor-position' ? (get(h, p) ?? 0) : get(h, p)))(t.th.host.getProp);
  mountInWindow((c) => {
    const e = $n.element('entry');
    $n.bind_readonly(e, 'cursor-position', (x) => $.set(pos, x));
    $n.append(c, e);
  });
  assert.equal($.get(pos), 0);
  assert.ok(t.take().includes('observe #2 cursor-position on'));
  t.th.host.setProp(2, 'cursor-position', 4); // "layout" moved it, unannounced
  await timers();
  assert.equal($.get(pos), 4, 'the first-layout snapshot');
  t.native(2, 'cursor-position', 7);
  assert.equal($.get(pos), 7);
  assert.ok(!t.take().some((l) => l.startsWith('set #2 cursor-position')), 'never written');
});

test('bind_once: native -> state once, then write-only; no observation, no signal', () => {
  const t = harness();
  t.th.host.getProp = ((get) => (h, p) => (p === 'text' && get(h, p) === 'Ann' ? 'ANN' : get(h, p)))(t.th.host.getProp); // normalizes
  const v = $.state('Ann');
  mountInWindow((c) => {
    const e = $n.element('entry');
    $n.bind_once(e, 'text', () => $.get(v), (x) => $.set(v, x), false);
    $n.append(c, e);
  });
  $.flush();
  assert.equal($.get(v), 'ANN', 'the initial read converges state to the native value');
  const ops = t.take();
  assert.ok(!ops.some((l) => l.startsWith('observe') || l.startsWith('listen #2')), ops.join('\n'));
  t.native(2, 'text', 'Zed');
  assert.equal($.get(v), 'ANN', 'native edits never reach state');
  $.set(v, 'Bob');
  $.flush();
  assert.deepEqual(t.take(), ['set #2 text "Bob"'], 'state still reaches native');
});

test('bind_once: an undefined bound value takes the native value instead of writing it', () => {
  const t = harness();
  const v = $.state(undefined);
  mountInWindow((c) => {
    const e = $n.element('entry');
    t.th.host.setProp(e.h, 'text', 'native');
    $n.bind_once(e, 'text', () => $.get(v), (x) => $.set(v, x), false);
    $n.append(c, e);
  });
  $.flush();
  assert.equal($.get(v), 'native');
  assert.equal(t.take().filter((l) => l.startsWith('set #2 text')).length, 1, 'only the native write above');
});

test('bind_once read-only: one snapshot after the first layout pass', async () => {
  const t = harness();
  const pos = $.state(-1);
  mountInWindow((c) => {
    const e = $n.element('entry');
    $n.bind_once(e, 'cursor-position', () => $.get(pos), (x) => $.set(pos, x), true);
    $n.append(c, e);
  });
  assert.equal($.get(pos), -1, 'nothing before the layout pass');
  t.th.host.setProp(2, 'cursor-position', 3);
  await timers();
  assert.equal($.get(pos), 3);
  t.th.host.setProp(2, 'cursor-position', 5);
  await timers();
  assert.equal($.get(pos), 3, 'one snapshot only');
  assert.ok(!t.take().some((l) => l.startsWith('observe')));
});
