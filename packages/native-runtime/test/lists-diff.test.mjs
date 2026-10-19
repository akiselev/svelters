import test from 'node:test';
import assert from 'node:assert/strict';
import { diffKeys, applySplices, lis } from '../src/lists-diff.js';

const range = (n, from = 0) => Array.from({ length: n }, (_, i) => i + from);

function check(a, b) {
  const ops = diffKeys(a, b);
  assert.deepEqual(applySplices(a, ops), b, `${a} -> ${b}: ${JSON.stringify(ops)}`);
  return ops;
}

test('lis', () => {
  assert.deepEqual(lis([]), []);
  assert.equal(lis([3, 1, 2, 0, 4]).length, 3);
  assert.deepEqual(lis([0, 1, 2]), [0, 1, 2]);
});

test('no change produces no ops', () => {
  assert.deepEqual(diffKeys(range(5), range(5)), []);
});

test('append, prepend, insert in the middle, remove: one splice each', () => {
  assert.equal(check(range(5), range(7)).length, 1);
  assert.equal(check(range(5), [9, ...range(5)]).length, 1);
  assert.equal(check(range(5), [0, 1, 9, 8, 2, 3, 4]).length, 1);
  assert.equal(check(range(5), [0, 4]).length, 1);
  assert.equal(check(range(5), []).length, 1);
  assert.equal(check([], range(5)).length, 1);
});

test('two separate removals are two splices, never a reset', () => {
  const ops = check(range(10), [1, 2, 3, 5, 6, 7, 8]);
  assert.equal(ops.length, 3);
  assert.ok(ops.every((o) => o.add.length === 0));
  assert.equal(
    ops.reduce((n, o) => n + o.removed, 0),
    3,
  );
});

test('a single move is one remove and one insert', () => {
  const ops = check(range(10), [0, 1, 2, 4, 5, 6, 7, 3, 8, 9]);
  assert.equal(ops.length, 2);
  assert.equal(ops[0].removed + ops[1].add.length, 2);
});

test('swap of the ends', () => {
  const ops = check(range(10), [9, ...range(8, 1), 0]);
  assert.ok(ops.length <= 4);
});

test('reverse stays a handful of splices', () => {
  const ops = check(range(1000), range(1000).reverse());
  assert.ok(ops.length <= 4, `${ops.length}`);
});

test('randomized', () => {
  let seed = 12345;
  const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;
  for (let t = 0; t < 300; t++) {
    const a = range(Math.floor(rnd() * 30));
    const pool = range(40);
    const b = pool.filter(() => rnd() < 0.5);
    for (let i = b.length - 1; i > 0; i--) {
      const j = Math.floor(rnd() * (i + 1));
      [b[i], b[j]] = [b[j], b[i]];
    }
    check(a, b);
  }
});

test('many scattered changes collapse to one range replace', () => {
  const a = range(100);
  const b = a.filter((k) => k % 2 === 1);
  const ops = check(a, b);
  assert.equal(ops.length, 1);
  assert.deepEqual([ops[0].pos, ops[0].removed], [0, 99]);
  assert.equal(check(a, b.slice(), ).length, 1);
  // under the limit stays exact
  assert.equal(check(range(10), [1, 2, 3, 5, 6, 7, 8]).length, 3);
});
