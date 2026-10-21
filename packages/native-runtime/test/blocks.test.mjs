// Unit tests for the template blocks of @svelters/native-runtime: {#key}, {#await}, dynamic
// {@render} (snippet), runtime class sets. Run with the package's `pnpm test`.
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

/** Mounts `body(container)` inside a presented window #1 > box #2. */
function mountInBox(body) {
  return $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const b = $n.element('box');
    body($n.slot(b, null));
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $.pop();
  });
}

const label = (anchor, text) => {
  const l = $n.element('label');
  $.template_effect(() => $n.set(l, 'label', typeof text === 'function' ? text() : text));
  $n.append(anchor, l);
  return l;
};

/** Lets promise reactions and the runtime's microtasks run. */
const settle = () => new Promise((r) => setImmediate(r));

test('key block: a changed key re-creates the natives (new first, then the old removed + destroyed)', () => {
  const t = harness();
  const k = $.state(1);
  const other = $.state('a');
  mountInBox((c) => {
    $n.key_block(c, () => $.get(k), (anchor) => label(anchor, () => `v${$.get(other)}`));
    label(c, 'after');
  });
  t.take();
  $.set(other, 'b');
  $.flush();
  assert.deepEqual(t.take(), ['set #3 label "vb"'], 'non-key state updates in place');
  $.set(k, 1);
  $.flush();
  assert.deepEqual(t.take(), [], 'same key: nothing');
  $.set(k, 2);
  $.flush();
  assert.deepEqual(t.take(), ['create label #5', 'set #5 label "vb"', 'insert #2 #5 before=#4 slot=default', 'remove #2 #3', 'destroy #3']);
  $.set(k, NaN);
  $.flush();
  t.take();
  $.set(k, NaN);
  $.flush();
  assert.deepEqual(t.take(), [], 'NaN equals NaN');
});

test('key block: objects compare by identity', () => {
  const t = harness();
  const o = { a: 1 };
  const k = $.state(o); // a raw source: no proxy
  mountInBox((c) => $n.key_block(c, () => $.get(k), (anchor) => label(anchor, 'x')));
  t.take();
  $.set(k, o);
  $.flush();
  assert.deepEqual(t.take(), []);
  $.set(k, { a: 1 });
  $.flush();
  assert.equal(t.take().filter((l) => l.startsWith('destroy')).length, 1);
});

test('snippet: a changed function replaces the branch in place; null renders nothing; args are thunks', () => {
  const t = harness();
  const which = $.state(0);
  const arg = $.state('x');
  const a = (anchor, v) => label(anchor, () => `a ${v()}`);
  const b = (anchor, v) => {
    label(anchor, () => `b ${v()}`);
    label(anchor, '!');
  };
  mountInBox((c) => {
    $n.snippet(c, () => [a, b, null][$.get(which)], () => $.get(arg));
    label(c, 'end');
  });
  assert.deepEqual(t.take().slice(2, 5), ['create label #3', 'set #3 label "a x"', 'insert #2 #3 before=end slot=default']);
  $.set(arg, 'y');
  $.flush();
  assert.deepEqual(t.take(), ['set #3 label "a y"']);
  $.set(which, 1);
  $.flush();
  assert.deepEqual(t.take(), [
    'create label #5',
    'set #5 label "b y"',
    'insert #2 #5 before=#4 slot=default',
    'create label #6',
    'set #6 label "!"',
    'insert #2 #6 before=#4 slot=default',
    'remove #2 #3',
    'destroy #3',
  ]);
  $.set(which, 2);
  $.flush();
  assert.deepEqual(t.take(), ['remove #2 #5', 'remove #2 #6', 'destroy #5', 'destroy #6']);
  $.set(which, 0);
  $.flush();
  assert.deepEqual(t.take(), ['create label #7', 'set #7 label "a y"', 'insert #2 #7 before=#4 slot=default']);
});

/** Await block with labelled branches; `input` is a source holding the awaited value. */
function mountAwait(input, { pending = true, then = true, catch_ = true } = {}) {
  return mountInBox((c) => {
    $n.await_block(
      c,
      () => $.get(input),
      pending ? (anchor) => label(anchor, 'pending') : null,
      then ? (anchor, v) => label(anchor, () => `then ${$.get(v)}`) : null,
      catch_ ? (anchor, e) => label(anchor, () => `catch ${$.get(e)}`) : null,
    );
    label(c, 'end');
  });
}

const labels = (ops) => ops.filter((l) => / label "/.test(l)).map((l) => l.replace(/^set #\d+ label /, ''));

test('await: a non-promise shows `then` synchronously', () => {
  const t = harness();
  mountAwait($.state(42));
  assert.deepEqual(labels(t.take()), ['"then 42"', '"end"']);
});

test('await: a pending promise shows nothing synchronously, `pending` from a microtask, then `then`', async () => {
  const t = harness();
  let resolve;
  mountAwait($.state(new Promise((r) => (resolve = r))));
  assert.deepEqual(labels(t.take()), ['"end"'], 'nothing before the microtask');
  await settle();
  assert.deepEqual(t.take(), ['create label #4', 'set #4 label "pending"', 'insert #2 #4 before=#3 slot=default']);
  resolve('ok');
  await settle();
  assert.deepEqual(t.take(), ['create label #5', 'set #5 label "then ok"', 'insert #2 #5 before=#3 slot=default', 'remove #2 #4', 'destroy #4']);
});

test('await: a promise that settles before the pending task runs never shows `pending`', async () => {
  const t = harness();
  const input = $.state(0);
  mountAwait(input);
  t.take();
  // the block re-runs in Svelte's batch task; the promise's reaction is queued before the
  // pending task (a microtask of its own), so `then` wins and the branch is kept
  $.set(input, Promise.resolve(1));
  await settle();
  assert.deepEqual(t.take(), ['set #3 label "then 1"']);
});

test('await: under flushSync the pending task runs at once (Svelte\'s queue_micro_task)', async () => {
  const t = harness();
  const input = $.state(0);
  mountAwait(input);
  t.take();
  $.set(input, Promise.resolve(1));
  $.flush();
  assert.deepEqual(labels(t.take()), ['"pending"'], 'flushSync drains the task queue: pending before the promise job');
  await settle();
  assert.deepEqual(labels(t.take()), ['"then 1"']);
});

test('await: a superseded promise is ignored; settling into the shown branch updates its value', async () => {
  const t = harness();
  let resolveA, resolveB;
  const input = $.state(new Promise((r) => (resolveA = r)));
  mountAwait(input);
  await settle();
  t.take();
  $.set(input, new Promise((r) => (resolveB = r)));
  $.flush();
  await settle();
  assert.deepEqual(t.take(), [], 'still pending: the branch stays');
  resolveA('A');
  await settle();
  assert.deepEqual(t.take(), [], 'A is stale');
  resolveB('B');
  await settle();
  assert.deepEqual(labels(t.take()), ['"then B"']);
  $.set(input, Promise.resolve('C'));
  await settle();
  assert.deepEqual(t.take(), ["set #5 label \"then C\""], 'same branch, new value: no rebuild, no pending');
});

test('await: rejection shows `catch`; without `catch` the error is rethrown', async () => {
  const t = harness();
  const input = $.state(0);
  mountAwait(input);
  t.take();
  $.set(input, Promise.reject('boom'));
  await settle();
  assert.deepEqual(labels(t.take()), ['"catch boom"']);

  // a thenable lets the test observe what the rejection handler throws
  const t2 = harness();
  let thrown;
  const thenable = {
    then(_resolve, reject) {
      queueMicrotask(() => {
        try {
          reject('bad');
        } catch (e) {
          thrown = e;
        }
      });
    },
  };
  const input2 = $.state(0);
  mountAwait(input2, { catch_: false });
  t2.take();
  $.set(input2, thenable);
  await settle();
  assert.equal(thrown, 'bad');
  assert.deepEqual(labels(t2.take()), [], 'the then branch is removed, and `pending` was never shown');
});

test('await: a promise settling after teardown produces no ops', async () => {
  const t = harness();
  let resolve;
  const unmount = mountAwait($.state(new Promise((r) => (resolve = r))));
  await settle();
  unmount();
  t.take();
  resolve('late');
  await settle();
  assert.deepEqual(t.take(), []);
});

test('component: bind:this follows the instance of a dynamic component (Codex P2, PR #3)', () => {
  const t = harness();
  const make = (name) => ($$anchor, props) => {
    label($$anchor, `${name} ${props.label}`);
    return { name };
  };
  const A = make('A');
  const B = make('B');
  const C = $.state(A);
  let ref = 'unset';
  mountInBox((c) => {
    $n.component(c, () => $.get(C), { label: 'x' }, ($$exports) =>
      $.bind_this($$exports, ($$value) => (ref = $$value), () => ref),
    );
  });
  $.flush();
  assert.deepEqual(ref, { name: 'A' });
  t.take();
  $.set(C, B);
  $.flush();
  assert.deepEqual(ref, { name: 'B' }, 'rebound to the new instance');
  assert.ok(t.take().some((l) => l.includes('"B x"')));
  $.set(C, null);
  $.flush();
  assert.equal(ref, null, 'cleared when the component goes');
});

test('styling helpers: class_name, style_text, setter_value (ST1)', () => {
  assert.equal($n.class_name('a svelte-x', new Set(['b', 'a']), [['c', true], ['a', false], ['d', false]]), 'svelte-x b c');
  assert.equal($n.class_name('', null, []), '');
  assert.equal($n.style_text('color: red', [['font-weight', 'bold'], ['opacity', null]]), 'color: red; font-weight: bold');
  assert.equal($n.style_text(null, [['color', 'blue']]), 'color: blue');
  assert.equal($n.setter_value('gtk', 'spacing', 8), 8);
  assert.equal($n.setter_value('gtk', 'spacing', '12px'), 12);
  assert.equal($n.setter_value('gtk', 'halign', 'center'), 'center');
  assert.deepEqual($n.setter_value('qt', 'contents-margins', '1 2'), { top: 1, right: 2, bottom: 1, left: 2 });
});

test('attr: classes, widget name and inline style reach host.style once per job (ST1)', async () => {
  const t = harness();
  let b;
  mountInBox((c) => {
    b = $n.element('button');
    $n.attr(b, 'class', 'primary svelte-x');
    $n.attr(b, 'id', 'ok');
    $n.attr(b, 'style', 'color: red; hexpand: true');
    $n.append(c, b);
  });
  await new Promise((r) => setImmediate(r));
  const ops = t.take();
  assert.ok(ops.includes(`style #${b.h} classes "primary svelte-x"`), ops.join('\n'));
  assert.ok(ops.includes(`style #${b.h} name "ok"`), ops.join('\n'));
  assert.ok(ops.includes(`style #${b.h} inline "color: red;"`), ops.join('\n'));
  assert.ok(ops.includes(`set #${b.h} hexpand true`), 'inline setter written with set');
  $n.attr(b, 'class', null);
  await new Promise((r) => setImmediate(r));
  assert.deepEqual(t.take(), [`style #${b.h} classes null`]);
});

test('classes: clsx semantics', () => {
  assert.deepEqual([...$n.classes('a  b')], ['a', 'b']);
  assert.deepEqual([...$n.classes(['a', { b: true, c: false }, null])], ['a', 'b']);
  assert.deepEqual([...$n.classes(null)], []);
});

test('boundary: an error while rendering shows `failed`, reset() renders the children again', () => {
  const t = harness();
  const broken = $.state(true);
  const seen = [];
  let retry;
  mountInBox((c) =>
    $n.boundary($n.region(c), {
      onerror: (e, reset) => {
        seen.push(e.message);
        retry = reset;
      },
      failed: (anchor, error) => label(anchor, () => `failed: ${error().message}`),
      children: (anchor) =>
        label(anchor, () => {
          if ($.get(broken)) throw new Error('boom');
          return 'ok';
        }),
    }),
  );
  $.flush(); // drains Svelte's task queue: onerror, then `failed`
  assert.deepEqual(seen, ['boom']);
  const failedOps = t.take();
  assert.ok(failedOps.some((l) => /label "failed: boom"/.test(l)), failedOps.join('\n'));
  $.set(broken, false);
  retry();
  $.flush();
  const ops = t.take();
  assert.ok(ops.some((l) => /label "ok"/.test(l)), ops.join('\n'));
  assert.ok(ops.some((l) => /^destroy/.test(l)), 'failed content destroyed');
});

test('boundary: an error in an effect after mount reaches the boundary; one without handlers passes it on', () => {
  const t = harness();
  const n = $.state(0);
  const seen = [];
  mountInBox((c) =>
    $n.boundary($n.region(c), {
      onerror: (e) => seen.push(`outer ${e.message}`),
      children: (anchor) =>
        $n.boundary($n.region(anchor), {
          children: (a) =>
            label(a, () => {
              if ($.get(n) > 0) throw new Error('later');
              return 'fine';
            }),
        }),
    }),
  );
  t.take();
  $.set(n, 1);
  $.flush();
  assert.deepEqual(seen, ['outer later']);
  assert.ok(t.take().some((l) => /^destroy/.test(l)), 'content removed');
});
