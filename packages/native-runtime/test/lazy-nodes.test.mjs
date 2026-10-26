// W-C3 (plans/research/skelter/compiler-and-js.md §3.3): runtime nodes carry every field as their
// own (no read falls through to the provider-method proxy) and create their collections on first
// use; `setp` is the typed scalar write, deduped in JS unless the host dedupes (`host.dedupes`).
// Run: node --import ./test/register.mjs --test test/lazy-nodes.test.mjs
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

const LAZY = ['last', 'captured', 'tokens', 'handlers', 'listening', 'cattrs', 'passive', 'slots', 'requested', 'snippets'];

test('a fresh node has no collections, and every field is its own', () => {
  harness();
  let label;
  mountInWindow((c) => {
    label = $n.element('label');
    $n.append(c, label);
  });
  for (const f of LAZY) {
    assert.ok(Object.hasOwn(label, f), `${f} is an own field`);
    assert.equal(label[f], null, `${f} is not allocated`);
  }
  for (const f of ['inertFlag', 'anchorScope', 'explicit', 'ruled', 'src', 'vis', 'styleDecl']) assert.ok(Object.hasOwn(label, f), `${f} is an own field`);
  assert.deepEqual(Object.keys(label.attrs), []);
  assert.ok(Object.isFrozen(label.attrs), 'attrs is the shared empty object until a write');
});

test('collections appear on first use and behave as before', () => {
  const t = harness();
  let label;
  mountInWindow((c) => {
    label = $n.element('label');
    $n.set(label, 'label', 'a');
    $n.on(label, 'notify', () => {});
    $n.attr(label, 'class', 'big');
    $n.append(c, label);
  });
  assert.ok(label.last instanceof Map && label.handlers instanceof Map && label.listening instanceof Set);
  assert.equal(label.attrs.class, 'big');
  assert.equal(label.slots, null, 'a leaf never gets slots');
  t.take();
  $n.set(label, 'label', 'a');
  assert.deepEqual(t.take(), [], 'an unchanged scalar is still dropped in JS');
  $n.set(label, 'label', 'b');
  assert.deepEqual(t.take(), ['set #2 label "b"']);
});

test('setp writes typed scalars and dedupes in JS when the host does not', () => {
  const t = harness();
  let label;
  mountInWindow((c) => {
    label = $n.element('label');
    $n.append(c, label);
  });
  t.take();
  $n.setp(label, 'label', 'x');
  $n.setp(label, 'label', 'x');
  $n.setp(label, 'label', 'y');
  assert.deepEqual(t.take(), ['set #2 label "x"', 'set #2 label "y"']);
});

test('setp leaves dedupe to a host that dedupes and calls its setp for an id', () => {
  const t = harness();
  let label;
  mountInWindow((c) => {
    label = $n.element('label');
    $n.append(c, label);
  });
  t.take();
  const calls = [];
  const host = label.host;
  label.host = Object.create(host, { dedupes: { value: true }, setp: { value: (h, p, v) => calls.push([h, p, v]) } });
  $n.setp(label, 7, 'x');
  $n.setp(label, 7, 'x');
  $n.setp(label, 'label', 'z');
  assert.deepEqual(calls, [[label.h, 7, 'x'], [label.h, 7, 'x']], 'ids go to host.setp, no JS dedupe');
  assert.equal(label.last, null, 'nothing kept in JS');
  assert.deepEqual(t.take(), ['set #2 label "z"'], 'a name goes to setProp');
});

test('a dead node takes no setp', () => {
  const t = harness();
  let label;
  mountInWindow((c) => {
    label = $n.element('label');
    $n.append(c, label);
  });
  t.take();
  label.destroyed = true;
  $n.setp(label, 'label', 'x');
  assert.deepEqual(t.take(), []);
});
