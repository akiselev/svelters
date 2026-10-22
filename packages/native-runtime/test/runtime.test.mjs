// Unit tests for @svelters/native-runtime against the Node TestHost.
// Run: node --import ./test/register.mjs --test test/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { types } from 'node:util';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

/** Fresh TestHost as the global host; `take()` returns the ops logged since the last take. */
function harness() {
  const th = createTestHost();
  globalThis.__svelters_host = th.host;
  let seen = 0;
  const take = () => {
    const out = th.lines.slice(seen);
    seen = th.lines.length;
    return out;
  };
  const event = (h, signal, args = []) => {
    globalThis.__svelters_dispatch(h, signal, args);
    $.flush();
  };
  const gone = (h, reason) => {
    for (const [x, why] of th.killNative(h, reason)) globalThis.__svelters_gone(x, why);
  };
  return { th, take, event, gone };
}

/** Mounts `body($$anchor)` as a component inside a presented window #1. */
function mountInWindow(body) {
  return $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    body($n.slot(w, null), w);
    $n.append($$anchor, w);
    $.pop();
  });
}

const moves = (ops) => ops.filter((l, i) => l.startsWith('remove') && ops[i + 1]?.startsWith(`insert ${l.split(' ').slice(1).join(' ')} `)).length;

test('bind: native echo and redundant writes produce no setProp, a JS write produces one', () => {
  const t = harness();
  const v = $.state('');
  let entry;
  mountInWindow((c) => {
    entry = $n.element('entry');
    $n.bind(entry, 'text', 'changed', 0, () => $.get(v), (x) => $.set(v, x));
    $n.append(c, entry);
  });
  assert.deepEqual(t.take().filter((l) => l.includes('#2')), [
    'create entry #2',
    'set #2 text ""',
    'listen #2 changed on',
    'insert #1 #2 before=end slot=default',
  ]);
  t.event(2, 'changed', ['abc']);
  assert.equal($.get(v), 'abc');
  assert.deepEqual(t.take(), [], 'the native value is not echoed');
  $.set(v, 'xyz');
  $.flush();
  assert.deepEqual(t.take(), ['set #2 text "xyz"']);
  $.set(v, 'xyz');
  $.flush();
  $n.set(entry, 'text', 'xyz');
  assert.deepEqual(t.take(), [], 'redundant writes are skipped');
});

test('track: a one-way prop the native side changes is not written back', () => {
  const t = harness();
  const v = $.state('');
  mountInWindow((c) => {
    const e = $n.element('entry');
    $.template_effect(() => $n.set(e, 'text', $.get(v)));
    $n.track(e, 'text', 'changed', 0);
    $n.on(e, 'changed', (x) => $.set(v, x));
    $n.append(c, e);
  });
  t.take();
  t.event(2, 'changed', ['a']);
  assert.deepEqual(t.take(), []);
  $.set(v, '');
  $.flush();
  assert.deepEqual(t.take(), ['set #2 text ""']);
});

test('capture / RESTORE writes the native value read before the first write', () => {
  const t = harness();
  const reads = [];
  t.th.host.getProp = (h, p) => (reads.push(`${h}.${p}`), 'native');
  const on = $.state(false);
  mountInWindow((c) => {
    const l = $n.element('label');
    $n.capture(l, 'tooltip');
    $.template_effect(() => $n.set(l, 'tooltip', $.get(on) ? 'mine' : $n.RESTORE));
    $n.append(c, l);
  });
  assert.ok(!t.take().some((l) => l.startsWith('set #2')), 'restoring the untouched value is a no-op');
  $.set(on, true);
  $.flush();
  $.set(on, false);
  $.flush();
  assert.deepEqual(t.take(), ['set #2 tooltip "mine"', 'set #2 tooltip "native"']);
  assert.deepEqual(reads, ['2.tooltip']);
});

test('token lists add and remove one token, reading the native list once', () => {
  const t = harness();
  let reads = 0;
  t.th.host.getProp = () => (reads++, ['flat', 'circular']);
  const accent = $.state(true);
  mountInWindow((c) => {
    const b = $n.element('button');
    $.template_effect(() => $n.token(b, 'css-classes', 'suggested-action', $.get(accent)));
    $n.token(b, 'css-classes', 'flat', true);
    $n.append(c, b);
  });
  assert.deepEqual(t.take().filter((l) => l.startsWith('set')), ['set #2 css-classes ["flat","circular","suggested-action"]']);
  $.set(accent, false);
  $.flush();
  assert.deepEqual(t.take(), ['set #2 css-classes ["flat","circular"]']);
  assert.equal(reads, 1);
});

test('if block: the new branch is built and inserted, then the old one removed and destroyed', () => {
  const t = harness();
  const show = $.state(false);
  mountInWindow((c) => {
    $n.if_block(
      c,
      () => $.get(show),
      (a) => $n.append(a, $n.element('label')),
      (a) => $n.append(a, $n.element('button')),
    );
    $n.append(c, $n.element('separator'));
  });
  assert.ok(t.take().includes('insert #1 #2 before=end slot=default'));
  $.set(show, true);
  $.flush();
  assert.deepEqual(t.take(), ['create label #4', 'insert #1 #4 before=#3 slot=default', 'remove #1 #2', 'destroy #2']);
  $.set(show, false);
  $.flush();
  assert.deepEqual(t.take(), ['create button #5', 'insert #1 #5 before=#3 slot=default', 'remove #1 #4', 'destroy #4']);
});

test('nested regions and component regions keep native order', () => {
  const t = harness();
  const show = $.state(false);
  function Child($$anchor, $$props) {
    $.push($$props, true);
    $n.if_block($$anchor, () => $$props.show, (a) => {
      const l = $n.element('label');
      $n.set(l, 'label', 'x');
      $n.append(a, l);
    });
    $.pop();
  }
  mountInWindow((c) => {
    const box = $n.element('box');
    const s = $n.slot(box, null);
    $n.append(s, $n.element('label')); // #3
    Child($n.region(s), {
      get show() {
        return $.get(show);
      },
    });
    $n.append(s, $n.element('button')); // #4
    $n.append(c, box);
  });
  t.take();
  $.set(show, true);
  $.flush();
  assert.deepEqual(t.take(), ['create label #5', 'set #5 label "x"', 'insert #2 #5 before=#4 slot=default']);
  $.set(show, false);
  $.flush();
  assert.deepEqual(t.take(), ['remove #2 #5', 'destroy #5']);
});

function keyedList(initial) {
  const t = harness();
  const rows = $.state($.proxy(initial));
  mountInWindow((c) => {
    $n.each_block(c, () => $.get(rows), (r) => r, (a, r) => {
      const l = $n.element('label');
      $.template_effect(() => $n.set(l, 'label', `${$.get(r)}`));
      $n.append(a, l);
    });
  });
  t.take();
  const update = (next) => {
    $.set(rows, next, true);
    $.flush();
    return t.take();
  };
  const order = () => t.th.widgets.get(1).children.map((h) => t.th.widgets.get(h).props.get('label'));
  return { update, order };
}

test('keyed each: reverse moves n-1 items', () => {
  const k = keyedList([1, 2, 3, 4, 5]);
  const ops = k.update([5, 4, 3, 2, 1]);
  assert.equal(moves(ops), 4);
  assert.equal(ops.length, 8);
  assert.deepEqual(k.order(), ['5', '4', '3', '2', '1']);
});

test('keyed each: rotate moves one item', () => {
  const k = keyedList([1, 2, 3, 4, 5]);
  assert.deepEqual(k.update([2, 3, 4, 5, 1]), ['remove #1 #2', 'insert #1 #2 before=end slot=default']);
  assert.deepEqual(k.update([1, 2, 3, 4, 5]), ['remove #1 #2', 'insert #1 #2 before=#3 slot=default']);
  assert.deepEqual(k.order(), ['1', '2', '3', '4', '5']);
});

test('keyed each: insert in the middle moves nothing', () => {
  const k = keyedList([1, 2, 4]);
  assert.deepEqual(k.update([1, 2, 3, 4]), ['create label #5', 'set #5 label "3"', 'insert #1 #5 before=#4 slot=default']);
  const ops = k.update([0, 1, 2, 3, 4, 9]);
  assert.equal(moves(ops), 0);
  assert.deepEqual(k.order(), ['0', '1', '2', '3', '4', '9']);
});

test('keyed each: mixed removals, moves and insertions end in the right order', () => {
  const k = keyedList([1, 2, 3, 4, 5, 6, 7, 8]);
  k.update([8, 3, 10, 1, 6, 5, 11]);
  assert.deepEqual(k.order(), ['8', '3', '10', '1', '6', '5', '11']);
  k.update([]);
  assert.deepEqual(k.order(), []);
  k.update([2, 1]);
  assert.deepEqual(k.order(), ['2', '1']);
});

test('each: item and index sources update in place', () => {
  const t = harness();
  const rows = $.state($.proxy([{ id: 1, t: 'a' }, { id: 2, t: 'b' }]));
  const plain = $.state($.proxy(['x', 'y']));
  mountInWindow((c) => {
    $n.each_block(c, () => $.get(rows), (r) => r.id, (a, r, i) => {
      const l = $n.element('label');
      $.template_effect(() => $n.set(l, 'label', `${$.get(i)}:${$.get(r).t}`));
      $n.append(a, l);
    });
    $n.each_block(c, () => $.get(plain), null, (a, r, i) => {
      const l = $n.element('button');
      $.template_effect(() => $n.set(l, 'label', `${$.get(i)}:${$.get(r)}`));
      $n.append(a, l);
    });
  });
  t.take();
  $.set(rows, [{ id: 2, t: 'B' }, { id: 1, t: 'a' }], true);
  $.flush();
  const ops = t.take();
  assert.ok(ops.includes('set #3 label "0:B"') && ops.includes('set #2 label "1:a"'), ops.join('\n'));
  $.get(plain)[1] = 'Y';
  $.flush();
  assert.deepEqual(t.take(), ['set #5 label "1:Y"']);
  $.get(plain).push('z');
  $.flush();
  assert.deepEqual(t.take(), ['create button #6', 'set #6 label "2:z"', 'insert #1 #6 before=end slot=default']);
  $.set(plain, ['only'], true);
  $.flush();
  // the block reconciles first; the surviving item's effect runs later in the same flush
  assert.deepEqual(t.take(), ['remove #1 #6', 'destroy #6', 'remove #1 #5', 'destroy #5', 'set #4 label "0:only"']);
});

test('each: fallback branch', () => {
  const t = harness();
  const rows = $.state($.proxy([]));
  mountInWindow((c) => {
    $n.each_block(c, () => $.get(rows), (r) => r, (a) => $n.append(a, $n.element('label')), (a) => $n.append(a, $n.element('spinner')));
  });
  assert.ok(t.take().includes('create spinner #2'));
  $.get(rows).push(1);
  $.flush();
  assert.deepEqual(t.take(), ['remove #1 #2', 'destroy #2', 'create label #3', 'insert #1 #3 before=end slot=default']);
});

test('teardown: remove top-level natives once, destroy children before parents, nested scopes first', () => {
  const t = harness();
  const show = $.state(true);
  const inner = $.state(true);
  mountInWindow((c) => {
    $n.if_block(c, () => $.get(show), (a) => {
      const box = $n.element('box'); // #2
      const s = $n.slot(box, null);
      $n.append(s, $n.element('label')); // #3
      $n.if_block(s, () => $.get(inner), (b) => {
        const row = $n.element('box'); // #4
        $n.append($n.slot(row, null), $n.element('button')); // #5
        $n.append(b, row);
      });
      $n.append(a, box);
      $n.append(a, $n.element('separator')); // #6
      $n.if_block(a, () => true, (b) => $n.append(b, $n.element('label'))); // #7, top level in the window
    });
  });
  t.take();
  $.set(show, false);
  $.flush();
  assert.deepEqual(t.take(), [
    'remove #1 #2',
    'remove #1 #6',
    'remove #1 #7',
    'destroy #5',
    'destroy #4',
    'destroy #7',
    'destroy #3',
    'destroy #2',
    'destroy #6',
  ]);
});

test('teardown after native death: no ops for dead nodes, handlers of dead nodes ignored', () => {
  const t = harness();
  const open = $.state(true);
  let clicks = 0;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    $n.if_block($$anchor, () => $.get(open), (a) => {
      const w = $n.element('window', 'present'); // #1
      const b = $n.element('button'); // #2
      $n.on(b, 'clicked', () => clicks++);
      $.template_effect(() => $n.set(b, 'label', $.get(open) ? 'open' : 'closed'));
      $n.append($n.slot(w, null), b);
      $n.append(a, w);
    });
    $.pop();
  });
  t.take();
  t.gone(1, 'closed');
  assert.deepEqual(t.take(), ['gone #1 closed', 'gone #2 parent']);
  t.event(2, 'clicked');
  assert.equal(clicks, 0);
  $.set(open, false);
  $.flush();
  assert.deepEqual(t.take(), []);
  $.set(open, true);
  $.flush();
  assert.deepEqual(t.take(), [
    'create window #3',
    'create button #4',
    'listen #4 clicked on',
    'set #4 label "open"',
    'insert #3 #4 before=end slot=default',
    'call #3 present []',
  ]);
});

test('handlers registered in a branch on an outer node are dropped with the branch', () => {
  const t = harness();
  const show = $.state(true);
  let hits = 0;
  mountInWindow((c) => {
    const b = $n.element('button');
    $n.append(c, b);
    $n.if_block(c, () => $.get(show), () => $n.on(b, 'clicked', () => hits++));
  });
  t.take();
  t.event(2, 'clicked');
  $.set(show, false);
  $.flush();
  assert.deepEqual(t.take(), ['listen #2 clicked off']);
  t.event(2, 'clicked');
  assert.equal(hits, 1);
});

test('unmount destroys effects, then natives (toplevels are destroyed, not removed)', () => {
  const t = harness();
  const n = $.state(0);
  let cleaned = false;
  const unmount = mountInWindow((c) => {
    const l = $n.element('label');
    $.template_effect(() => $n.set(l, 'label', `${$.get(n)}`));
    $.user_effect(() => () => (cleaned = true));
    $n.append(c, l);
  });
  $.flush();
  t.take();
  unmount();
  assert.ok(cleaned);
  assert.deepEqual(t.take(), ['destroy #2', 'destroy #1']);
  $.set(n, 1);
  $.flush();
  assert.deepEqual(t.take(), []);
  globalThis.__svelters_dispatch(1, 'clicked', []); // unknown handle: ignored
});

test('a first render that throws destroys what it created and rethrows (Codex P1, PR #3)', () => {
  const t = harness();
  assert.throws(
    () =>
      mountInWindow((c) => {
        const b = $n.element('button');
        $n.on(b, 'clicked', () => {});
        $n.append(c, b);
        throw new Error('boom');
      }),
    /boom/,
  );
  const ops = t.take();
  assert.ok(ops.includes('destroy #1'), `window destroyed: ${ops.join(' | ')}`);
  assert.ok(ops.includes('destroy #2'), `button destroyed: ${ops.join(' | ')}`);
  assert.ok(ops.indexOf('destroy #2') < ops.indexOf('destroy #1'), 'children before parents');
  globalThis.__svelters_dispatch(2, 'clicked', []); // forgotten: ignored
  assert.deepEqual(t.take(), []);
});

test('a deferred sync signal stops at stopImmediatePropagation, as dispatch_sync does (Codex P2, PR #3)', () => {
  harness();
  const seen = [];
  let h;
  mountInWindow((c) => {
    const b = $n.element('button');
    h = b.h;
    $n.on(b, 'clicked', (ev) => {
      seen.push('first');
      ev.stopImmediatePropagation();
    });
    $n.on(b, 'clicked', () => seen.push('second'));
    $n.append(c, b);
  });
  $.flush();
  globalThis.__svelters_dispatch(h, 'clicked', [], true);
  assert.deepEqual(seen, ['first']);
  seen.length = 0;
  assert.equal(globalThis.__svelters_dispatch_sync(h, 'clicked', []), true, 'stopping consumes');
  assert.deepEqual(seen, ['first']);
});

test('lis', () => {
  assert.deepEqual([...$n.lis([4, 3, 2, 1])].length, 1);
  assert.deepEqual([...$n.lis([0, -1, 1, 5, 2, 3])].sort(), [0, 2, 4, 5]);
  assert.deepEqual([...$n.lis([])], []);
});

test('RT1: a record prop from $state reaches the host as plain data, nested mutations included', () => {
  const t = harness();
  const plain = (v) => {
    assert.ok(!types.isProxy(v), 'no Proxy reaches the host');
    if (v && typeof v === 'object') for (const x of Object.values(v)) plain(x);
  };
  const pos = (line, column) => ({ line, column });
  // an array of objects holding objects, and an array inside an object inside an array
  const selection = $.state(
    $.proxy({
      ranges: [{ anchor: pos(1, 0), head: pos(1, 4) }],
      groups: [{ tags: ['a', 'b'], meta: { spans: [pos(0, 0)] } }],
    }),
  );
  const handle = { svlHandle: 99 };
  let ed;
  mountInWindow((c) => {
    ed = $n.element('entry');
    $.template_effect(() => $n.set(ed, 'selection', $.get(selection)));
    $.template_effect(() => $n.set(ed, 'extra', { ref: handle, list: [1, 2] }));
    $n.append(c, ed);
  });
  const sent = () => t.th.host.getProp(ed.h, 'selection');
  plain(sent());
  assert.deepEqual(sent(), {
    ranges: [{ anchor: { line: 1, column: 0 }, head: { line: 1, column: 4 } }],
    groups: [{ tags: ['a', 'b'], meta: { spans: [{ line: 0, column: 0 }] } }],
  });
  // a value with no proxy in it goes through uncopied (the handle object stays the same)
  assert.equal(t.th.host.getProp(ed.h, 'extra').ref, handle);
  t.take();

  // a nested mutation writes the prop again, as plain data
  $.get(selection).ranges[0].head.line = 3;
  $.flush();
  assert.deepEqual(t.take(), [
    'set #2 selection {"ranges":[{"anchor":{"line":1,"column":0},"head":{"line":3,"column":4}}],"groups":[{"tags":["a","b"],"meta":{"spans":[{"line":0,"column":0}]}}]}',
  ]);
  plain(sent());
  $.get(selection).groups[0].meta.spans.push(pos(2, 5));
  $.flush();
  assert.deepEqual(sent().groups[0].meta.spans, [{ line: 0, column: 0 }, { line: 2, column: 5 }]);
  plain(sent());
  t.take();

  // the same content again (a new proxy, equal data) is not rewritten
  $.set(selection, $.proxy($.snapshot($.get(selection))));
  $.flush();
  assert.deepEqual(t.take(), [], 'equal data is not written twice');
});
