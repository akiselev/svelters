// EV2b (ARCH-EMBED §8.5): `nativeBehavior`, the Svelte action the generated `svelters:behaviors/<ns>`
// modules export. `use:pick={params}` creates the behaviour's attached node, sets its params as props,
// inserts it into the element with no slot or position, and listens to the events whose `on<event>`
// callback the params carry. An update writes only the params that changed (one `set_prop` each,
// arrays by content) and swaps callbacks with no host op; destroy destroys the node.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

function harness() {
  const th = createTestHost();
  globalThis.__svelters_host = th.host;
  return { th, take: () => th.lines.splice(0) };
}

test('use: a native behaviour: one attached node, changed params only, callbacks without ops', () => {
  const t = harness();
  const pick = $n.nativeBehavior('cad.pick', ['picked']);
  let label;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.toplevel('window', 'present');
    label = $n.element('label');
    $n.append($n.slot(w, null), label);
    $n.append($$anchor, w);
    $.pop();
  });
  t.take();
  const got = [];
  const a = pick(label, { accepts: ['mcad.face'], active: true, onpicked: (r) => got.push(['first', r]) });
  assert.deepEqual(t.take(), [
    'create cad.pick #3',
    'set #3 accepts ["mcad.face"]',
    'set #3 active true',
    'listen #3 picked on',
    'insert #2 #3 before=end slot=default',
  ]);
  // an update with a fresh but equal array, the same flag and a new callback: one set_prop
  a.update({ accepts: ['mcad.face'], active: false, onpicked: (r) => got.push(['second', r]) });
  assert.deepEqual(t.take(), ['set #3 active false']);
  $n.dispatch(3, 'picked', [{ kind: 'mcad.face', label: 'Face' }]);
  assert.deepEqual(got, [['second', { kind: 'mcad.face', label: 'Face' }]]);
  // a param that goes away is reset; a dropped callback stops delivering (the connection stays)
  a.update({ accepts: ['mcad.edge'] });
  assert.deepEqual(t.take(), ['set #3 accepts ["mcad.edge"]', 'set #3 active null']);
  $n.dispatch(3, 'picked', [{}]);
  assert.equal(got.length, 1);
  a.destroy();
  assert.deepEqual(t.take(), ['destroy #3']);
  a.destroy();
  assert.deepEqual(t.take(), []);
});
