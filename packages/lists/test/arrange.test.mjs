import test from 'node:test';
import assert from 'node:assert/strict';
import { arrange } from '../src/arrange.js';

const fruit = [
  { name: 'pear', kind: 'Fruit' },
  { name: 'leek', kind: 'Vegetables' },
  { name: 'apple', kind: 'Fruit' },
  { name: 'beet', kind: 'Vegetables' },
];

test('filter, sort and group: a header only on the first row of each group', () => {
  const rows = arrange(fruit, {
    filter: (f) => f.name !== 'beet',
    compare: (a, b) => a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name),
    group: (f) => f.kind,
  });
  assert.deepEqual(
    rows.map((r) => [r.item.name, r.header]),
    [['apple', 'Fruit'], ['pear', ''], ['leek', 'Vegetables']],
  );
});

test('without options the order is kept, no headers, and the input is not mutated', () => {
  const copy = fruit.slice();
  const rows = arrange(fruit, { compare: (a, b) => a.name.localeCompare(b.name) });
  assert.deepEqual(rows.map((r) => r.item.name), ['apple', 'beet', 'leek', 'pear']);
  assert.ok(rows.every((r) => r.header === ''));
  assert.deepEqual(fruit, copy);
  assert.deepEqual(arrange(fruit).map((r) => r.item.name), ['pear', 'leek', 'apple', 'beet']);
});
