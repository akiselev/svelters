// Virtualized lists (src/lists.js) against the Node TestHost: the model-view protocol, row-slot
// recycling and the deferred delivery of row lifecycle signals.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import * as $n from '../src/index.js';
import { ListView, flattenTree, expandAll, collapseAll, expandToDepth, expandRecursively } from '../src/lists.js';

function harness() {
  const th = createTestHost();
  globalThis.__svelters_host = th.host;
  let seen = 0;
  const take = () => {
    const out = th.lines.slice(seen);
    seen = th.lines.length;
    return out;
  };
  const event = (h, signal, args = [], deferred = false) => {
    globalThis.__svelters_dispatch(h, signal, args, deferred);
    $.flush();
  };
  const sync = (h, signal, args = []) => {
    const r = globalThis.__svelters_dispatch_sync(h, signal, args);
    $.flush();
    return r;
  };
  return { th, take, event, sync };
}

/**
 * A window #1 holding a ListView (#2) of `items` whose rows are a label `"<index>:<text>"` plus a
 * row-local counter in a second label; `show` toggles the list.
 */
function mountList(props = {}) {
  const items = $.state(props.items ?? [{ id: 'a', text: 'A' }, { id: 'b', text: 'B' }, { id: 'c', text: 'C' }]);
  const show = $.state(true);
  const selected = $.state(-1);
  let api = null;
  const row = ($$anchor, item, index) => {
    const local = $.state(0);
    const box = $n.element('box');
    const l = $n.element('label');
    $.template_effect(() => $n.set(l, 'label', `${index()}:${item().text}`));
    $n.append($n.slot(box, null), l);
    const c = $n.element('label');
    $.template_effect(() => $n.set(c, 'label', `n=${$.get(local)}`));
    $n.on(c, 'bump', () => $.update(local));
    $n.append($n.slot(box, null), c);
    $n.append($$anchor, box);
  };
  const unmount = $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    $n.if_block(
      $n.slot(w, null),
      () => $.get(show),
      (a) => {
        api = ListView(a, {
          get items() {
            return $.get(items);
          },
          key: (it) => it.id,
          row,
          get selected() {
            return $.get(selected);
          },
          set selected(v) {
            $.set(selected, v);
          },
          'min-height': 100,
          ...props.extra,
        });
      },
    );
    $n.append($$anchor, w);
    $.pop();
  });
  $.flush();
  return { items, show, selected, unmount, api: () => api };
}

test('creation: the virtual-list gets its props, listens to the row lifecycle and receives the ids', () => {
  const t = harness();
  mountList();
  assert.deepEqual(t.take(), [
    'create window #1',
    'create virtual-list #2',
    'set #2 layout "list"',
    'set #2 selection "single"',
    'set #2 min-height 100',
    'listen #2 setup on',
    'listen #2 bind on',
    'listen #2 unbind on',
    'listen #2 teardown on',
    'listen #2 activate on',
    'listen #2 select on',
    'insert #1 #2 before=end slot=default',
    'call #1 present []',
    'call #2 splice [0,0,[1,2,3]]',
  ]);
});

test('updates are minimal splices; a replaced item object with the same key is no native op', () => {
  const t = harness();
  const l = mountList();
  t.take();
  $.set(l.items, [...$.get(l.items), { id: 'd', text: 'D' }]);
  $.flush();
  assert.deepEqual(t.take(), ['call #2 splice [3,0,[4]]']);
  $.set(l.items, $.get(l.items).map((it) => (it.id === 'b' ? { ...it, text: 'B!' } : it)));
  $.flush();
  assert.deepEqual(t.take(), []);
  $.set(l.items, $.get(l.items).toReversed());
  $.flush();
  assert.deepEqual(t.take(), ['call #2 splice [0,3,[]]', 'call #2 splice [1,0,[3,2,1]]']);
});

test('row slots: content mounted at the first bind, rebinding keeps row-local state and natives', () => {
  const t = harness();
  mountList();
  t.take();
  assert.equal(t.sync(2, 'setup', [7]), null);
  assert.deepEqual(t.take(), ['create box #3', 'call #2 setRow [7,3]']);
  t.sync(2, 'bind', [7, 0, 1, 1]);
  assert.deepEqual(t.take(), [
    'create box #4',
    'create label #5',
    'set #5 label "0:A"',
    'insert #4 #5 before=end slot=default',
    'create label #6',
    'set #6 label "n=0"',
    'listen #6 bump on',
    'insert #4 #6 before=end slot=default',
    'insert #3 #4 before=end slot=default',
  ]);
  t.event(6, 'bump');
  assert.deepEqual(t.take(), ['set #6 label "n=1"']);
  // scrolled: the slot shows another item; same natives, local state kept
  t.sync(2, 'unbind', [7, 1]);
  t.sync(2, 'bind', [7, 2, 3, 2]);
  assert.deepEqual(t.take(), ['set #5 label "2:C"']);
});

test('bound rows follow splices and item replacements without a rebind', () => {
  const t = harness();
  const l = mountList();
  t.sync(2, 'setup', [1]);
  t.sync(2, 'bind', [1, 2, 3, 1]);
  t.take();
  $.set(l.items, $.get(l.items).slice(1));
  $.flush();
  assert.deepEqual(t.take(), ['call #2 splice [0,1,[]]', 'set #5 label "1:C"']);
  $.set(l.items, $.get(l.items).map((it) => (it.id === 'c' ? { ...it, text: 'C!' } : it)));
  $.flush();
  assert.deepEqual(t.take(), ['set #5 label "1:C!"']);
});

test('SV59: `label` names the rows (splice labels, setLabels on a rename) and `rowAnchor` anchors the option node', () => {
  const t = harness();
  const l = mountList({ extra: { label: (it) => it.text, rowAnchor: (it) => `row:${it.id}` } });
  assert.ok(t.take().includes('call #2 splice [0,0,[1,2,3],["A","B","C"]]'), 'labels ride the splice');
  $.set(l.items, [...$.get(l.items), { id: 'd', text: 'D' }]);
  $.flush();
  assert.deepEqual(t.take(), ['call #2 splice [3,0,[4],["D"]]']);
  // a rename keeps the key: no splice, the labels go out whole
  $.set(l.items, $.get(l.items).map((it) => (it.id === 'b' ? { ...it, text: 'B!' } : it)));
  $.flush();
  assert.deepEqual(t.take(), ['call #2 setLabels [["A","B!","C","D"]]']);
  // the row outlet (the option) takes the item's anchor at bind, and drops it unbound
  t.sync(2, 'setup', [7]);
  t.take();
  t.sync(2, 'bind', [7, 1, 2, 1]);
  assert.ok(t.take().includes('set #3 anchor "row:b"'));
  t.sync(2, 'unbind', [7, 1]);
  assert.deepEqual(t.take(), ['set #3 anchor ""']);
  t.sync(2, 'bind', [7, 2, 3, 2]);
  assert.ok(t.take().includes('set #3 anchor "row:c"'));
});

test('deferred delivery: raw args accepted, a stale bind finds its id, a late unbind of an older binding is ignored', () => {
  const t = harness();
  const l = mountList();
  t.event(2, 'setup', [1], true);
  t.take();
  $.set(l.items, $.get(l.items).slice(1)); // ids now [2, 3]
  $.flush();
  t.take();
  t.event(2, 'bind', [1, 0, 3, 2], true); // id 3 is at position 1 now
  assert.ok(t.take().includes('set #5 label "1:C"'));
  t.event(2, 'unbind', [1, 1], true); // older gen: ignored
  $.set(l.items, $.get(l.items).map((it) => ({ ...it, text: it.text + '?' })));
  $.flush();
  assert.deepEqual(t.take(), ['set #5 label "1:C?"'], 'still bound');
  t.event(2, 'unbind', [1, 2], true);
  $.set(l.items, $.get(l.items).map((it) => ({ ...it, text: it.text + '?' })));
  $.flush();
  assert.deepEqual(t.take(), [], 'unbound rows are not updated');
});

test('selection both ways, activate, scrollTo', () => {
  const t = harness();
  const l = mountList();
  t.take();
  t.event(2, 'select', [1]);
  assert.equal($.get(l.selected), 1);
  assert.deepEqual(t.take(), [], 'a native selection is not echoed');
  $.set(l.selected, 2);
  $.flush();
  assert.deepEqual(t.take(), ['set #2 selected 2']);
  l.api().scrollTo(2);
  assert.deepEqual(t.take(), ['call #2 scrollTo [2]']);
  assert.equal(l.api().controller().indexOfKey('c'), 2);
});

test('teardown: a removed list destroys every slot (content and outlet) and the view', () => {
  const t = harness();
  const l = mountList();
  t.sync(2, 'setup', [1]);
  t.sync(2, 'bind', [1, 0, 1, 1]);
  t.sync(2, 'setup', [2]);
  t.take();
  $.set(l.show, false);
  $.flush();
  const ops = t.take();
  for (const h of [2, 3, 4, 5, 6, 7]) assert.ok(ops.includes(`destroy #${h}`), `destroy #${h} in ${ops}`);
  assert.ok(ops.includes('remove #1 #2'));
  assert.ok(ops.includes('remove #3 #4'));
  assert.equal(t.th.lines.filter((x) => x.startsWith('create')).length, t.th.lines.filter((x) => x.startsWith('destroy')).length + 1, 'only the window is left');
});

test('flattenTree: expanded nodes splice their loaded children in; a filter keeps the path to a match', () => {
  const tree = [
    { id: 'a', children: [{ id: 'a1', children: [{ id: 'a1x' }] }, { id: 'a2' }] },
    { id: 'b', children: [{ id: 'b1' }] },
  ];
  const opts = (expanded, filter) => ({
    key: (n) => n.id,
    childrenOf: (n) => n.children,
    hasChildren: (n) => !!n.children,
    expanded: new Set(expanded),
    filter,
  });
  const rows = (expanded, filter) => flattenTree(tree, opts(expanded, filter)).map((r) => `${r.key}:${r.level}${r.expandable ? (r.expanded ? '-' : '+') : ''}`);
  assert.deepEqual(rows([]), ['a:0+', 'b:0+']);
  assert.deepEqual(rows(['a', 'a1']), ['a:0-', 'a1:1-', 'a1x:2', 'a2:1', 'b:0+']);
  // `a1x` matches deep inside: its ancestors stay, `a2` and `b` go
  assert.deepEqual(rows(['a', 'a1'], (n) => n.id === 'a1x'), ['a:0-', 'a1:1-', 'a1x:2']);
  // collapsed: the ancestor of the match stays, closed
  assert.deepEqual(rows([], (n) => n.id === 'a1x'), ['a:0+']);
});

test('OW5a1b: expandAll, collapseAll, expandToDepth, expandRecursively return the keys for bind:expanded', () => {
  const tree = [
    { id: 'a', children: [{ id: 'b', children: [{ id: 'c' }] }, { id: 'd', children: [] }] },
    { id: 'e', children: [{ id: 'f', children: [{ id: 'g' }] }] },
  ];
  assert.deepEqual(expandAll(tree), ['a', 'b', 'e', 'f'], 'nodes with children only');
  assert.deepEqual(collapseAll(), []);
  assert.deepEqual(expandToDepth(tree, 0), ['a', 'e'], 'depth 0: the roots open');
  assert.deepEqual(expandToDepth(tree, 1), ['a', 'b', 'e', 'f']);
  assert.deepEqual(expandRecursively(['a'], tree[1]), ['a', 'e', 'f']);
  const lazy = { children: (n) => n.kids, key: (n) => n.k };
  assert.deepEqual(expandAll([{ k: 1, kids: [{ k: 2, kids: [{ k: 3 }] }] }], lazy), [1, 2]);
});
