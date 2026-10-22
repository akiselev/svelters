// Spreads on native elements and <svelte:element> (SP1, runtime-contract.md "Spread attributes")
// against the Node TestHost, with a hand-written spec table in the shape `svelters-compile --kinds`
// writes. Run: node --import ./test/register.mjs --test test/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

// flags: 1 writable, 2 readable, 4 nullable, 8 string_form
const K = {
  backend: 'gtk',
  aliases: { click: 'clicked' },
  tags: { window: 'Gtk.Window', box: 'Gtk.Box', button: 'Gtk.Button', label: 'Gtk.Label', entry: 'Gtk.Entry' },
  kinds: {
    'Gtk.Widget': {
      c: ['Gtk.Widget'],
      p: {
        visible: ['visible', 3, 'bool', true, 0, 0],
        'tooltip-text': ['tooltip-text', 7, 'string', null, 0, 0],
        'width-request': ['width-request', 3, 'i32', -1, 0, 0],
        halign: ['halign', 3, 'enum', 'fill', ['fill', 'start', 'end', 'center'], 0],
        'scale-factor': ['scale-factor', 2, 'i32', 1, 0, 0],
      },
      e: { destroy: [0, 0, ['after']], 'key-pressed': [0, 1, ['after', 'capture']] },
    },
    'Gtk.Window': { c: ['Gtk.Window', 'Gtk.Widget'], p: { title: ['title', 7, 'string', null, 0, 0] }, e: {}, top: ['present'] },
    'Gtk.Box': { c: ['Gtk.Box', 'Gtk.Widget'], p: { spacing: ['spacing', 3, 'i32', 0, 0, 0] }, e: {} },
    'Gtk.Button': { c: ['Gtk.Button', 'Gtk.Widget'], p: { label: ['label', 7, 'string', null, 0, 0] }, e: { clicked: [0, 0, ['after']] }, t: 'label' },
    'Gtk.Label': { c: ['Gtk.Label', 'Gtk.Widget'], p: { label: ['label', 3, 'string', '', 0, 0] }, e: {}, t: 'label' },
    'Gtk.Entry': { c: ['Gtk.Entry', 'Gtk.Widget'], p: { text: ['text', 3, 'string', '', 0, ['changed', -1]] }, e: { changed: [0, 0, ['after']] } },
  },
};

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
    const w = $n.toplevel('window', 'present');
    body($n.slot(w, null), w);
    $n.append($$anchor, w);
    $.pop();
  });
}

test('spread: keys in source order (later wins), strings coerced like static attributes, removal resets', () => {
  const t = harness();
  const p = $.state($.proxy({ label: 'from spread', 'width-request': '40', halign: 'center' }));
  let b;
  mountInWindow((c) => {
    b = $n.element('button');
    $n.spread(b, K, () => ({ label: 'explicit', visible: true, ...$.get(p), 'tooltip-text': 'tip' }));
    $n.append(c, b);
  });
  const sets = (lines) => lines.filter((l) => l.startsWith('set #2'));
  assert.deepEqual(sets(t.take()), [
    'set #2 label "from spread"',
    'set #2 visible true',
    'set #2 width-request 40',
    'set #2 halign "center"',
    'set #2 tooltip-text "tip"',
  ]);
  // keys that go away: back to the spec default, or to the value read before the first write
  $.set(p, { label: 'other' });
  $.flush();
  assert.deepEqual(t.take(), ['set #2 label "other"', 'set #2 width-request -1', 'set #2 halign "fill"']);
  $.set(p, {});
  $.flush();
  assert.deepEqual(t.take(), ['set #2 label "explicit"'], 'the explicit attribute before the spread comes back');
});

test('spread: a prop without a spec default is restored to its native value; bad and unknown keys are skipped', () => {
  const t = harness();
  const p = $.state($.proxy({ title: 'T', 'width-request': 'wide', nope: 1, 'scale-factor': 2 }));
  let w;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    w = $n.toplevel('window', 'present');
    t.th.host.setProp(w.h, 'title', 'native');
    $n.spread(w, K, () => ({ ...$.get(p) }));
    $n.append($$anchor, w);
    $.pop();
  });
  assert.deepEqual(t.take().filter((l) => l.startsWith('set')), ['set #1 title "native"', 'set #1 title "T"']);
  $.set(p, {});
  $.flush();
  assert.deepEqual(t.take(), ['set #1 title "native"']);
});

test('spread: on<event> keys connect once, call the current handler, disconnect when removed', () => {
  const t = harness();
  const calls = [];
  const a = (...args) => calls.push(['a', ...args]);
  const bh = (...args) => calls.push(['b', ...args]);
  const h = $.state(a);
  let btn;
  mountInWindow((c) => {
    btn = $n.element('button');
    $n.spread(btn, K, () => ({ onclicked: $.get(h), 'onkey-pressedcapture': $.get(h) === a ? a : undefined }));
    $n.append(c, btn);
  });
  assert.deepEqual(
    t.take().filter((l) => l.startsWith('listen')),
    ['listen #2 clicked on', 'listen #2 key-pressed on capture'],
  );
  $n.dispatch(btn.h, 'clicked', [1]);
  $.set(h, bh);
  $.flush();
  assert.deepEqual(t.take(), ['listen #2 key-pressed off capture'], 'a new handler does not reconnect');
  $n.dispatch(btn.h, 'clicked', [2]);
  assert.deepEqual(calls, [['a', 1], ['b', 2]]);
  $.set(h, null);
  $.flush();
  assert.deepEqual(t.take(), ['listen #2 clicked off']);
});

test('spread: the DOM-style alias resolves as the compiler does; class, style and parent: go their own way', async () => {
  const t = harness();
  const on = $.state(true);
  let calls = 0;
  mountInWindow((c) => {
    const box = $n.element('box');
    const b = $n.element('button');
    $n.spread(b, K, () => ({ onclick: () => calls++, class: ['a', { b: $.get(on) }], 'parent:x': 1, [$n.CLASS]: { c: $.get(on) } }), null, 'svelte-s');
    $n.append($n.slot(box, null), b);
    $n.append(c, box);
  });
  await new Promise((res) => setImmediate(res));
  const ops = t.take();
  assert.ok(ops.includes('listen #3 clicked on'), ops.join('\n'));
  assert.ok(ops.includes('attr #2 #3 x 1'), ops.join('\n'));
  assert.ok(ops.includes('style #3 classes "a b svelte-s c"'), ops.join('\n'));
  $.set(on, false);
  $.flush();
  await new Promise((res) => setImmediate(res));
  const after = t.take();
  assert.ok(after.includes('style #3 classes "a svelte-s"'), after.join('\n'));
});

test('spread: memoized dependencies are not re-evaluated when another key changes', () => {
  harness();
  const n = $.state(0);
  let runs = 0;
  const rest = () => {
    runs++;
    return { visible: true };
  };
  mountInWindow((c) => {
    const l = $n.element('label');
    $n.spread(l, K, ($0) => ({ label: `${$.get(n)}`, ...$0 }), [() => rest()]);
    $n.append(c, l);
  });
  $.set(n, 1);
  $.flush();
  assert.equal(runs, 1);
});

test('dynamic_element: nullish renders nothing, a new tag rebuilds (new first), the same tag keeps the node', () => {
  const t = harness();
  const tag = $.state('label');
  const text = $.state('hi');
  const built = [];
  mountInWindow((c) => {
    $n.dynamic_element(
      c,
      () => $.get(tag),
      (node) => {
        built.push(node.kind);
        $n.spread(node, K, () => ({ visible: true }));
        $.template_effect(() => $n.text(node, K, $.get(text)));
      },
      K,
    );
  });
  assert.deepEqual(t.take().filter((l) => !l.startsWith('call')), [
    'create window #1',
    'create label #2',
    'set #2 visible true',
    'set #2 label "hi"',
    'insert #1 #2 before=end slot=default',
  ]);
  $.set(text, 'ho');
  $.flush();
  assert.deepEqual(t.take(), ['set #2 label "ho"']);
  $.set(tag, 'button');
  $.flush();
  assert.deepEqual(t.take(), ['create button #3', 'set #3 visible true', 'set #3 label "ho"', 'insert #1 #3 before=end slot=default', 'remove #1 #2', 'destroy #2']);
  $.set(tag, null);
  $.flush();
  assert.deepEqual(t.take(), ['remove #1 #3', 'destroy #3']);
  $.set(tag, 'nonsense');
  $.flush();
  assert.deepEqual(t.take(), [], 'an unknown tag renders nothing');
  assert.deepEqual(built, ['label', 'button']);
});
