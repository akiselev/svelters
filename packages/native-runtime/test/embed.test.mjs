// EM1: `@svelters/native-runtime/embed` (`__svelters_embed`): roots mounted into host-created targets,
// reactive props (shallow merge, `$$delete`), the per-root boundary (`rootFault`), per-root animation
// frames held while the root is hidden, `root()` info and close handlers. Run with the package's
// `pnpm test`; crates/svelters-egui/tests/embed.rs runs the same module on QuickJS and egui.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';

// A frame clock of our own: embed.js wraps the global requestAnimationFrame when it loads.
let frameQueue = [];
globalThis.requestAnimationFrame = (cb) => frameQueue.push(cb);
globalThis.cancelAnimationFrame = () => {};
const runFrame = () => {
  const q = frameQueue;
  frameQueue = [];
  for (const cb of q) cb(0);
};

const th = createTestHost();
const reports = [];
globalThis.__svelters_host = Object.assign(th.host, {
  rootFault: (id, message) => reports.push(['fault', id, message]),
  rootInfo: (id, title, dirty) => reports.push(['info', id, title, dirty]),
  rootEmit: (id, name, json) => reports.push(['emit', id, name, JSON.parse(json)]),
});
const { root } = await import('../src/embed.js');
const api = globalThis.__svelters_embed;

let seen = 0;
const take = () => {
  const out = th.lines.slice(seen);
  seen = th.lines.length;
  return out;
};

/** A label showing `text()`. */
const label = (anchor, text) => {
  const l = $n.element('label');
  $.template_effect(() => $n.set(l, 'label', text()));
  $n.append(anchor, l);
  return l;
};

test('roots: reactive props, root() info, close handlers, unmount', () => {
  const target = th.host.create('box');
  let handle;
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    handle = root();
    handle.title = 'T';
    handle.onCloseRequest(() => !handle.dirty);
    label($$anchor, () => `${$$props.title ?? 'none'} ${handle.visible ? 'shown' : 'hidden'}`);
    $.pop();
  };
  take();
  api.mount(Comp, 7, target, { title: 'a' });
  assert.ok(take().some((l) => /label "a shown"/.test(l)));
  assert.equal(handle.id, 7);
  assert.deepEqual(reports.splice(0), [['info', 7, 'T', null]]);

  api.props(7, { title: 'b' });
  assert.ok(take().some((l) => /label "b shown"/.test(l)), 'props are reactive');
  api.props(7, { $$delete: ['title'] });
  assert.ok(take().some((l) => /label "none shown"/.test(l)), '$$delete removes a key');
  api.info(7, { visible: false, key: 'k' });
  assert.ok(take().some((l) => /label "none hidden"/.test(l)), 'info fields are reactive');
  assert.equal(handle.key, 'k');

  assert.equal(api.close(7), true);
  handle.dirty = true;
  assert.equal(api.close(7), false, 'a close handler keeps it open');
  handle.emit('saved', { n: 1 });
  assert.deepEqual(reports.splice(0), [['info', 7, null, true], ['emit', 7, 'saved', { n: 1 }]]);

  api.unmount(7);
  assert.ok(take().some((l) => /^destroy/.test(l)), 'its natives are destroyed');
  assert.deepEqual(api.roots(), []);
});

test('roots: an effect error after mount reaches the root boundary, not the caller', () => {
  const target = th.host.create('box');
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    label($$anchor, () => {
      if ($$props.n > 0) throw new Error('later');
      return 'fine';
    });
    $.pop();
  };
  api.mount(Comp, 8, target, { n: 0 });
  api.props(8, { n: 1 });
  assert.deepEqual(reports.splice(0), [['fault', 8, 'later']]);
  api.unmount(8);
  // a first render that throws fails the mount itself
  const Bad = () => {
    throw new Error('first');
  };
  assert.throws(() => api.mount(Bad, 9, target, {}), /first/);
  assert.deepEqual(api.roots(), []);
});

test('roots: animation frames are per root and wait while it is hidden', () => {
  const target = th.host.create('box');
  let ticks = 0;
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    $.user_effect(() => {
      $$props.go;
      requestAnimationFrame(() => ticks++);
    });
    label($$anchor, () => 'x');
    $.pop();
  };
  api.mount(Comp, 10, target, { go: 0 });
  runFrame();
  assert.equal(ticks, 1);
  api.info(10, { visible: false });
  api.props(10, { go: 1 }); // requests a frame while hidden
  runFrame();
  assert.equal(ticks, 1, 'held while hidden');
  api.info(10, { visible: true });
  runFrame();
  assert.equal(ticks, 2, 'runs once shown');
  api.unmount(10);
});

test('roots: a handler runs in its root: root() and animation frames held while hidden (EM2)', () => {
  const target = th.host.create('box');
  let ticks = 0;
  let btn;
  let seenRoot;
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    btn = $n.element('button');
    $n.on(btn, 'clicked', () => {
      seenRoot = root().id;
      requestAnimationFrame(() => ticks++);
    });
    $n.append($$anchor, btn);
    $.pop();
  };
  api.mount(Comp, 11, target, {});
  runFrame();
  globalThis.__svelters_dispatch(btn.h, 'clicked', []);
  assert.equal(seenRoot, 11, 'root() inside a handler is its root');
  runFrame();
  assert.equal(ticks, 1, 'a handler frame runs while shown');
  api.info(11, { visible: false });
  globalThis.__svelters_dispatch(btn.h, 'clicked', []);
  runFrame();
  assert.equal(ticks, 1, "a hidden root's handler frame waits");
  api.info(11, { visible: true });
  runFrame();
  assert.equal(ticks, 2, 'and runs once shown');
  api.unmount(11);
});

test('roots: retarget moves where new natives go (EM2 move_root)', () => {
  const a = th.host.create('box');
  const b = th.host.create('box');
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    $n.if_block($$anchor, () => $$props.more, ($$anchor) => label($$anchor, () => 'more'));
    $.pop();
  };
  api.mount(Comp, 12, a, { more: false });
  take();
  api.retarget(12, { target: b });
  api.props(12, { more: true });
  const lines = take();
  assert.ok(lines.some((l) => l.startsWith(`insert #${b} `)), lines.join('\n'));
  assert.ok(!lines.some((l) => l.startsWith(`insert #${a} `)), lines.join('\n'));
  api.unmount(12);
});
