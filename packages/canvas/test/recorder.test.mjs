import test from 'node:test';
import assert from 'node:assert/strict';
import { Canvas2D } from '../src/recorder.js';

test('calls record [name, ...args] in order', () => {
  const c = new Canvas2D(10, 20);
  c.fillStyle = '#f00';
  c.fillRect(0, 0, 5, 5);
  c.save();
  c.translate(1, 2);
  c.beginPath();
  c.arc(3, 3, 2, 0, Math.PI, true);
  c.closePath();
  c.stroke();
  c.restore();
  assert.deepEqual(c.ops, [
    ['fillStyle', '#f00'],
    ['fillRect', 0, 0, 5, 5],
    ['save'],
    ['translate', 1, 2],
    ['beginPath'],
    ['arc', 3, 3, 2, 0, Math.PI, true],
    ['closePath'],
    ['stroke'],
    ['restore'],
  ]);
  assert.equal(c.width, 10);
  assert.equal(c.height, 20);
});

test('style properties read back what was set and record each set', () => {
  const c = new Canvas2D();
  assert.equal(c.lineWidth, undefined);
  c.lineWidth = 3;
  c.font = 'bold 12px sans-serif';
  c.textAlign = 'center';
  c.globalAlpha = 0.5;
  assert.equal(c.lineWidth, 3);
  assert.deepEqual(c.ops, [['lineWidth', 3], ['font', 'bold 12px sans-serif'], ['textAlign', 'center'], ['globalAlpha', 0.5]]);
});

test('toJSON is the native ops string; clear empties the list; line dash copies its argument', () => {
  const c = new Canvas2D();
  const dash = new Float64Array([4, 2]);
  c.setLineDash(dash);
  c.fillText(42, 1, 2);
  assert.equal(c.toJSON(), '[["setLineDash",[4,2]],["fillText","42",1,2]]');
  c.clear();
  assert.equal(c.toJSON(), '[]');
});
