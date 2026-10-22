// H1: $n.hmr (component swap with rollback). Svelte 5 semantics: parent handles untouched, props and
// the instance object kept, a throwing new version rolls back.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

function harness() {
  const th = createTestHost();
  globalThis.__svelters_host = th.host;
  return th;
}

const label = (anchor, text) => {
  const l = $n.element('label');
  $.template_effect(() => $n.set(l, 'label', typeof text === 'function' ? text() : text));
  $n.append(anchor, l);
  return l;
};

/** A component that renders one label with `text` and returns exports `{ v }`. */
const comp = (text, seen) => {
  function C($$anchor, $$props) {
    $.push($$props, true);
    seen?.push(label($$anchor, () => `${text}:${$$props.n}`));
    $.pop();
    return { v: text };
  }
  return C;
};

/** `window > box` holding `before`, the HMR component, `after`; returns pieces. */
function app(Wrapped, props = {}) {
  const nodes = {};
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const b = $n.element('box');
    const c = $n.slot(b, null);
    nodes.before = label(c, 'before');
    nodes.inst = Wrapped(c, props);
    nodes.after = label(c, 'after');
    nodes.box = b;
    nodes.win = w;
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $.pop();
  });
  return nodes;
}

const texts = (th) => th.lines;

test('swap keeps parent handles, props and position; new text shows', () => {
  const th = harness();
  const n = $.state(1);
  const A = comp('a');
  const W = $n.hmr(A, 'A.svelte');
  const props = { get n() { return $.get(n); } };
  const seen = [];
  const first = [];
  const nodes = app((c, p) => W(c, p), props);
  const boxH = nodes.box.h, winH = nodes.win.h, beforeH = nodes.before.h, afterH = nodes.after.h;
  const B = comp('b', seen);
  $n.hmrStats.swaps = 0;
  const WB = $n.hmr(B, 'A.svelte');
  W[$n.HMR].update(WB);
  $.flush();
  assert.equal(nodes.box.h, boxH);
  assert.equal(nodes.win.h, winH);
  assert.equal(nodes.before.h, beforeH);
  assert.equal(nodes.after.h, afterH);
  assert.equal($n.hmrStats.swaps, 1);
  assert.equal(seen.length, 1, 'new version rendered once');
  const out = th.lines.join('\n');
  assert.match(out, /b:1/);
  // props stay live for the new version
  $.set(n, 2);
  $.flush();
  assert.match(th.lines.join('\n'), /b:2/);
  void first;
});

test('the instance object is refilled (bind:this stays valid)', () => {
  harness();
  const W = $n.hmr(comp('a'), 'A.svelte');
  const nodes = app(W, { n: 0 });
  const inst = nodes.inst;
  assert.equal(inst.v, 'a');
  W[$n.HMR].update($n.hmr(comp('b'), 'A.svelte'));
  $.flush();
  assert.equal(nodes.inst, inst);
  assert.equal(inst.v, 'b');
});

test('a throwing new version rolls back and the next good version applies', () => {
  const th = harness();
  const reports = [];
  $n.setHmrReporter((id, e) => reports.push([id, e.message]));
  const W = $n.hmr(comp('a'), 'A.svelte');
  const nodes = app(W, { n: 0 });
  const boom = $n.hmr(function Boom($$anchor, $$props) {
    $.push($$props, true);
    label($$anchor, 'partial');
    throw new Error('boom');
  }, 'A.svelte');
  W[$n.HMR].update(boom);
  $.flush();
  assert.deepEqual(reports, [['A.svelte', 'boom']]);
  assert.equal(nodes.inst.v, 'a', 'old version still live');
  assert.equal($n.hmrStats.failed.has('A.svelte'), true);
  W[$n.HMR].update($n.hmr(comp('c'), 'A.svelte'));
  $.flush();
  assert.equal(nodes.inst.v, 'c');
  assert.equal($n.hmrStats.failed.has('A.svelte'), false);
  $n.setHmrReporter(null);
});

test('each instance swaps independently of other components', () => {
  harness();
  const W1 = $n.hmr(comp('x'), 'X.svelte');
  const W2 = $n.hmr(comp('y'), 'Y.svelte');
  let a, b;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const bx = $n.element('box');
    const c = $n.slot(bx, null);
    a = W1(c, { n: 0 });
    b = W2(c, { n: 0 });
    $n.append($n.slot(w, null), bx);
    $n.append($$anchor, w);
    $.pop();
  });
  W1[$n.HMR].update($n.hmr(comp('x2'), 'X.svelte'));
  $.flush();
  assert.equal(a.v, 'x2');
  assert.equal(b.v, 'y');
});
