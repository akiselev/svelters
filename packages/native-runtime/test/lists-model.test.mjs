// Virtualized lists in model mode (N3, plans/milestones/N3-native-models.md): `<ListView>` showing a
// native list model instead of an array. The TestHost plays the toolkit: it records the ops, and the
// test delivers the row lifecycle and the change reports (`items-changed`, `data-changed`) by hand.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';
import { NativeObject, WRAP } from '@svelters/types/core';
import * as $n from '../src/index.js';
import { ListView } from '../src/lists.js';

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

class FakeItem extends NativeObject {
  constructor(h, text) {
    super(WRAP, h);
    this.text = text;
  }
}

/** A GTK-like model: rows come from `getItem`. */
class FakeListModel extends NativeObject {
  constructor(h, items) {
    super(WRAP, h);
    this.items = items;
  }
  getItem(pos) {
    return this.items[pos] ?? null;
  }
}

/** A Qt-like model: no `getItem`; a row is `index(row, 0)`. */
class FakeItemModel extends NativeObject {
  constructor(h, rows) {
    super(WRAP, h);
    this.rows = rows;
  }
  index(row) {
    return this.rows[row];
  }
}

/** A window #1 holding a ListView (#2) of `model`; rows are a label `"<index>:<item.text>"`. */
function mountModel(model) {
  const items = $.state(model);
  let api = null;
  let activated = null;
  const row = ($$anchor, item, index) => {
    const l = $n.element('label');
    $.template_effect(() => $n.set(l, 'label', `${index()}:${item().text}`));
    $n.append($$anchor, l);
  };
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.element('window', 'present');
    api = ListView($n.slot(w, null), {
      get items() {
        return $.get(items);
      },
      key: (it) => {
        if (it instanceof NativeObject) throw new Error('key is not called in model mode');
        return it.id;
      },
      row,
      onactivate: (item, pos) => (activated = [pos, item.text]),
    });
    $n.append($$anchor, w);
    $.pop();
  });
  $.flush();
  return { items, api: () => api, activated: () => activated };
}

test('model mode: the model is set on the view, no splices, the change reports are listened', () => {
  const t = harness();
  mountModel(new FakeListModel(900, [new FakeItem(901, 'A'), new FakeItem(902, 'B')]));
  const lines = t.take();
  assert.ok(lines.includes('set #2 model 900'), lines.join('\n'));
  assert.ok(lines.includes('listen #2 items-changed on'));
  assert.ok(lines.includes('listen #2 data-changed on'));
  assert.ok(!lines.some((l) => l.includes('splice')), 'no splice in model mode');
});

test('model mode: bind takes the toolkit item; items-changed shifts bound rows ordered by epoch', () => {
  const t = harness();
  const items = [new FakeItem(901, 'A'), new FakeItem(902, 'B'), new FakeItem(903, 'C')];
  const l = mountModel(new FakeListModel(900, items));
  t.sync(2, 'setup', [1]);
  t.take();
  t.sync(2, 'bind', [1, 1, 0, 1, 0, items[1]]);
  assert.ok(t.take().includes('set #4 label "1:B"'));
  // An insert at 0 that the toolkit reported after this bind (epoch 1 > the slot's 0): shifted.
  t.event(2, 'items-changed', [0, 0, 1, 1]);
  assert.deepEqual(t.take(), ['set #4 label "2:B"']);
  // A sync bind that already saw change 2 is not shifted again when the report of change 2 arrives.
  t.sync(2, 'bind', [1, 0, 0, 2, 2, items[0]]);
  t.event(2, 'items-changed', [0, 0, 1, 2]);
  assert.deepEqual(t.take(), ['set #4 label "0:A"']);
  // A deferred bind older than the one applied is ignored.
  t.event(2, 'bind', [1, 2, 0, 1, 0, items[2]], true);
  assert.deepEqual(t.take(), []);
  // Removal of the bound item: the slot leaves its position (the toolkit unbinds or rebinds it).
  t.event(2, 'items-changed', [0, 1, 0, 3]);
  const ctl = l.api().controller();
  assert.equal(ctl.slots.get(1).pos, -1);
  assert.equal(ctl.modelChanges, 3);
  t.event(2, 'activate', [1]);
  assert.deepEqual(l.activated(), [1, 'B'], 'activate resolves the item through the model');
});

test('model mode: data-changed re-reads bound rows (Qt); an array again leaves model mode', () => {
  const t = harness();
  const rows = [{ text: 'x' }, { text: 'y' }];
  const l = mountModel(new FakeItemModel(900, rows));
  t.sync(2, 'setup', [1]);
  t.take();
  t.sync(2, 'bind', [1, 1, 0, 1, 0, null]);
  assert.ok(t.take().includes('set #4 label "1:y"'));
  rows[1] = { text: 'y2' };
  t.event(2, 'data-changed', [0, 1, 0]);
  assert.deepEqual(t.take(), ['set #4 label "1:y2"']);
  t.event(2, 'data-changed', [0, 0, 0]);
  assert.deepEqual(t.take(), [], 'rows outside the range are untouched');
  $.set(l.items, [{ id: 'k', text: 'K' }]);
  $.flush();
  const lines = t.take();
  assert.ok(lines.includes('set #2 model null'), lines.join('\n'));
  assert.ok(lines.includes('call #2 splice [0,0,[1]]'), lines.join('\n'));
});
