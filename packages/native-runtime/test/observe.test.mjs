// ABI v2.6 `observe` on the Node TestHost: refcounted per (handle, prop), logged as
// `observe #h prop on|off` on the transitions only (like the QuickJS host in front of the log backend).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createTestHost } from '@svelters/testhost';

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
