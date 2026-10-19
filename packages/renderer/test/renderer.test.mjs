// Drives the renderer methods directly (no Svelte compile) against the TestHost.
import os from 'node:os';
import test from 'node:test';
import assert from 'node:assert/strict';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root, dispatch, dispatchSync, gone, StaleHandleError, BINDINGS } = await import('../src/index.js');

const el = (tag, attrs = {}) => {
  const e = r.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) r.setAttribute(e, k, v);
  return e;
};
const tick = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');

test('lazy creation, label folding, ordering', () => {
  const win = el('window', { title: 'T' });
  const box = el('box', { spacing: '4' });
  const btn = el('button');
  const text = r.createTextNode('Hi');
  r.insert(btn, text, null);
  r.insert(box, btn, null);
  r.insert(win, box, null);
  assert.equal(take(), '', 'nothing native before attach to root');
  r.insert(root, win, null);
  assert.equal(
    take(),
    [
      'create window #1',
      'set #1 title "T"',
      'create box #2',
      'set #2 spacing 4',
      'create button #3',
      'set #3 label "Hi"',
      'insert #2 #3 before=end slot=default',
      'insert #1 #2 before=end slot=default',
      'call #1 present []',
    ].join('\n'),
  );
  r.setText(text, 'Yo');
  assert.equal(take(), 'set #3 label "Yo"');
});

test('dispatch returns handler results and the host event object', () => {
  const win = el('window');
  const b = el('button');
  r.insert(win, b, null);
  r.addEventListener(b, 'clicked', function (e) {
    if (e) e.defaultPrevented = true;
    return 'handled';
  });
  r.addEventListener(b, 'clicked', () => 42);
  r.insert(root, win, null);
  const lines = take().split('\n');
  const h = Number(lines.find((l) => l.startsWith('create button')).split('#')[1]);
  assert.ok(lines.includes(`listen #${h} clicked on`));
  assert.equal(lines.filter((l) => l.startsWith('listen')).length, 1, 'one listen per signal');
  const evt = { defaultPrevented: false };
  const out = dispatch(h, 'clicked', [evt]);
  assert.deepEqual(out.results, ['handled', 42]);
  assert.equal(out.event, evt);
  assert.equal(out.event.defaultPrevented, true);
  assert.deepEqual(dispatch(h, 'clicked', []).event, null);
});

test('sv-slot proxy routes native descendants to the parent slot', async () => {
  const win = el('window');
  const proxy = el('sv-slot', { name: 'titlebar' });
  const bar = el('header-bar');
  const body = el('box');
  r.insert(proxy, bar, null);
  r.insert(win, body, null);
  r.insert(win, proxy, body); // proxy before body in the tree
  r.insert(root, win, null);
  const out = take();
  assert.match(out, /insert #\d+ #\d+ before=end slot=titlebar/);
  assert.match(out, /insert #\d+ #\d+ before=end slot=default/);
  assert.ok(!out.includes('sv-slot'), 'proxy never reaches the host');
  // removing the proxy removes its widget
  r.remove(proxy);
  assert.match(take(), /^remove #\d+ #\d+$/);
  await tick();
  assert.match(take(), /^destroy #\d+$/);
});

test('move is remove + insert and is not destroyed; removal destroys children first', async () => {
  const win = el('window');
  const box = el('box');
  const a = el('label');
  const b = el('label');
  r.insert(box, a, null);
  r.insert(box, b, null);
  r.insert(win, box, null);
  r.insert(root, win, null);
  take();
  r.insert(box, b, a); // move b before a
  assert.match(take(), /^remove #\d+ #\d+\ninsert #\d+ #\d+ before=#\d+ slot=default$/);
  await tick();
  assert.equal(take(), '', 'moved node survives the sweep');
  r.remove(box);
  const rm = take();
  assert.match(rm, /^remove #\d+ #\d+$/);
  await tick();
  const d = take().split('\n');
  assert.equal(d.length, 3);
  assert.ok(d.every((l) => l.startsWith('destroy')));
});

// Replays insert/remove ops into per-parent child lists, the way a native backend orders them.
function replay(lines, order = new Map()) {
  for (const l of lines) {
    let m = /^insert #(\d+) #(\d+) before=(end|#\d+)/.exec(l);
    if (m) {
      const list = order.get(m[1]) ?? [];
      order.set(m[1], list);
      assert.ok(!list.includes(m[2]), `double insert of #${m[2]}`);
      const at = m[3] === 'end' ? list.length : list.indexOf(m[3].slice(1));
      assert.ok(at >= 0, `before ${m[3]} is not a child of #${m[1]}`);
      list.splice(at, 0, m[2]);
    }
    m = /^remove #(\d+) #(\d+)/.exec(l);
    if (m) {
      const list = order.get(m[1]);
      const at = list.indexOf(m[2]);
      assert.ok(at >= 0, `remove of non-child #${m[2]}`);
      list.splice(at, 1);
    }
  }
  return order;
}

function seeded(seed) {
  let s = seed;
  return () => (s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32;
}

test('keyed-each style moves keep the native order equal to the JS order', async () => {
  const win = el('window');
  const box = el('box');
  r.insert(win, box, null);
  r.insert(root, win, null);
  const boxH = th.lines.find((l) => l.startsWith('create box')).split('#')[1];
  const order = replay(th.lines.splice(0));
  const rows = [];
  const rnd = seeded(7);
  let n = 0;
  for (let step = 0; step < 400; step++) {
    const op = rnd();
    if (rows.length < 3 || op < 0.3) {
      const row = el('label');
      r.setText(row, `r${n++}`);
      const at = Math.floor(rnd() * (rows.length + 1));
      r.insert(box, row, rows[at] ?? null);
      rows.splice(at, 0, row);
    } else if (op < 0.5) {
      const at = Math.floor(rnd() * rows.length);
      r.remove(rows[at]);
      rows.splice(at, 1);
    } else {
      // move an existing row before another (or to the end), like a keyed reorder
      const from = Math.floor(rnd() * rows.length);
      const [row] = rows.splice(from, 1);
      const at = Math.floor(rnd() * (rows.length + 1));
      r.insert(box, row, rows[at] ?? null);
      rows.splice(at, 0, row);
    }
    if (step % 25 === 0) await tick();
  }
  await tick();
  replay(th.lines.splice(0), order);
  const jsOrder = [];
  for (let c = box.first; c; c = c.next) jsOrder.push(c);
  assert.deepEqual(jsOrder, rows);
  const native = order.get(boxH);
  assert.deepEqual(
    native,
    rows.map((row) => String(row.h)),
  );
});

// Wall-clock ratio test: meaningless on an oversubscribed machine (parallel agent builds push the
// load average to 8x the core count), so it skips there unless SVELTERS_PERF_TESTS=1 forces it.
const overloaded = os.loadavg()[0] > os.availableParallelism() * 1.5 && !process.env.SVELTERS_PERF_TESTS;
test('reordering many rows is not quadratic', { skip: overloaded && 'machine overloaded' }, async () => {
  const run = async (n) => {
    const win = el('window');
    const box = el('box');
    r.insert(win, box, null);
    r.insert(root, win, null);
    const rows = [];
    for (let i = 0; i < n; i++) {
      const row = el('label');
      r.setText(row, `r${i}`);
      r.insert(box, row, null);
      rows.push(row);
    }
    await tick();
    th.lines.length = 0;
    const t0 = performance.now();
    for (const row of [...rows].reverse()) r.insert(box, row, box.first === row ? null : box.first);
    await tick();
    const dt = performance.now() - t0;
    th.lines.length = 0;
    r.remove(win);
    await tick();
    th.lines.length = 0;
    return dt;
  };
  await run(200); // warm up
  // Best of three: wall time on a loaded machine is noisy (GC, preemption); the minimum is not.
  const best = async (n) => Math.min(await run(n), await run(n), await run(n));
  const small = await best(1000);
  const big = await best(4000);
  // linear work grows 4x; a linear scan per move would grow 16x
  assert.ok(big < Math.max(small, 5) * 9, `1000 rows ${small.toFixed(1)} ms, 4000 rows ${big.toFixed(1)} ms`);
});

// ---- bind:this: native access through the node ----

test('bind:this node: call and getProp reach the host, dead nodes throw StaleHandleError', async () => {
  const win = el('window');
  const e = el('entry', { text: 'seed' });
  r.insert(win, e, null);
  assert.throws(() => e.call('x'), /no native widget yet/);
  assert.throws(() => r.createTextNode('t').call('x'), TypeError);
  r.insert(root, win, null);
  take();
  assert.equal(win.call('present'), null);
  assert.equal(take(), `call #${win.handle} present []`);
  assert.equal(e.getProp('text'), 'seed');
  assert.equal(take(), '', 'getProp is not logged');

  r.remove(win);
  await tick();
  assert.equal(e.dead, 'destroyed');
  assert.equal(e.handle, null);
  for (const f of [() => e.call('present'), () => e.getProp('text')]) {
    assert.throws(f, (err) => err instanceof StaleHandleError && err.reason === 'destroyed' && err.name === 'StaleHandleError');
  }

  // re-inserting a dead node re-realizes it as a new native object
  const win2 = el('window');
  r.insert(win2, e, null);
  r.insert(root, win2, null);
  assert.equal(e.dead, null);
  assert.equal(e.getProp('text'), 'seed');
  r.remove(win2);
  await tick();
  take();
});

// ---- borrowed handles: nodes for native objects the app did not create ----

test('nodeFor: owned handles resolve to their element, borrowed ones to one stable node', async () => {
  const { nodeFor, gone } = await import('../src/index.js');
  const win = el('window');
  const e = el('entry');
  r.insert(win, e, null);
  r.insert(root, win, null);
  take();
  assert.equal(nodeFor(e.handle), e, 'an object we created keeps its bound node');

  const h = 9001; // a handle the backend minted for a native-made object
  const b = nodeFor(h);
  assert.equal(b.borrowed, true);
  assert.equal(b.handle, h);
  assert.equal(b.svlHandle, h);
  assert.equal(nodeFor(h), b, 'one node per handle');
  assert.equal(b.parent, null, 'never part of the tree');
});

test('borrowed nodes cannot be inserted, removed through the tree or listened to', async () => {
  const { nodeFor } = await import('../src/index.js');
  const b = nodeFor(9002);
  const box = el('box');
  assert.throws(() => r.insert(box, b, null), /borrowed native object cannot be inserted/);
  assert.throws(() => r.insert(b, box, null), /borrowed native object cannot be inserted into/);
  assert.throws(() => r.addEventListener(b, 'clicked', () => {}), /borrowed/);
  assert.equal(b.parent, null);
});

test('a native death marks a borrowed node dead with the reason', async () => {
  const { nodeFor, gone } = await import('../src/index.js');
  const b = nodeFor(9003);
  gone(9003, 'owner');
  assert.equal(b.dead, 'owner');
  assert.equal(b.handle, null);
  for (const f of [() => b.call('x'), () => b.getProp('y')]) {
    assert.throws(f, (err) => err instanceof StaleHandleError && err.reason === 'owner');
  }
  assert.notEqual(nodeFor(9003), b, 'a handle that died is never wrapped by the dead node again');
  gone(9999, 'owner'); // unknown handles are ignored
});

// ---- bind hook ----

test('bind:text: update, native change, compare before set, destroy', async () => {
  const win = el('window');
  const e = el('entry');
  r.insert(win, e, null);
  r.insert(root, win, null);
  take();
  const h = e.handle;
  const seen = [];
  const b = r.bind(e, 'text', () => 'unused', (v) => seen.push(v));
  assert.equal(take(), `listen #${h} changed on`);

  b.update('x');
  assert.equal(take(), `set #${h} text "x"`);
  b.update('x');
  assert.equal(take(), '', 'same value: no native write');
  b.update(null);
  assert.equal(take(), `set #${h} text ""`, 'null clears the native text');

  // a native edit reaches set() and is remembered, so writing it back is a no-op
  dispatch(h, 'changed', ['typed']);
  assert.deepEqual(seen, ['typed']);
  b.update('typed');
  assert.equal(take(), '');
  dispatch(h, 'changed', [{ detail: 'viaEvent' }]);
  assert.deepEqual(seen, ['typed', 'viaEvent']);

  b.destroy();
  assert.equal(take(), `listen #${h} changed off`);
  dispatch(h, 'changed', ['after']);
  assert.deepEqual(seen, ['typed', 'viaEvent'], 'no set() after destroy');

  // destroy after the native widget died is harmless
  const b2 = r.bind(e, 'text', () => '', () => {});
  r.remove(win);
  await tick();
  take();
  b2.update('late');
  b2.destroy();
  assert.equal(take(), '', 'no host ops for a dead node');
  assert.equal(e.props.get('text'), 'late', 'state is kept for a later re-realization');
});

test('bind: update before the widget exists is replayed on realize', () => {
  const win = el('window');
  const e = el('entry');
  r.insert(win, e, null);
  const b = r.bind(e, 'text', () => '', () => {});
  b.update('early');
  assert.equal(take(), '');
  r.insert(root, win, null);
  const lines = take().split('\n');
  assert.ok(lines.includes('set #' + e.handle + ' text "early"'));
  assert.ok(lines.includes('listen #' + e.handle + ' changed on'));
  r.remove(win);
});

test('bind: unsupported names and kinds throw with the element kind', () => {
  const lbl = el('label');
  assert.throws(() => r.bind(lbl, 'text', () => '', () => {}), /bind:text is not supported on <label> \(bindable: [^)]*\blabel\b/);
  const sep = el('separator');
  assert.throws(() => r.bind(sep, 'text', () => '', () => {}), /bind:text is not supported on <separator> \(it has no bindable properties\)/);
  const e = el('entry');
  assert.throws(() => r.bind(e, 'value', () => '', () => {}), /bind:value is not supported on <entry> \(bindable: [^)]*\btext\b/);
});

test('bind: valueFrom prop reads the property when the change signal has no value', async () => {
  BINDINGS['check-button'] = { active: { prop: 'active', signal: 'toggled', type: 'bool', direction: 'two_way', compare: true, readBack: false, valueFrom: 'prop', toolkits: ['gtk'] } };
  const win = el('window');
  const c = el('check-button');
  r.insert(win, c, null);
  r.insert(root, win, null);
  take();
  try {
    const seen = [];
    r.bind(c, 'active', () => false, (v) => seen.push(v));
    th.host.setProp(c.handle, 'active', true); // the user toggles it natively
    dispatch(c.handle, 'toggled', []);
    assert.deepEqual(seen, [true]);
  } finally {
    delete BINDINGS['check-button'];
  }
  r.remove(win);
  await tick();
  take();
});

test('bind:this node of a natively dead widget reports the ABI v2 death reason', async () => {
  const win = el('window');
  const lbl = el('label');
  r.insert(win, lbl, null);
  r.insert(root, win, null);
  take();
  const { handle: wh } = win;
  const { handle: lh } = lbl;
  gone(wh, 'closed');
  gone(lh, 'parent');
  for (const [node, why] of [[win, 'closed'], [lbl, 'parent']]) {
    assert.throws(() => node.getProp('title'), (err) => err instanceof StaleHandleError && err.reason === why && err.message.includes(why));
  }
  // the app removing the dead subtree afterwards keeps the native reason and sends nothing
  r.remove(win);
  await tick();
  assert.equal(take(), '');
  assert.equal(lbl.dead, 'parent');
});

test('bind: read-back reports a value the native side changed', async () => {
  BINDINGS.spinner = { value: { prop: 'value', signal: 'changed', type: 'number', direction: 'two_way', compare: true, readBack: true, toolkits: ['gtk'] } };
  const win = el('window');
  const s = el('spinner');
  r.insert(win, s, null);
  r.insert(root, win, null);
  take();
  const real = th.host.getProp;
  th.host.getProp = (h, name) => Math.min(real(h, name), 10); // the native side clamps to 10
  try {
    const seen = [];
    const b = r.bind(s, 'value', () => 0, (v) => seen.push(v));
    b.update('50');
    assert.equal(take(), `listen #${s.handle} changed on\nset #${s.handle} value 50`);
    assert.deepEqual(seen, [10], 'the clamped value is reported');
    b.update(10);
    assert.equal(take(), '', 'the clamped value is already the native value');
  } finally {
    th.host.getProp = real;
    delete BINDINGS.spinner;
  }
  r.remove(win);
  await tick();
  take();
});

test('dispatchSync: result comes from the event object, not the return value', () => {
  const win = el('window');
  const seen = [];
  r.addEventListener(win, 'close-request', (e, ...rest) => {
    seen.push([e.type, e.signal, e.deferred, rest]);
    return 'ignored';
  });
  r.insert(root, win, null);
  const h = Number(take().split('\n').find((l) => l.startsWith('create window')).split('#')[1]);
  // no flags: no decision
  assert.equal(dispatchSync(h, 'close-request', []), null);
  assert.deepEqual(seen[0], ['close-request', 'close-request', false, []]);
  // a second listener vetoes
  r.addEventListener(win, 'close-request', (e) => e.preventDefault());
  assert.equal(dispatchSync(h, 'close-request', []), true);
  // stopPropagation counts as a stop too; extra args follow the event
  const key = el('entry');
  r.insert(win, key, null);
  const kh = Number(take().split('\n').find((l) => l.startsWith('create entry')).split('#')[1]);
  r.addEventListener(key, 'key-pressed', (e, keyval, keycode, state) => {
    seen.push([keyval, keycode, state]);
    if (keyval === 97) e.stopPropagation();
  });
  assert.equal(dispatchSync(kh, 'key-pressed', [97, 38, 0]), true);
  assert.equal(dispatchSync(kh, 'key-pressed', [98, 56, 0]), null);
  assert.deepEqual(seen.slice(-2), [[97, 38, 0], [98, 56, 0]]);
  // an explicit result wins over the flags
  r.addEventListener(key, 'key-pressed', (e) => {
    e.result = false;
  });
  assert.equal(dispatchSync(kh, 'key-pressed', [97, 38, 0]), false);
});

test('dispatchSync: stopImmediatePropagation, throwing listeners, unknown handles', () => {
  const win = el('window');
  const order = [];
  r.addEventListener(win, 'close-request', (e) => {
    order.push(1);
    e.preventDefault();
    e.stopImmediatePropagation();
  });
  r.addEventListener(win, 'close-request', () => order.push(2));
  r.insert(root, win, null);
  const h = Number(take().split('\n').find((l) => l.startsWith('create window')).split('#')[1]);
  assert.equal(dispatchSync(h, 'close-request', []), true);
  assert.deepEqual(order, [1]);
  r.addEventListener(win, 'close-request', () => {
    throw new Error('boom');
  });
  const w2 = el('window');
  r.addEventListener(w2, 'close-request', () => {
    throw new Error('boom');
  });
  r.insert(root, w2, null);
  const h2 = Number(take().split('\n').find((l) => l.startsWith('create window')).split('#')[1]);
  assert.throws(() => dispatchSync(h2, 'close-request', []), /boom/);
  assert.throws(() => dispatchSync(999999, 'close-request', []), /unknown handle/);
});

test('dispatch with deferred=true gives listeners the same event shape and ignores the flags', () => {
  const win = el('window');
  let got;
  r.addEventListener(win, 'close-request', (e, ...rest) => {
    got = [e.deferred, e.cancelable, rest];
    e.preventDefault();
  });
  r.insert(root, win, null);
  const h = Number(take().split('\n').find((l) => l.startsWith('create window')).split('#')[1]);
  const out = dispatch(h, 'close-request', [], true);
  assert.deepEqual(got, [true, false, []]);
  assert.equal(out.event.defaultPrevented, false, 'a late event cannot veto anything');
});

test('dispatchSync warns about handlers that return a Promise', () => {
  const win = el('window');
  r.addEventListener(win, 'close-request', async (e) => {
    await null;
    e.preventDefault();
  });
  r.insert(root, win, null);
  const h = Number(take().split('\n').find((l) => l.startsWith('create window')).split('#')[1]);
  const warned = [];
  const orig = console.warn;
  console.warn = (m) => warned.push(m);
  try {
    assert.equal(dispatchSync(h, 'close-request', []), null, 'the late flag does not count');
  } finally {
    console.warn = orig;
  }
  assert.equal(warned.length, 1);
  assert.match(warned[0], /returned a Promise/);
});

// ---- child attributes (ABI v2.2) ----

const ids = (lines, tag) => lines.filter((l) => l.startsWith(`create ${tag} `)).map((l) => Number(l.split('#')[1]));

test('child attrs: sent after insert, reactive, re-applied on a new parent, cleared on remove', () => {
  const win = el('window');
  const g1 = el('grid');
  const g2 = el('grid');
  const lbl = el('label', { 'parent:column': 1, 'parent:row': 2 });
  r.insert(win, g1, null);
  r.insert(win, g2, null);
  r.insert(g1, lbl, null);
  r.insert(root, win, null);
  const lines = take().split('\n');
  const [h1, h2] = ids(lines, 'grid');
  const [hl] = ids(lines, 'label');
  assert.deepEqual(
    lines.filter((l) => l.startsWith('attr ') || (l.startsWith('insert ') && l.includes(`#${hl} `))),
    [`insert #${h1} #${hl} before=end slot=default`, `attr #${h1} #${hl} column 1`, `attr #${h1} #${hl} row 2`],
    'attrs follow the insert, in attribute order; they are not props of the child',
  );
  assert.ok(!lines.some((l) => l.startsWith(`set #${hl} parent`)));
  r.setAttribute(lbl, 'parent:row', 3);
  r.setAttribute(lbl, 'parent:row', 3);
  assert.equal(take(), `attr #${h1} #${hl} row 3`, 'a change is one op; an unchanged value sends nothing');
  r.insert(g2, lbl, null); // move: remove + insert + attrs again
  assert.equal(
    take(),
    [`remove #${h1} #${hl}`, `insert #${h2} #${hl} before=end slot=default`, `attr #${h2} #${hl} column 1`, `attr #${h2} #${hl} row 3`].join('\n'),
  );
  r.removeAttribute(lbl, 'parent:column');
  assert.equal(take(), `attr #${h2} #${hl} column null`, 'removing the attribute resets it');
  r.remove(lbl);
  assert.equal(take(), `remove #${h2} #${hl}`, 'nothing is sent for a removed child');
  r.setAttribute(lbl, 'parent:row', 9);
  assert.equal(take(), '', 'a detached child only remembers the value');
  r.insert(g1, lbl, null);
  assert.equal(take(), `insert #${h1} #${hl} before=end slot=default\nattr #${h1} #${hl} row 9`);
  r.remove(win);
});

test('child attrs: attributes set before the parent is native are applied when the tree is realized', async () => {
  await tick();
  take();
  const win = el('window');
  const g = el('grid');
  const a = el('label', { 'parent:row': 1 });
  r.insert(g, a, null);
  r.insert(win, g, null);
  assert.equal(take(), '');
  r.insert(root, win, null);
  const lines = take().split('\n');
  const [hg] = ids(lines, 'grid');
  const [hl] = ids(lines, 'label');
  const ins = lines.indexOf(`insert #${hg} #${hl} before=end slot=default`);
  assert.ok(ins >= 0);
  assert.equal(lines[ins + 1], `attr #${hg} #${hl} row 1`);
  r.remove(win);
});

// ---- listener phases (ABI v2.2) ----

test('listen phases: separate native listens per flags, dispatch by flags, capture option, attach helper', async () => {
  const { listen } = await import('../src/index.js');
  const win = el('window');
  const e = el('entry');
  r.insert(win, e, null);
  r.insert(root, win, null);
  take();
  const calls = [];
  r.addEventListener(e, 'key-pressed', () => calls.push('plain'));
  r.addEventListener(e, 'key-pressed', () => calls.push('capture'), { capture: true });
  const detach = listen('key-pressed', () => calls.push('after'), { after: true })(e);
  const lines = take().split('\n');
  assert.deepEqual(lines.map((l) => l.replace(/#\d+/, '#h')), ['listen #h key-pressed on', 'listen #h key-pressed on capture', 'listen #h key-pressed on after']);
  const h = Number(lines[0].match(/#(\d+)/)[1]);
  dispatch(h, 'key-pressed', [1], false, 0);
  dispatch(h, 'key-pressed', [1], false, 2);
  dispatch(h, 'key-pressed', [1], false, 1);
  assert.deepEqual(calls, ['plain', 'capture', 'after']);
  r.addEventListener(e, 'key-pressed', () => calls.push('plain2'));
  assert.equal(take(), '', 'a second listener with the same flags does not listen again');
  detach();
  assert.equal(take(), `listen #${h} key-pressed off after`);
  r.remove(win);
});

test('toplevel kinds: a dialog inside a window is transient, shown after the window, removed with its block', async () => {
  const win = el('window', { title: 'Main' });
  const box = el('box');
  const dlg = el('message-box', { text: 'Hi' });
  const lbl = el('label');
  r.insert(box, lbl, null);
  r.insert(box, dlg, null);
  r.insert(win, box, null);
  take();
  r.insert(root, win, null);
  const out = take().split('\n');
  const id = (tag) => Number(out.find((l) => l.startsWith(`create ${tag} `)).split('#')[1]);
  const [w, b, d] = [id('window'), id('box'), id('message-box')];
  assert.ok(out.includes(`insert #${w} #${d} before=end slot=default`), out.join('\n'));
  assert.ok(out.indexOf(`call #${w} present []`) < out.indexOf(`call #${d} open []`), out.join('\n'));
  assert.ok(!out.some((l) => l.startsWith(`insert #${b} #${d}`)), 'the dialog is not a layout child');
  const late = el('label');
  r.insert(box, late, null);
  assert.match(take(), new RegExp(`^create label #\\d+\\ninsert #${b} #\\d+ before=end slot=default$`));
  r.remove(dlg);
  assert.equal(take(), `remove #${w} #${d}`);
  await tick();
  assert.equal(take(), `destroy #${d}`);
  r.remove(win);
  await tick();
  take();
});

test('toplevel kinds: show policy and a dialog mounted at the root', async () => {
  const hidden = el('message-box', { show: 'none' });
  const modeless = el('message-box', { show: 'modeless' });
  r.insert(root, hidden, null);
  r.insert(root, modeless, null);
  const out = take();
  assert.ok(!out.includes('call #') || out.split('\n').filter((l) => l.startsWith('call')).length === 1, out);
  assert.match(out, /call #\d+ present \[\]/);
  assert.throws(() => r.insert(root, el('label'), null), /only toplevel kinds/);
  assert.throws(() => r.insert(root, el('message-box', { show: 'sometimes' }), null), /expected modal, modeless or none/);
  r.remove(hidden);
  r.remove(modeless);
  await tick();
  take();
});

test('toplevel kinds: a dialog nested in a subtree that is not yet realized waits for the window', () => {
  const win = el('window');
  const dlg = el('about-dialog');
  const inner = el('box');
  r.insert(inner, dlg, null);
  r.insert(win, inner, null);
  take();
  r.insert(root, win, null);
  const lines = take().split('\n');
  const w = lines.find((l) => l.startsWith('create window')).split('#')[1];
  const d = lines.find((l) => l.startsWith('create about-dialog')).split('#')[1];
  assert.ok(lines.indexOf(`call #${w} present []`) >= 0 && lines.indexOf(`call #${w} present []`) < lines.indexOf(`call #${d} present []`), lines.join('\n'));
  assert.ok(lines.includes(`insert #${w} #${d} before=end slot=default`), lines.join('\n'));
});

test('dispatchSync: a typed result object reaches the host unchanged (out-parameters)', () => {
  const spin = el('spin-button');
  const w0 = el('window');
  r.insert(w0, spin, null);
  r.insert(root, w0, null);
  const h = Number(take().split('\n').find((l) => l.startsWith('create spin-button')).split('#')[1]);
  r.addEventListener(spin, 'input', (e) => {
    e.result = { ret: 1, newValue: 16 };
  });
  assert.deepEqual(dispatchSync(h, 'input', []), { ret: 1, newValue: 16 });
});
