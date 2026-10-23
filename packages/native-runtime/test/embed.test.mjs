// EM1: `@svelters/native-runtime/embed` (`__svelters_embed`): roots mounted into host-created targets,
// reactive props (shallow merge, `$$delete`), the per-root boundary (`rootFault`), per-root animation
// frames held while the root is hidden, `root()` info and close handlers. Run with the package's
// `pnpm test`; crates/svelters-egui/tests/embed.rs runs the same module on QuickJS and egui.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import * as $ from 'svelte/internal/client';
import { flush_tasks } from 'svelte/internal/client/dom/task';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';
import { subscriber } from '../src/core/internal.js';

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
const { flushing_sync } = await import('../src/core/svelte.js');
const api = globalThis.__svelters_embed;

const nextTask = () => new Promise((r) => setTimeout(r, 0));
/**
 * A host call as svelters-host's `root_call` makes it: the call, then a job drain. The embed's
 * entries do not flush (FX6b): what they changed renders in Svelte's microtask flush, in the drain.
 */
const call = async (name, ...args) => {
  const r = api[name](...args);
  await nextTask();
  return r;
};

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

test('roots: reactive props, root() info, close handlers, unmount', async () => {
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
  await call('mount', Comp, 7, target, { title: 'a' });
  assert.ok(take().some((l) => /label "a shown"/.test(l)));
  assert.equal(handle.id, 7);
  assert.deepEqual(reports.splice(0), [['info', 7, 'T', null]]);

  api.props(7, { title: 'b' });
  assert.ok(!take().some((l) => /label "b shown"/.test(l)), 'the call itself does not flush');
  await nextTask();
  assert.ok(take().some((l) => /label "b shown"/.test(l)), 'props are reactive');
  await call('props', 7, { $$delete: ['title'] });
  assert.ok(take().some((l) => /label "none shown"/.test(l)), '$$delete removes a key');
  await call('info', 7, { visible: false, key: 'k' });
  assert.ok(take().some((l) => /label "none hidden"/.test(l)), 'info fields are reactive');
  assert.equal(handle.key, 'k');

  assert.equal(api.close(7), true);
  handle.dirty = true;
  assert.equal(api.close(7), false, 'a close handler keeps it open');
  handle.emit('saved', { n: 1 });
  assert.deepEqual(reports.splice(0), [['info', 7, null, true], ['emit', 7, 'saved', { n: 1 }]]);

  await call('unmount', 7);
  assert.ok(take().some((l) => /^destroy/.test(l)), 'its natives are destroyed');
  assert.deepEqual(api.roots(), []);
});

test('roots: an effect error after mount reaches the root boundary, not the caller', async () => {
  const target = th.host.create('box');
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    label($$anchor, () => {
      if ($$props.n > 0) throw new Error('later');
      return 'fine';
    });
    $.pop();
  };
  await call('mount', Comp, 8, target, { n: 0 });
  await call('props', 8, { n: 1 });
  assert.deepEqual(reports.splice(0), [['fault', 8, 'later']]);
  await call('unmount', 8);
  // a first render that throws fails the mount itself
  const Bad = () => {
    throw new Error('first');
  };
  assert.throws(() => api.mount(Bad, 9, target, {}), /first/);
  assert.deepEqual(api.roots(), []);
});

test('roots: animation frames are per root and wait while it is hidden', async () => {
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
  await call('mount', Comp, 10, target, { go: 0 });
  runFrame();
  assert.equal(ticks, 1);
  await call('info', 10, { visible: false });
  await call('props', 10, { go: 1 }); // requests a frame while hidden
  runFrame();
  assert.equal(ticks, 1, 'held while hidden');
  await call('info', 10, { visible: true });
  runFrame();
  assert.equal(ticks, 2, 'runs once shown');
  await call('unmount', 10);
});

test('roots: a handler runs in its root: root() and animation frames held while hidden (EM2)', async () => {
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
  await call('mount', Comp, 11, target, {});
  runFrame();
  globalThis.__svelters_dispatch(btn.h, 'clicked', []);
  assert.equal(seenRoot, 11, 'root() inside a handler is its root');
  runFrame();
  assert.equal(ticks, 1, 'a handler frame runs while shown');
  await call('info', 11, { visible: false });
  globalThis.__svelters_dispatch(btn.h, 'clicked', []);
  runFrame();
  assert.equal(ticks, 1, "a hidden root's handler frame waits");
  await call('info', 11, { visible: true });
  runFrame();
  assert.equal(ticks, 2, 'and runs once shown');
  await call('unmount', 11);
});

test('roots: retarget moves where new natives go (EM2 move_root)', async () => {
  const a = th.host.create('box');
  const b = th.host.create('box');
  const Comp = ($$anchor, $$props) => {
    $.push($$props, true);
    $n.if_block($$anchor, () => $$props.more, ($$anchor) => label($$anchor, () => 'more'));
    $.pop();
  };
  await call('mount', Comp, 12, a, { more: false });
  take();
  await call('retarget', 12, { target: b });
  await call('props', 12, { more: true });
  const lines = take();
  assert.ok(lines.some((l) => l.startsWith(`insert #${b} `)), lines.join('\n'));
  assert.ok(!lines.some((l) => l.startsWith(`insert #${a} `)), lines.join('\n'));
  await call('unmount', 12);
});

// ---- FX6 (plans/milestones/FX6-embed-destroy-mutation.md): an entry the host aborted ----

/**
 * Runs `fn` and aborts it after `ms`, as QuickJS's turn budget does: V8's termination (vm's
 * `timeout`) skips every `finally` too, so Svelte's state stays as it was mid-update. Then what the
 * host does after an aborted entry: `__svelters_recover()`.
 */
function abort(fn, ms = 50) {
  globalThis.__fx6_run = fn;
  try {
    assert.throws(() => vm.runInThisContext('__fx6_run()', { timeout: ms }), { code: 'ERR_SCRIPT_EXECUTION_TIMEOUT' });
  } finally {
    delete globalThis.__fx6_run;
  }
  if (!process.env.FX6_NO_RECOVER) globalThis.__svelters_recover();
}

const spin = $.state(false);
const shared = $.state(0);
let stops = 0;
// a `createSubscriber` (as a store field or `svelte/reactivity` reads): its last unsubscribe bumps
// its version one microtask after the teardown, in the drain after the host's unmount
const sub = subscriber(() => () => stops++);
/** A root whose derived never returns once `spin` (or the `spin` prop) is set. */
const Slow = ($$anchor, $$props) => {
  $.push($$props, true);
  const d = $.derived(() => {
    if ($.get(spin) || $$props.spin) for (;;);
    return 'idle';
  });
  label($$anchor, () => (sub(), `slow ${$.get(d)}`));
  $.pop();
};
/** A healthy root reading what the aborted flush changed too. */
const Other = ($$anchor, $$props) => {
  $.push($$props, true);
  label($$anchor, () => `other ${$.get(shared)} ${$$props.n ?? 0}`);
  $.pop();
};
test('FX6: a flush aborted in a derived; the slow root unmounts, the other root keeps updating', async () => {
  const a = th.host.create('box');
  const b = th.host.create('box');
  await call('mount', Slow, 20, a, {});
  await call('mount', Other, 21, b, {});
  take();
  // Svelte's microtask flush (a checkpoint's job) runs the derived, which never returns; the other
  // root's effect was scheduled in the same batch, after it
  abort(() => {
    $.set(spin, true);
    $.set(shared, 1);
    flush_tasks();
  });
  // the host disposes the interrupted root: its subscriber's cleanup runs in the drain after unmount
  stops = 0;
  api.unmount(20);
  // the work the aborted flush had not done, in a later microtask
  await nextTask();
  assert.equal(stops, 1, 'the subscription stopped');
  assert.deepEqual(reports.splice(0), []);
  assert.ok(take().some((l) => /label "other 1 0"/.test(l)), "the other root got the aborted batch's change");
  // its later changes flush as before: Svelte's microtask flush, and the host's calls
  $.set(shared, 2);
  await nextTask();
  assert.ok(take().some((l) => /label "other 2 0"/.test(l)), 'a write outside any flush still schedules one');
  await call('props', 21, { n: 5 });
  assert.ok(take().some((l) => /label "other 2 5"/.test(l)));
  $.set(spin, false);
  await call('unmount', 21);
});

// FX6b: the host's call no longer flushes, so the turn budget stops the flush in the drain that
// follows it (Svelte's microtask), never inside a `flushSync`: `is_flushing_sync` stays clear and
// writes from timers and handlers keep scheduling their own flush.
test('FX6b: a host call aborted in its flush; no flushSync is left open, a timer change renders by itself', async () => {
  const a = th.host.create('box');
  const b = th.host.create('box');
  await call('mount', Slow, 22, a, { spin: false });
  await call('mount', Other, 23, b, {});
  take();
  // the call, then its drain: Svelte's microtask flush runs the slow derived
  abort(() => {
    api.props(22, { spin: true });
    flush_tasks();
  });
  assert.equal(flushing_sync(), false, 'no flushSync was interrupted');
  stops = 0;
  await call('unmount', 22);
  assert.equal(stops, 1, 'the subscription stopped');
  assert.deepEqual(reports.splice(0), []);
  // a timer's write in the other root renders without another host call into it
  setTimeout(() => $.set(shared, 3), 0);
  await nextTask();
  await nextTask();
  assert.ok(take().some((l) => /label "other 3 0"/.test(l)), 'a timer change renders in its own flush');
  await call('props', 23, { n: 7 });
  assert.ok(take().some((l) => /label "other 3 7"/.test(l)));
  await call('unmount', 23);
  assert.deepEqual(api.roots(), []);
});
