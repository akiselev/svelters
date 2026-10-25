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

test('ids: element_id creates what element does by name, once the ids match the host', () => {
  const th = harness();
  const K = th.host.kindId('label');
  $n.ids('testhost', 'hash-ok', [[K, 'label']]);
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    const l = $n.element_id(K);
    $n.set(l, 'label', 'by id');
    $n.append($n.slot(w, null), l);
    $n.append($$anchor, w);
    $.pop();
  });
  assert.ok(th.lines.includes('create label #2') && th.lines.includes('set #2 label "by id"'), th.lines.join('\n'));
});

test('ids: a module compiled for other ids is refused', () => {
  const th = harness();
  const K = th.host.kindId('label');
  assert.throws(() => $n.ids('testhost', 'hash-other', [[K + 1000, 'label']]), /compiled for testhost spec hash-other/);
});

test('passive listeners share the connection key; one that can cancel re-listens without the flag', () => {
  const th = harness();
  let b;
  const calls = [];
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    b = $n.element('button');
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $n.on(b, 'clicked', () => calls.push('passive'), $n.LISTEN_PASSIVE);
    $.pop();
  });
  $n.dispatch(b.h, 'clicked', []);
  assert.deepEqual(calls, ['passive'], 'dispatch (flags 0) finds the passive listener');
  const before = th.lines.length;
  $n.on(b, 'clicked', () => calls.push('cancels'));
  const relisten = th.lines.slice(before).filter((l) => l.includes('listen'));
  assert.equal(relisten.length, 2, th.lines.slice(before).join('\n'));
  $n.dispatch(b.h, 'clicked', []);
  assert.deepEqual(calls, ['passive', 'passive', 'cancels']);
});

test('on a host that builds templates, an instance is one host.instantiate and the runtime adopts its nodes', () => {
  const th = harness();
  const tables = [];
  const hs = th.host;
  // a host whose backend builds templates: creates, sets and inserts the nodes itself
  globalThis.__svelters_host = Object.assign(Object.create(hs), {
    templates: true,
    template(t) {
      tables.push(t);
      return tables.length - 1;
    },
    instantiate(id) {
      const t = tables[id];
      const handles = [];
      for (let k = 0; k < t.nodes.length / 3; k++) {
        const [kind, parent, slotName] = t.nodes.slice(k * 3, k * 3 + 3);
        handles.push(hs.create(kind));
        if (parent > 0) hs.insert(handles[parent - 1], handles[k], null, slotName ?? undefined);
      }
      for (let i = 0; i < t.props.length; i += 3) hs.setProp(handles[t.props[i]], t.props[i + 1], t.props[i + 2]);
      return handles;
    },
  });
  const T = $n.template('test-native', [['box', null, [['class', 'row']], [1], null], ['label', null, [['set', 'label', 'hi']], [], null]], []);
  let nodes;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    nodes = $n.instantiate(T, $n.slot(w, null));
    $n.append($$anchor, w);
    $.pop();
  });
  assert.equal(tables.length, 1);
  assert.deepEqual(tables[0].props, [1, 'label', 'hi']);
  assert.equal(nodes[1].kind, 'label');
  assert.equal(nodes[1].np, nodes[0], 'the child is recorded in its parent');
  assert.ok(th.lines.includes('insert #1 #2 before=end slot=default'), th.lines.join('\n'));
  // a second instance reuses the registration
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    $n.instantiate(T, $n.slot(w, null));
    $n.append($$anchor, w);
    $.pop();
  });
  assert.equal(tables.length, 1);
});
