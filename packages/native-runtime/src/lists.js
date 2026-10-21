// Virtualized native lists for components compiled by the NC1 compiler: the native-runtime
// implementation of @svelters/lists (`ListView`, `ListController`). Under `nativeConfig` the
// specifier `@svelters/lists` resolves here (vite.js), so an app written against the JS
// pipeline's `<ListView>` compiles unchanged and speaks the same model-view protocol
// (plans/architecture/backend-abi.md "Model-view protocol (M5)", plans/milestones/M5-lists.md):
//
//   JS -> native   splice(pos, removed, ids[]), setRow(row, outlet), scrollTo(pos), selected
//   native -> JS   setup(row), bind(row, pos, id, gen), unbind(row, gen), teardown(row)  [sync]
//                  activate(pos), select(pos)                                           [async]
//
// Semantics (docs/notes/native-compiler/runtime-contract.md "Virtualized lists"): items are
// identified by `key(item, index)`; each key gets a native id for as long as it stays in the list;
// the keyed diff (the same `diffKeys` as the JS pipeline) turns an update into minimal splices.
// Row content lives in recycled *row slots*: one hosted root per slot, mounted at the slot's first
// bind into an outlet (a native box outside the window tree that the toolkit places in the view),
// rebound to other items as the view scrolls, destroyed at teardown.
//
// Native models (N3, plans/milestones/N3-native-models.md): `items` (or `model`) may instead be a
// native list model the app holds (a GTK `GListModel`: Gio.ListStore, Gtk.DirectoryList, a filter /
// sort / selection model...; a Qt `QAbstractItemModel`). The view then reads that model natively:
// JS keeps no copy, no keys and no ids. The native side reports the model's changes
// (`items-changed(pos, removed, added, epoch)`, Qt `data-changed(first, last, epoch)`) so bound rows
// keep their index, and `bind` carries the item (GTK) and the change counter `epoch`.
import * as $ from './core/index.js';
import { NativeObject, wrapObject } from '@svelters/types/core';
// the core's component context functions (Svelte's, through the core adapter)
import { flush, get_all_contexts as getAllContexts, set_context as setContext } from './core/internal.js';
import { diffKeys } from './lists-diff.js';
import { append, element, mount, on, rowOutlet, set } from './index.js';

export { diffKeys, applySplices, lis, MAX_OPS } from './lists-diff.js';

/** `arr.splice` without spreading (a 100k-item insert would overflow the argument limit). */
function spliceArray(arr, pos, removed, add) {
  if (add.length < 4096) {
    arr.splice(pos, removed, ...add);
    return arr;
  }
  return arr.slice(0, pos).concat(add, arr.slice(pos + removed));
}

/**
 * The event object a sync signal's handlers receive first (same surface as the JS renderer's
 * NativeEvent): `preventDefault()` / `stopPropagation()` give the toolkit `true`, `result = v` an
 * explicit value. Only used when no other sync dispatcher is installed.
 */
class SyncEvent {
  constructor(signal, args, deferred) {
    this.type = signal;
    this.signal = signal;
    this.args = args;
    this.deferred = deferred;
    this.cancelable = !deferred;
    this.defaultPrevented = false;
    this.cancelBubble = false;
    this.result = undefined;
  }
  preventDefault() {
    if (this.cancelable) this.defaultPrevented = true;
  }
  stopPropagation() {
    if (this.cancelable) this.cancelBubble = true;
  }
}

/**
 * Minimal `__svelters_dispatch_sync` (call_sync delivery) when the runtime has none: handlers get
 * `(event, ...args)` in registration order; the result follows the JS renderer's rules.
 */
function installSyncDispatch() {
  if (typeof globalThis.__svelters_dispatch_sync === 'function') return;
  globalThis.__svelters_dispatch_sync = (h, signal, args = []) => {
    const ev = new SyncEvent(signal, args, false);
    globalThis.__svelters_dispatch(h, signal, [ev, ...args]);
    if (ev.result !== undefined) return ev.result;
    return ev.defaultPrevented || ev.cancelBubble ? true : null;
  };
}

/**
 * A row-lifecycle handler that accepts both deliveries of a sync signal: `(event, ...args)` from
 * call_sync, raw `(...args)` from a deferred async delivery (the host was busy).
 */
const lifecycle = (fn) =>
  function (...a) {
    if (a.length && a[0] !== null && typeof a[0] === 'object') a = a.slice(1);
    return fn(...a);
  };

export class ListController {
  /**
   * @param {object} opts
   * @param {(item: any, index: number) => unknown} opts.key
   * @param {() => Function} opts.row  current row snippet: `(anchor, item, index)` with thunks
   * @param {() => Map<any, any> | undefined} [opts.context]  contexts handed to row content
   * @param {(pos: number, item: any) => void} [opts.onactivate]
   */
  constructor(opts) {
    this.opts = opts;
    this.view = null; // the virtual-list native
    this.keys = []; // key per position
    this.ids = []; // native id per position
    this.entries = new Map(); // key -> id
    this.list = []; // current items snapshot, by position
    this.nextId = 1;
    this.slots = new Map(); // row slot -> { outlet, unmountOutlet, unmountRow, item, index, id, pos, gen }
    this.destroyed = false;
    this.splices = 0; // native splice calls issued (tests, benchmarks)
    this.model = null; // model mode: the native model shown (a wrapper), else null
    this.modelChanges = 0; // model mode: items-changed reports received (tests, benchmarks)
  }

  /** `x` is a native list model rather than an array or iterable: an object with a native handle. */
  static isNativeModel(x) {
    return x instanceof NativeObject || (x !== null && typeof x === 'object' && typeof x.svlHandle === 'number' && !Array.isArray(x) && typeof x[Symbol.iterator] !== 'function');
  }

  attach(view) {
    this.view = view;
  }

  detach() {
    this.destroyed = true;
    for (const row of [...this.slots.keys()]) this.teardown(row);
    this.view = null;
  }

  /**
   * Brings the native id list in line with `items`. Runs inside an effect: reading the items and
   * their keys is tracked, everything after is not.
   */
  update(items) {
    if (ListController.isNativeModel(items)) {
      $.untrack(() => this.#setModel(items));
      return;
    }
    if (this.model) $.untrack(() => this.#setModel(null));
    const list = Array.isArray(items) ? items : Array.from(items ?? []);
    const key = this.opts.key;
    const newKeys = new Array(list.length);
    for (let i = 0; i < list.length; i++) newKeys[i] = key(list[i], i);
    $.untrack(() => {
      this.list = list;
      if (this.view?.handle != null) {
        const ops = diffKeys(this.keys, newKeys);
        if (ops.length) this.#applyOps(ops, newKeys);
      }
      this.keys = newKeys;
      this.#refreshBound();
    });
  }

  #applyOps(ops, newKeys) {
    const present = new Set(newKeys);
    for (const k of this.keys) if (!present.has(k)) this.entries.delete(k);
    for (const k of newKeys) if (!this.entries.has(k)) this.entries.set(k, this.nextId++);
    for (const op of ops) {
      const ids = op.add.map((k) => this.entries.get(k));
      this.ids = spliceArray(this.ids, op.pos, op.removed, ids);
      this.#shiftBound(op.pos, op.removed, ids.length);
      this.view.call('splice', op.pos, op.removed, ids);
      this.splices++;
    }
  }

  /**
   * Enters model mode with `m` (or leaves it for null). The native list empties its id model either
   * way, so the id-mode state starts over; bound rows are rebound by the toolkit.
   */
  #setModel(m) {
    if (m === this.model) return;
    this.keys = [];
    this.ids = [];
    this.entries.clear();
    this.list = [];
    for (const s of this.slots.values()) s.pos = -1;
    this.model = m;
    this.opts.onmodel?.(m);
  }

  /** The row item at `pos` of the native model, `raw` the item handle the toolkit sent (GTK). */
  itemAt(pos, raw) {
    const custom = this.opts.item?.();
    const m = this.model;
    if (custom) return custom(m, pos, raw ?? null);
    if (raw !== null && raw !== undefined) return this.#wrapItem(raw);
    if (!m) return undefined;
    if (typeof m.getItem === 'function') return this.#wrapItem(m.getItem(pos));
    // Qt: a QStandardItemModel's rows are its items; any other model gives the row's QModelIndex.
    if (typeof m.itemFromIndex === 'function' && typeof m.item === 'function') return m.item(pos);
    if (typeof m.index === 'function') return m.index(pos, 0);
    return undefined;
  }

  #wrapItem(raw) {
    if (raw === null || raw === undefined) return null;
    const C = this.opts.itemClass?.();
    const w = raw instanceof NativeObject ? raw : wrapObject(C ?? NativeObject, raw);
    return C && w && !(w instanceof C) ? C.from(w) : w;
  }

  /** Model mode: the native model replaced `removed` items at `pos` with `added` (change number `epoch`). */
  itemsChanged(pos, removed, added, epoch) {
    if (this.destroyed || !this.model) return;
    this.modelChanges++;
    let moved = false;
    for (const s of this.slots.values()) {
      // A slot bound after this change already has the new position.
      if (s.pos < 0 || s.epoch >= epoch) continue;
      const before = s.pos;
      this.#shiftOne(s, pos, removed, added);
      s.epoch = epoch;
      if (s.pos !== before && s.pos >= 0 && s.unmountRow) {
        $.set(s.index, s.pos);
        moved = true;
      }
    }
    if (moved) flush();
  }

  /**
   * Model mode: row slot `row` (binding `gen`) kept its item while the model moved it to `pos` (a
   * sort, a move); reported by the toolkit after the change numbered `epoch`.
   */
  moved(row, pos, gen, epoch) {
    if (this.destroyed || !this.model) return;
    const s = this.slots.get(row);
    if (!s || s.gen !== gen || epoch < s.epoch) return;
    s.epoch = epoch;
    if (s.pos === pos) return;
    s.pos = pos;
    if (s.unmountRow) {
      $.set(s.index, pos);
      flush();
    }
  }

  /** Model mode (Qt): the data of rows `first..last` changed; bound rows re-read their item. */
  dataChanged(first, last, epoch) {
    if (this.destroyed || !this.model) return;
    let any = false;
    for (const s of this.slots.values()) {
      if (s.pos < first || s.pos > last || s.epoch !== epoch || !s.unmountRow) continue;
      $.set(s.item, this.itemAt(s.pos, null));
      $.set(s.ver, s.ver.v + 1);
      any = true;
    }
    if (any) flush();
  }

  #shiftOne(s, pos, removed, added) {
    if (removed) {
      if (s.pos >= pos + removed) s.pos -= removed;
      else if (s.pos >= pos) s.pos = -1;
    }
    if (added && s.pos >= pos) s.pos += added;
  }

  #shiftBound(pos, removed, added) {
    for (const s of this.slots.values()) {
      if (s.pos < 0) continue;
      if (removed) {
        if (s.pos >= pos + removed) s.pos -= removed;
        else if (s.pos >= pos) s.pos = -1;
      }
      if (added && s.pos >= pos) s.pos += added;
    }
  }

  /** Bound rows pick up a new item object or index; flushed with the update that called us. */
  #refreshBound() {
    for (const s of this.slots.values()) {
      if (s.pos < 0 || !s.unmountRow) continue;
      const item = this.list[s.pos];
      if (s.item.v !== item) $.set(s.item, item);
      if (s.index.v !== s.pos) $.set(s.index, s.pos);
    }
  }

  // ---- native events ----

  setup(row) {
    if (this.destroyed || this.slots.has(row) || !this.view) return;
    const s = { outlet: null, unmountOutlet: null, unmountRow: null, item: $.state(undefined), index: $.state(-1), ver: $.state(0), id: -1, pos: -1, gen: 0, epoch: 0 };
    // The outlet is a root of its own: it belongs to the slot, not to the component tree.
    s.unmountOutlet = mount(() => {
      s.outlet = element(rowOutlet());
    });
    this.slots.set(row, s);
    this.view.call('setRow', row, s.outlet);
  }

  bind(row, pos, id, gen, epoch, item) {
    if (this.destroyed) return;
    const s = this.slots.get(row);
    if (!s) return;
    if (this.model) {
      // A deferred bind older than one already applied (a sync bind overtook it) is stale.
      if (gen < s.gen) return;
      s.id = -1;
      s.pos = pos;
      s.gen = gen;
      s.epoch = epoch ?? 0;
      $.set(s.item, this.itemAt(pos, item));
      $.set(s.index, pos);
      this.#mountRow(s);
      flush();
      return;
    }
    if (this.ids[pos] !== id) {
      // A deferred bind that arrived after more splices: find where the id is now.
      const at = this.ids.indexOf(id);
      if (at < 0) return;
      pos = at;
    }
    s.id = id;
    s.pos = pos;
    s.gen = gen;
    $.set(s.item, this.list[pos]);
    $.set(s.index, pos);
    this.#mountRow(s);
    flush();
  }

  #mountRow(s) {
    if (s.unmountRow) return;
    const snippet = this.opts.row();
    const contexts = this.opts.context?.();
    // `ver` makes a row re-read an item whose data changed in place (a Qt dataChanged).
    const item = () => ($.get(s.ver), $.get(s.item));
    s.unmountRow = mount(
      ($$anchor) => {
        $.push({}, true);
        if (contexts) for (const [k, v] of contexts) setContext(k, v);
        snippet?.($$anchor, item, () => $.get(s.index));
        $.pop();
      },
      { target: s.outlet },
    );
  }

  unbind(row, gen) {
    const s = this.slots.get(row);
    if (!s || s.gen !== gen) return; // a late unbind of an older binding
    s.id = -1;
    s.pos = -1;
  }

  teardown(row) {
    const s = this.slots.get(row);
    if (!s) return;
    this.slots.delete(row);
    s.unmountRow?.();
    s.unmountOutlet();
  }

  activate(pos) {
    this.opts.onactivate?.(pos, this.model ? this.itemAt(pos, null) : this.list[pos]);
  }

  /** Position of the first item with `key`, or -1. */
  indexOfKey(key) {
    return this.keys.indexOf(key);
  }
}

const OWN_PROPS = new Set(['$$slots', '$$events', '$$legacy', 'items', 'model', 'item', 'itemClass', 'key', 'row', 'layout', 'selection', 'selected', 'row-height', 'onactivate', 'children']);

/**
 * `<ListView items key row layout selection bind:selected row-height onactivate ...rest>`, the
 * component API of @svelters/lists, in the shape the compiler generates for a component
 * (`ListView(container, $$props)`, returns its exports `{ scrollTo, controller }`). Other props go
 * to the native `virtual-list` unchanged (`vexpand`, `min-height`, ...).
 *
 * `items` (or `model`) may be a native list model instead of an array (model mode, N3): `key` is
 * then unused, rows get the model's items (`itemClass` wraps GTK items as that generated class;
 * `item(model, pos, raw)` overrides how a row's item is found), and the toolkit's view reads the
 * model directly.
 */
export function ListView($$anchor, $$props) {
  $.push($$props, true);
  installSyncDispatch();
  const key = $.prop($$props, 'key', 3, (_item, index) => index);
  const layout = $.prop($$props, 'layout', 3, 'list');
  const selection = $.prop($$props, 'selection', 3, 'single');
  const selected = $.prop($$props, 'selected', 31, () => -1);
  const rowHeight = $.prop($$props, 'row-height', 3, 32);
  const rest = $.rest_props($$props, OWN_PROPS);

  const contexts = getAllContexts();
  const ctl = new ListController({
    key: (item, index) => key()(item, index),
    row: () => $$props.row,
    context: () => contexts,
    onactivate: (pos, item) => $$props.onactivate?.(item, pos),
    itemClass: () => $$props.itemClass,
    item: () => $$props.item,
    onmodel: (m) => {
      if (m && !listeningModel) {
        // Only lists that show a native model listen: id-mode op logs stay as they were.
        listeningModel = true;
        on(view, 'items-changed', (pos, removed, added, epoch) => ctl.itemsChanged(pos, removed, added, epoch));
        on(view, 'data-changed', (first, last, epoch) => ctl.dataChanged(first, last, epoch));
        on(view, 'moved', (row, pos, gen, epoch) => ctl.moved(row, pos, gen, epoch));
      }
      if (view.handle != null) set(view, 'model', m ?? null);
    },
  });
  let listeningModel = false;
  let lastSelected = -1; // last position known natively: a native change is not echoed back

  const view = element('virtual-list');
  $.template_effect(() => set(view, 'layout', layout()));
  $.template_effect(() => set(view, 'selection', selection()));
  $.template_effect(() => set(view, 'row-height', rowHeight()));
  let restKeys = [];
  $.template_effect(() => {
    const keys = Object.keys(rest).filter((k) => !(k.startsWith('on') && typeof rest[k] === 'function'));
    for (const k of restKeys) if (!keys.includes(k)) set(view, k, null);
    for (const k of keys) set(view, k, rest[k]);
    restKeys = keys;
  });
  on(view, 'setup', lifecycle((r) => ctl.setup(r)));
  on(view, 'bind', lifecycle((r, pos, id, gen, epoch, item) => ctl.bind(r, pos, id, gen, epoch, item)));
  on(view, 'unbind', lifecycle((r, gen) => ctl.unbind(r, gen)));
  on(view, 'teardown', lifecycle((r) => ctl.teardown(r)));
  on(view, 'activate', (pos) => ctl.activate(pos));
  on(view, 'select', (pos) => {
    lastSelected = pos;
    selected(pos);
  });
  append($$anchor, view);
  ctl.attach(view);

  $.user_effect(() => ctl.update($$props.model ?? $$props.items));
  $.user_effect(() => {
    const sel = selected();
    if (sel !== lastSelected) {
      lastSelected = sel;
      $.untrack(() => view.handle != null && view.host.setProp(view.h, 'selected', sel));
    }
  });
  $.user_effect(() => () => ctl.detach());

  /** Scrolls item `index` into view. */
  function scrollTo(index) {
    if (view.handle != null) view.call('scrollTo', index);
  }
  /** The controller (tests and tools). */
  function controller() {
    return ctl;
  }
  return $.pop({ scrollTo, controller });
}
