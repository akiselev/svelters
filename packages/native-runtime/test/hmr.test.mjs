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

// ---- H3: $state kept across a swap ----

/** A component whose init is what the compiler emits for `let n = $state(init)` under --hmr. */
const stateComp = (init, text, seen) => {
  function C($$anchor, $$props) {
    $.push($$props, true);
    const n = $.state($n.hmr_keep('n', () => init, () => $.get(n)));
    const items = $.proxy($n.hmr_keep('items', () => [1], () => items));
    seen?.push({ n, items });
    label($$anchor, () => `${text}:${$.get(n)}:${items.length}`);
    $.pop();
    return {};
  }
  return C;
};

/** Mounts each wrapped component inside one window > box. */
function mountAll(...Ws) {
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const b = $n.element('box');
    const c = $n.slot(b, null);
    for (const W of Ws) W(c, {});
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $.pop();
  });
}

test('H3: a changed $state survives a swap, per instance; an untouched one runs the new initializer', () => {
  const th = harness();
  const seen = [];
  const W = $n.hmr(stateComp(0, 'a', seen), 'S.svelte', 'sig1');
  mountAll(W, W);
  $.set(seen[0].n, 5);
  seen[0].items.push(2);
  // the second instance's n stays at its initial value: only changed values are carried (SV36)
  const before = $n.hmrStats.statesKept;
  W[$n.HMR].update($n.hmr(stateComp(100, 'b', seen), 'S.svelte', 'sig1'));
  $.flush();
  assert.equal(seen.length, 4);
  const next = seen.slice(2);
  const changed = next.find((x) => $.get(x.n) === 5);
  assert.ok(changed, 'changed primitive is kept');
  assert.deepEqual([...changed.items], [1, 2], 'proxy contents are kept');
  assert.ok(next.some((x) => $.get(x.n) === 100 && x.items.length === 1), 'untouched instance takes the new initializer');
  assert.equal($n.hmrStats.statesKept - before, 3, 'n and items of the first instance, items of the second (objects are always carried)');
  assert.ok(th.lines.some((l) => l.includes('b:5:2')), th.lines.join('\n'));
});

test('H3: a different signature, null (@hmr:reset) or the global switch start fresh', () => {
  harness();
  for (const [sigB, off] of [['sig2', false], [null, false], ['sig1', true]]) {
    const seen = [];
    const A = $n.hmr(stateComp(0, 'a', seen), 'S.svelte', 'sig1');
    mountAll(A);
    $.set(seen[0].n, 7);
    if (off) $n.setHmrPreserveState(false);
    try {
      A[$n.HMR].update($n.hmr(stateComp(1, 'b', seen), 'S.svelte', sigB));
      $.flush();
    } finally {
      $n.setHmrPreserveState(true);
    }
    assert.equal($.get(seen[1].n), 1, `sig ${sigB} off=${off}`);
  }
});

test('H3: a throwing new version rolls back and keeps the state for the next good one', () => {
  harness();
  const seen = [];
  const A = $n.hmr(stateComp(0, 'a', seen), 'S.svelte', 'sig1');
  mountAll(A);
  $.set(seen[0].n, 3);
  $n.setHmrReporter(() => {});
  const bad = function Bad($$anchor, $$props) {
    $.push($$props, true);
    $n.hmr_keep('n', () => 0, () => 0);
    throw new Error('boom');
  };
  A[$n.HMR].update($n.hmr(bad, 'S.svelte', 'sig1'));
  $.flush();
  A[$n.HMR].update($n.hmr(stateComp(0, 'c', seen), 'S.svelte', 'sig1'));
  $.flush();
  $n.setHmrReporter(null);
  assert.equal($.get(seen[seen.length - 1].n), 3);
});

test('H3: module state — values and store objects survive a re-evaluation with the same signature', () => {
  const hot = { data: {} };
  const evalModule = (sig, init) => {
    const m = $n.hmr_module(hot, sig);
    let count;
    const c = $.state(m.keep('count', () => init, () => $.get(c)));
    const store = m.keep_object('store', () => ({ v: init }));
    m.commit();
    return { c, store };
  };
  const v1 = evalModule('s', 0);
  $.set(v1.c, 4);
  const v2 = evalModule('s', 0);
  assert.equal($.get(v2.c), 4);
  assert.equal(v2.store, v1.store, 'same store object');
  const v3 = evalModule('other', 0);
  assert.equal($.get(v3.c), 0);
  assert.notEqual(v3.store, v2.store);
});

// ---- H3c: children of an edited parent keep their state (positional keys) ----

test('H3c: editing a parent keeps its children\'s changed state by position', () => {
  const th = harness();
  const seen = [];
  const Child = $n.hmr(stateComp(0, 'k', seen), 'Child.svelte', 'csig');
  const parent = (tag) => {
    function P($$anchor, $$props) {
      $.push($$props, true);
      label($$anchor, tag);
      Child($$anchor, {});
      Child($$anchor, {});
      $.pop();
      return {};
    }
    return P;
  };
  const P = $n.hmr(parent('p1'), 'Parent.svelte');
  mountAll(P);
  assert.equal(seen.length, 2);
  $.set(seen[1].n, 9); // the second child changed, the first did not
  seen[1].items.push(7);
  P[$n.HMR].update($n.hmr(parent('p2'), 'Parent.svelte'));
  $.flush();
  assert.equal(seen.length, 4, 'both children were re-created by the parent swap');
  assert.equal($.get(seen[2].n), 0, 'the unchanged first child starts from its initializer');
  assert.equal($.get(seen[3].n), 9, 'the second child kept its changed state');
  assert.deepEqual([...seen[3].items], [1, 7]);
  assert.ok(th.lines.some((l) => l.includes('k:9:2')), th.lines.join('\n'));
});

test('H3c: the global switch resets the children of an edited parent', () => {
  harness();
  for (const off of [false, true]) {
    const seen = [];
    const Child = $n.hmr(stateComp(0, 'k', seen), 'Child.svelte', 'csig');
    const parent = () => function P($$anchor, $$props) {
      $.push($$props, true);
      Child($$anchor, {});
      $.pop();
      return {};
    };
    const P = $n.hmr(parent(), 'Parent.svelte');
    mountAll(P);
    $.set(seen[0].n, 4);
    if (off) $n.setHmrPreserveState(false);
    try {
      P[$n.HMR].update($n.hmr(parent(), 'Parent.svelte'));
      $.flush();
    } finally {
      $n.setHmrPreserveState(true);
    }
    assert.equal($.get(seen[1].n), off ? 0 : 4);
  }
});
