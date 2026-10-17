// Child attributes, listener phases and sync handlers (the X2/X3 renderer contract) against the
// Node TestHost. Run: node --import ./test/register.mjs --test test/*.test.mjs
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
  return { th, take };
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

test('child_attr: sent after insert, on change, reset with null, re-sent after a keyed move', () => {
  const t = harness();
  const order = $.state($.proxy(['a', 'b']));
  const col = $.state(1);
  mountInWindow((c) => {
    const g = $n.element('grid');
    $n.each_block($n.slot(g, null), () => $.get(order), (v) => v, (anchor, item, i) => {
      const l = $n.element('label');
      $.template_effect(() => $n.child_attr(l, 'row', $.get(i)));
      $.template_effect(() => $n.child_attr(l, 'column', $.get(item) === 'a' ? $.get(col) : undefined));
      $n.append(anchor, l);
    });
    $n.append(c, g);
  });
  assert.deepEqual(t.take().filter((l) => /^(insert #2|attr)/.test(l)), [
    'insert #2 #3 before=end slot=default',
    'attr #2 #3 row 0',
    'attr #2 #3 column 1',
    'insert #2 #4 before=end slot=default',
    'attr #2 #4 row 1',
  ]);
  $.set(col, 2);
  $.flush();
  assert.deepEqual(t.take(), ['attr #2 #3 column 2']);
  $.set(col, undefined);
  $.flush();
  assert.deepEqual(t.take(), ['attr #2 #3 column null'], 'null resets and drops the attribute');
  $.set(order, ['b', 'a']);
  $.flush();
  const ops = t.take();
  // the moved child gets its (remaining) attributes again right after its re-insert
  const i = ops.findIndex((l) => l.startsWith('insert #2'));
  assert.ok(i > 0, ops.join('\n'));
  const moved = ops[i].split(' ')[2];
  assert.equal(ops[i + 1], `attr #2 ${moved} row ${moved === '#3' ? 0 : 1}`);
});

test('on: flags make separate connections; dispatch runs only the listeners with those flags', () => {
  const t = harness();
  const seen = [];
  let e;
  mountInWindow((c) => {
    e = $n.element('entry');
    $n.on(e, 'changed', () => seen.push('plain'));
    $n.on(e, 'changed', () => seen.push('capture'), $n.LISTEN_CAPTURE);
    $n.on(e, 'changed', () => seen.push('after'), $n.LISTEN_AFTER);
    $n.append(c, e);
  });
  assert.deepEqual(t.take().filter((l) => l.startsWith('listen')), [
    'listen #2 changed on',
    'listen #2 changed on capture',
    'listen #2 changed on after',
  ]);
  globalThis.__svelters_dispatch(2, 'changed', ['x'], false, $n.LISTEN_CAPTURE);
  globalThis.__svelters_dispatch(2, 'changed', ['x'], false, 0);
  globalThis.__svelters_dispatch(2, 'changed', ['x'], false, $n.LISTEN_AFTER);
  assert.deepEqual(seen, ['capture', 'plain', 'after']);
});

test('dispatch_sync: NativeEvent result rules, (event, ...args), deferred delivery', () => {
  harness();
  const got = [];
  mountInWindow((c, w) => {
    $n.on(w, 'close-request', (ev, ...args) => {
      got.push([ev instanceof $n.NativeEvent, ev.deferred, args]);
      if (args[0] === 'veto') ev.preventDefault();
      if (args[0] === 'value') ev.result = 7;
    });
    void c;
  });
  const sync = globalThis.__svelters_dispatch_sync;
  assert.equal(sync(1, 'close-request', ['veto']), true);
  assert.equal(sync(1, 'close-request', ['undecided']), null);
  assert.equal(sync(1, 'close-request', ['value']), 7);
  globalThis.__svelters_dispatch(1, 'close-request', ['veto'], true);
  assert.deepEqual(got, [
    [true, false, ['veto']],
    [true, false, ['undecided']],
    [true, false, ['value']],
    [true, true, ['veto']],
  ]);
  assert.throws(() => sync(99, 'close-request', []), /unknown handle 99/);
});

test('listen(): the renderer helper as an attachment, removed by its cleanup', () => {
  const t = harness();
  const show = $.state(true);
  const seen = [];
  mountInWindow((c) => {
    const b = $n.element('button');
    $.attach(b, () => ($.get(show) ? $n.listen('clicked', () => seen.push('after'), { after: true }) : null));
    $n.append(c, b);
  });
  $.flush();
  assert.ok(t.take().includes('listen #2 clicked on after'));
  globalThis.__svelters_dispatch(2, 'clicked', [], false, $n.LISTEN_AFTER);
  assert.deepEqual(seen, ['after']);
  $.set(show, false);
  $.flush();
  assert.deepEqual(t.take(), ['listen #2 clicked off after']);
});

test('track is passive: it mirrors a delivered signal but never listens by itself', () => {
  const t = harness();
  let e;
  mountInWindow((c) => {
    e = $n.element('entry');
    $n.set(e, 'text', 'a');
    $n.track(e, 'text', 'changed', 0);
    $n.append(c, e);
  });
  assert.ok(!t.take().some((l) => l.startsWith('listen')));
  globalThis.__svelters_dispatch(2, 'changed', ['typed']);
  $n.set(e, 'text', 'typed');
  assert.deepEqual(t.take(), [], 'the mirrored native value is not written back');
});
