// EV1: the DOM-style events a backend dispatches along its own tree (egui), one call_sync per node
// and phase: the event object gets target, currentTarget, eventPhase and the per-event fields, and
// the answer is a bit set (1 stopPropagation, 2 preventDefault, 4 stopImmediatePropagation).
// Run: node --import ./test/register.mjs --test test/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

function mountInWindow(body) {
  globalThis.__svelters_host = createTestHost().host;
  return $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    body($n.slot(w, null), w);
    $n.append($$anchor, w);
    $.pop();
  });
}

test('EV1: propagating events carry the DOM fields and answer stop/prevent/immediate bits', () => {
  const got = [];
  let b, w;
  mountInWindow((c, win) => {
    w = win;
    b = $n.element('button');
    $n.on(
      w,
      'click',
      (ev) => {
        got.push(['capture', ev.target === b, ev.currentTarget === w, ev.eventPhase, ev.x, ev.y, ev.button, ev.shiftKey, ev.bubbles]);
        ev.stopPropagation();
      },
      $n.LISTEN_CAPTURE,
    );
    $n.on(b, 'click', (ev) => {
      got.push(['target', ev.eventPhase, ev.cancelable, ev.defaultPrevented]);
      ev.preventDefault();
      ev.stopImmediatePropagation();
    });
    $n.on(b, 'click', () => got.push('never'));
    $n.on(b, 'keydown', (ev) => got.push(['key', ev.key, ev.repeat, ev.ctrlKey, ev.altKey]));
    $n.on(b, 'focusin', (ev) => got.push(['focus', ev.relatedTarget === w, ev.cancelable]));
    $n.append(c, b);
  });
  $.flush();
  const sync = globalThis.__svelters_dispatch_sync;
  // the window (#1) in the capture phase, for the button (#2): stopPropagation only
  assert.equal(sync(1, 'click', [2, 1, 10, 20, 0, 1], $n.LISTEN_CAPTURE), 1);
  // at the target: preventDefault + stopImmediatePropagation (which also stops)
  assert.equal(sync(2, 'click', [2, 2, 10, 20, 0, 0]), 1 | 2 | 4);
  // already prevented (+8) and not cancelable (+16): the flags read so, preventDefault is ignored
  assert.equal(sync(2, 'click', [2, 2 | 8 | 16, 10, 20, 0, 0]), 1 | 4);
  assert.equal(sync(2, 'keydown', [2, 2, 'A', true, 2]), 0);
  assert.equal(sync(2, 'focusin', [2, 2 | 16, 1]), 0);
  assert.deepEqual(got, [
    ['capture', true, true, 1, 10, 20, 0, true, true],
    ['target', 2, true, false],
    ['target', 2, false, false],
    ['key', 'A', true, true, false],
    ['focus', true, false],
  ]);
});

test('EV1: a deferred delivery of a propagating event is not cancelable', () => {
  const got = [];
  let b;
  mountInWindow((c) => {
    b = $n.element('button');
    $n.on(b, 'click', (ev) => {
      ev.preventDefault();
      got.push([ev.target === b, ev.cancelable, ev.defaultPrevented]);
    });
    $n.append(c, b);
  });
  $.flush();
  globalThis.__svelters_dispatch(2, 'click', [2, 2, 0, 0, 0, 0], true);
  assert.deepEqual(got, [[true, false, false]]);
});
