// W-C1 and W-C2 (presto, lane COMPILER): the template expansion writes the same op log as the
// per-node ops the compiler writes without templates, and module ids are checked against the
// host before their first use. Run with the package's `pnpm test`.
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

const branch = (anchor) => {
  const l = $n.element('label');
  $n.set(l, 'label', 'in the block');
  $n.append(anchor, l);
};

/** window > box > [button "a" (clicked), {#if}, label {value}], as the compiler writes it per node. */
function perNode(value, onclick) {
  return $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const b = $n.element('box');
    const a = $n.element('button');
    $n.set(a, 'label', 'a');
    $n.append($n.slot(b, null), a);
    $n.if_block($n.slot(b, null), () => true, branch);
    const v = $n.element('label');
    $n.set(v, 'label', value);
    $n.append($n.slot(b, null), v);
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $n.on(a, 'clicked', onclick);
    $.pop();
  });
}

/** The same tree from a template table. */
function fromTemplate(value, onclick) {
  const T = $n.template(
    'test-template',
    [
      ['box', null, [], [1, [1, null], 2], null],
      ['button', null, [['set', 'label', 'a']], [], null],
      ['label', null, [['setv', 'label', 0]], [], null],
    ],
    [[1, 'clicked', 0]],
  );
  return $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const t = $n.instantiate(T, $n.slot(w, null), [() => value, ($$c) => $n.if_block($$c, () => true, branch)]);
    $n.append($$anchor, w);
    $n.template_listen(t, [onclick]);
    $.pop();
  });
}

test('a template instance writes the op log of its per-node ops', () => {
  const h = () => {};
  const a = harness();
  perNode('v', h);
  const want = a.lines.slice();
  const b = harness();
  fromTemplate('v', h);
  assert.deepEqual(b.lines, want);
  assert.ok(want.some((l) => l.includes('listen')), 'the listener is registered');
});

test('at addresses an instance node by its pre-order number', () => {
  harness();
  const T = $n.template('test-at', [['box', null, [], [1], null], ['label', null, [], [], null]], []);
  let label;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const t = $n.instantiate(T, $n.slot(w, null));
    label = $n.at(t, 1);
    $n.append($$anchor, w);
    $.pop();
  });
  assert.equal(label.kind, 'label');
});

test('ids: element_id and setp write by name after the ids match the host', () => {
  const th = harness();
  const K = th.host.kindId('label');
  const P = th.host.propId(K, 'label');
  $n.ids('testhost', 'hash-ok', [[K, 'label'], [P, 'label', 'label']]);
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const l = $n.element_id(K);
    $n.setp(l, P, 'by id');
    $n.append($n.slot(w, null), l);
    $n.append($$anchor, w);
    $.pop();
  });
  assert.ok(th.lines.some((l) => l === 'set #2 label "by id"'), th.lines.join('\n'));
});

test('ids: a module compiled for other ids is refused', () => {
  const th = harness();
  const K = th.host.kindId('label');
  $n.ids('testhost', 'hash-other', [[K + 1000, 'label']]);
  assert.throws(() => $n.element_id(K + 1000), /compiled for testhost spec hash-other/);
});
