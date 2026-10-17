// String attributes are coerced from the host's property descriptors, not from a list in the DOM shim.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createDom } from '../src/index.js';

function fakeHost() {
  const sets = [];
  let n = 0;
  const host = {
    kindId: (tag) => tag.length,
    propId: (_kind, name) => ({ spacing: 1, opacity: 2, enabled: 3 })[name] ?? 4,
    propType: (_kind, prop) => ({ 1: 'integer', 2: 'number', 3: 'boolean' })[prop] ?? 'string',
    create: () => ++n,
    setProp: (h, prop, value) => sets.push([prop, value]),
    insert() {},
    remove() {},
    destroy() {},
    listen() {},
    call() {},
    getProp() {},
  };
  return { host, sets };
}

test('attributes are coerced by the descriptor type of the prop', () => {
  const { host, sets } = fakeHost();
  const dom = createDom(host);
  const win = dom.document.createElement('window');
  const box = dom.document.createElement('box');
  box.setAttribute('spacing', '8');
  box.setAttribute('opacity', '0.5');
  box.setAttribute('enabled', '');
  box.setAttribute('name', '8');
  win.appendChild(box);
  dom.root.appendChild(win);
  assert.deepEqual(sets, [
    ['spacing', 8],
    ['opacity', 0.5],
    ['enabled', true],
    ['name', '8'],
  ]);
});

test('a host without descriptors gets the attribute text unchanged', () => {
  const { host, sets } = fakeHost();
  delete host.propType;
  const dom = createDom(host);
  const win = dom.document.createElement('window');
  const box = dom.document.createElement('box');
  box.setAttribute('spacing', '8');
  win.appendChild(box);
  dom.root.appendChild(win);
  assert.deepEqual(sets, [['spacing', '8']]);
});
