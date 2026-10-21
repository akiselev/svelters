// ABI v2.6 on the Node TestHost: `observe` refcounting and the R1 tracker (src/observe.js): reactive
// reads through `trackProp` / `@svelters/types` getters, `observeProp` callbacks, local invalidation of
// our own writes, `peek`, and teardown (REACTIVITY.md sections 5.2, 5.5, 7).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';
import { trackedCount } from '../src/observe.js';
import { NativeObject, observed, observedLocal, setProp, wrapObject } from '@svelters/types/core';

test('observe is refcounted and logs only the 0->1 and 1->0 transitions', () => {
  const th = createTestHost();
  const h = th.host;
  const b = h.create('text-buffer');
  assert.equal(h.observe(b, 'text', true), true);
  assert.equal(h.observe(b, 'text', true), true);
  assert.equal(th.isObserved(b, 'text'), true);
  assert.equal(h.observe(b, 'text', false), true);
  assert.equal(h.observe(b, 'text', false), false);
  assert.equal(h.observe(b, 'text', false), false, 'off without on is a no-op');
  assert.equal(th.isObserved(b, 'text'), false);
  assert.deepEqual(th.lines, [`create text-buffer #${b}`, `observe #${b} text on`, `observe #${b} text off`]);
});

test('destroy and native death drop the observations of a handle', () => {
  const th = createTestHost();
  const h = th.host;
  const a = h.create('label');
  const c = h.create('label');
  h.observe(a, 'label', true);
  h.observe(c, 'label', true);
  h.destroy(a);
  assert.equal(th.isObserved(a, 'label'), false);
  assert.equal(h.observe(a, 'label', false), false, 'off on a dead handle is a no-op');
  th.killNative(c, 'closed');
  assert.equal(th.isObserved(c, 'label'), false);
  assert.equal(th.lines.filter((l) => l.startsWith('observe')).length, 2);
});

test('a native change is delivered only when the value differs from the last one', () => {
  const th = createTestHost();
  const h = th.host;
  const b = h.create('label');
  h.setProp(b, 'label', 'a');
  h.observe(b, 'label', true);
  // No baseline (FU3): the first announcement after observe starts is delivered even when it equals
  // the value written before, then an identical one is dropped.
  assert.deepEqual(th.nativeChange(b, 'label', 'a'), [b, 'label', 'a'], 'first announcement: no baseline');
  assert.equal(th.nativeChange(b, 'label', 'a'), null, 'equal to the last one delivered');
  assert.deepEqual(th.nativeChange(b, 'label', 'b'), [b, 'label', 'b']);
  assert.equal(th.nativeChange(b, 'label', 'b'), null);
  h.setProp(b, 'label', 'c');
  assert.equal(th.nativeChange(b, 'label', 'c'), null, 'the echo of our own write');
  assert.deepEqual(th.nativeChange(b, 'label', [1, { x: 2 }]), [b, 'label', [1, { x: 2 }]]);
  assert.equal(th.nativeChange(b, 'label', [1, { x: 2 }]), null, 'compared deeply');
  assert.ok(th.lines.includes(`changed #${b} label "b"`));
});

/** A wrapper class like the generated ones: `text` exact, `mode` set_only, `name` plain. */
class Thing extends NativeObject {
  static native = 'Test.Thing';
  get text() {
    return observed(this, 'text');
  }
  set text(v) {
    setProp(this, 'text', v);
  }
  get mode() {
    return observedLocal(this, 'mode');
  }
  set mode(v) {
    setProp(this, 'mode', v);
  }
}

function harness() {
  const th = createTestHost();
  globalThis.__svelters_host = th.host;
  /** Runs a scripted native change through the runtime's `__svelters_changed`, then flushes. */
  const native = (h, prop, value) => {
    const batch = th.nativeChange(h, prop, value);
    if (batch) globalThis.__svelters_changed(batch);
    $.flush();
  };
  return { th, h: th.host, native };
}

test('a getter read in an effect re-runs on native changes and our own writes, and stops observing with the effect', async () => {
  const { th, h, native } = harness();
  const raw = h.create('label');
  const t = wrapObject(Thing, raw);
  h.setProp(raw, 'text', 'hello');
  const seen = [];
  const dispose = $.effect_root(() => {
    $.template_effect(() => seen.push(t.text));
  });
  $.flush();
  assert.deepEqual(seen, ['hello']);
  assert.equal(th.isObserved(raw, 'text'), true);
  native(raw, 'text', 'world');
  assert.deepEqual(seen, ['hello', 'world']);
  native(raw, 'text', 'world');
  assert.deepEqual(seen, ['hello', 'world'], 'an equal value is dropped by the host');
  t.text = 'mine';
  $.flush();
  assert.deepEqual(seen, ['hello', 'world', 'mine'], 'our own write invalidates locally');
  native(raw, 'text', 'mine');
  assert.equal(seen.length, 3, 'and its echo is dropped');
  // peek and reads outside effects never subscribe.
  assert.equal(t.peek('text'), 'mine');
  assert.equal(t.text, 'mine');
  dispose();
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(th.isObserved(raw, 'text'), false, 'the last effect gone: observe off');
  assert.equal(trackedCount(h), 0, 'nothing kept per handle');
  assert.ok(th.lines.includes(`observe #${raw} text on`) && th.lines.includes(`observe #${raw} text off`));
});

test('set_only getters re-run on our own writes without a native observation', () => {
  const { th, h } = harness();
  const raw = h.create('label');
  const t = wrapObject(Thing, raw);
  const seen = [];
  const dispose = $.effect_root(() => {
    $.template_effect(() => seen.push(t.mode));
  });
  $.flush();
  t.mode = 'b';
  $.flush();
  assert.deepEqual(seen, [null, 'b']);
  assert.equal(th.lines.filter((l) => l.startsWith('observe')).length, 0);
  dispose();
});

test('peek reads without subscribing inside an effect', () => {
  const { th, h, native } = harness();
  const raw = h.create('label');
  const t = wrapObject(Thing, raw);
  let runs = 0;
  const dispose = $.effect_root(() => {
    $.template_effect(() => {
      runs++;
      t.peek('text');
      t.peek('cursor-position');
    });
  });
  $.flush();
  assert.equal(th.isObserved(raw, 'text'), false);
  native(raw, 'text', 'x');
  assert.equal(runs, 1);
  dispose();
});

test('observeProp calls back once per delivered change and shares one host observation', () => {
  const { th, h, native } = harness();
  const raw = h.create('label');
  const a = [];
  const b = [];
  const offA = $n.observeProp(raw, 'label', (v) => a.push(v));
  const offB = $n.observeProp(raw, 'label', (v) => b.push(v));
  assert.equal(offA.observed, true);
  native(raw, 'label', 'one');
  native(raw, 'label', 'two');
  assert.deepEqual(a, ['one', 'two']);
  assert.deepEqual(b, ['one', 'two']);
  offA();
  offA();
  native(raw, 'label', 'three');
  assert.deepEqual(a, ['one', 'two']);
  assert.deepEqual(b, ['one', 'two', 'three']);
  assert.equal(th.lines.filter((l) => l === `observe #${raw} label on`).length, 1);
  offB();
  assert.equal(th.isObserved(raw, 'label'), false);
  assert.equal(trackedCount(h), 0);
});

// The contract R2's `bind:` lowering relies on (REACTIVITY.md §5.2 set path, §13 #7): our own `setProp` on an
// observed prop is read back into `last`, so its announcement (a scripted `changed` without a value, or the
// toolkit's echo) is dropped and `observeProp` callbacks do not run for it. A native change to a different value
// is delivered.
test('observeProp is not called for the echo of our own setProp, only for a different native value', () => {
  const { th, h, native } = harness();
  const raw = h.create('label');
  const seen = [];
  const off = $n.observeProp(raw, 'label', (v) => seen.push(v));
  h.setProp(raw, 'label', 'mine');
  native(raw, 'label', 'mine');
  assert.deepEqual(seen, [], 'the echo of our write is dropped');
  native(raw, 'label', 'theirs');
  assert.deepEqual(seen, ['theirs']);
  h.setProp(raw, 'label', 'theirs');
  native(raw, 'label', 'theirs');
  assert.deepEqual(seen, ['theirs'], 'writing the current value announces nothing either');
  off();
  void th;
});

test('trackProp spellings share one entry (cursor-position / cursorPosition)', () => {
  const { th, h } = harness();
  const raw = h.create('entry');
  let runs = 0;
  const dispose = $.effect_root(() => {
    $.template_effect(() => {
      runs++;
      $n.trackProp(raw, 'cursor-position');
    });
  });
  $.flush();
  globalThis.__svelters_changed([raw, 'cursorPosition', 3]);
  $.flush();
  assert.equal(runs, 2);
  dispose();
  void th;
});
