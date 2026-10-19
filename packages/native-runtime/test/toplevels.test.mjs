// X7 toplevels and attached kinds on the native runtime (RT1: what @svelters/renderer did for the
// stock pipeline): a toplevel declared inside an element is transient for the enclosing toplevel and
// shown after it, by its `show` policy; an attached kind (event controller, context menu) is
// inserted into its parent element with no slot and never used as a `before` sibling.
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

test('a nested toplevel is inserted into the enclosing toplevel and shown after it', () => {
  const t = harness();
  const open = $.state(true);
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.toplevel('window', 'present');
    const b = $n.element('box');
    $n.if_block($n.slot(b, null), () => $.get(open), (anchor) => {
      const child = $n.toplevel('window', 'present');
      $n.append(anchor, child);
    });
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $.pop();
  });
  assert.deepEqual(t.take(), [
    'create window #1',
    'create box #2',
    'create window #3',
    'insert #1 #2 before=end slot=default',
    'insert #1 #3 before=end slot=default',
    'call #1 present []',
    'call #3 present []',
  ]);
  $.set(open, false);
  $.flush();
  assert.deepEqual(t.take(), ['remove #1 #3', 'destroy #3']);
  $.set(open, true);
  $.flush();
  // the enclosing window is shown already: insert and show at once
  assert.deepEqual(t.take(), ['create window #4', 'insert #1 #4 before=end slot=default', 'call #4 present []']);
});

test('show policy: none is not shown, a dialog defaults to modal (modal set, then its open method)', () => {
  const t = harness();
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.toplevel('window', 'present');
    $n.append($$anchor, w);
    $n.append($$anchor, $n.toplevel('about-dialog', 'present', 'none'));
    $n.append($$anchor, $n.toplevel('about-dialog', 'present'));
    $.pop();
  });
  assert.deepEqual(t.take(), [
    'create window #1',
    'call #1 present []',
    'create about-dialog #2',
    'create about-dialog #3',
    'set #3 modal true',
    'call #3 present []',
  ]);
});

test('attached kinds: inserted without a slot, never the `before` of a sibling', () => {
  const t = harness();
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.toplevel('window', 'present');
    const b = $n.element('box');
    const c = $n.slot(b, null);
    const ctl = $n.attached('gesture-click');
    $n.append(c, ctl);
    const r = $n.region(c);
    $n.append(c, $n.element('label'));
    $n.append(r, $n.element('button')); // goes before the label, not before the controller
    $n.append($n.slot(w, null), b);
    $n.append($$anchor, w);
    $.pop();
  });
  assert.deepEqual(t.take(), [
    'create window #1',
    'create box #2',
    'create gesture-click #3',
    'insert #2 #3 before=end slot=default',
    'create label #4',
    'insert #2 #4 before=end slot=default',
    'create button #5',
    'insert #2 #5 before=#4 slot=default',
    'insert #1 #2 before=end slot=default',
    'call #1 present []',
  ]);
});
