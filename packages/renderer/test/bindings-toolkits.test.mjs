// bind:<name> on the AppKit and WinUI tags from the generated binding table, against the TestHost.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root, dispatch, BINDINGS } = await import('../src/index.js');

const take = () => th.lines.splice(0).join('\n');
const mount = (tag) => {
  const win = r.createElement('window');
  const e = r.createElement(tag);
  r.insert(win, e, null);
  r.insert(root, win, null);
  take();
  return e;
};

// [tag, bind name, prop, signal, native value, expected value]
const CASES = [
  ['button', 'state', 'state', 'action', 'on', 'on'],
  ['slider', 'double-value', 'double-value', 'action', 0.5, 0.5],
  ['text-field', 'string-value', 'string-value', 'control-text-did-change', 'abc', 'abc'],
  ['check-box', 'is-checked', 'is-checked', 'click', true, true],
  ['text-box', 'text', 'text', 'text-changed', 'hi', 'hi'],
];

for (const [tag, name, prop, signal, native, expected] of CASES) {
  test(`bind:${name} on <${tag}> wires listen, setProp and the native change`, () => {
    const e = mount(tag);
    const h = e.handle;
    const seen = [];
    // a host that publishes signal ids and knows this one (WinUI keeps `click`; the renderer would otherwise map it to `clicked`)
    th.host.signalId = (_kid, n) => (n === signal ? 1 : null);
    const b = r.bind(e, name, () => null, (v) => seen.push(v));
    assert.equal(take(), `listen #${h} ${signal} on`);
    b.update(expected);
    assert.equal(take(), `set #${h} ${prop} ${JSON.stringify(expected)}`);
    b.update(expected);
    assert.equal(take(), '', 'same value: no native write');
    // a user edit: the signal either carries the value (arg0) or the property is read back
    th.host.setProp(h, prop, native);
    take();
    dispatch(h, signal, BINDINGS[tag][name].valueFrom === 'arg0' ? [native] : []);
    assert.deepEqual(seen, [expected]);
    b.destroy();
    assert.equal(take(), `listen #${h} ${signal} off`);
    delete th.host.signalId;
  });
}

test('slider value: Qt and WinUI share one descriptor now that both use native kebab names', () => {
  // valueChanged (Qt) and ValueChanged (WinUI) are both `value-changed` (docs/notes/n1-rename.md), so the
  // toolkits no longer need per-toolkit variants here.
  const d = BINDINGS.slider.value;
  assert.ok(!Array.isArray(d));
  assert.equal(d.signal, 'value-changed');
  assert.deepEqual(d.toolkits, ['qt', 'winui']);
  const e = mount('slider');
  const seen = [];
  const b = r.bind(e, 'value', () => 0, (v) => seen.push(v));
  assert.equal(take(), `listen #${e.handle} value-changed on`);
  dispatch(e.handle, 'value-changed', [3]);
  assert.deepEqual(seen, [3]);
  b.destroy();
});
