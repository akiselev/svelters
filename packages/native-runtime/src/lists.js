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
import * as $ from 'svelte/internal/client';
// Svelte's client context functions (the public 'svelte' entry resolves to the server build
// without the browser condition); resolved by the alias in ../vite.js and test/register.mjs.
import { getAllContexts, setContext } from 'svelte/internal/client/context';
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
    const s = { outlet: null, unmountOutlet: null, unmountRow: null, item: $.state(undefined), index: $.state(-1), id: -1, pos: -1, gen: 0 };
    // The outlet is a root of its own: it belongs to the slot, not to the component tree.
    s.unmountOutlet = mount(() => {
      s.outlet = element(rowOutlet());
    });
    this.slots.set(row, s);
    this.view.call('setRow', row, s.outlet);
  }

  bind(row, pos, id, gen) {
    if (this.destroyed) return;
    const s = this.slots.get(row);
    if (!s) return;
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
    if (!s.unmountRow) {
      const snippet = this.opts.row();
      const contexts = this.opts.context?.();
      s.unmountRow = mount(
        ($$anchor) => {
          $.push({}, true);
          if (contexts) for (const [k, v] of contexts) setContext(k, v);
          snippet?.($$anchor, () => $.get(s.item), () => $.get(s.index));
          $.pop();
        },
        { target: s.outlet },
      );
    }
    $.flush();
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
    this.opts.onactivate?.(pos, this.list[pos]);
  }

  /** Position of the first item with `key`, or -1. */
  indexOfKey(key) {
    return this.keys.indexOf(key);
  }
}

const OWN_PROPS = new Set(['$$slots', '$$events', '$$legacy', 'items', 'key', 'row', 'layout', 'selection', 'selected', 'row-height', 'onactivate', 'children']);

/**
 * `<ListView items key row layout selection bind:selected row-height onactivate ...rest>`, the
 * component API of @svelters/lists, in the shape the compiler generates for a component
 * (`ListView(container, $$props)`, returns its exports `{ scrollTo, controller }`). Other props go
 * to the native `virtual-list` unchanged (`vexpand`, `min-height`, ...).
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
  });
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
  on(view, 'bind', lifecycle((r, pos, id, gen) => ctl.bind(r, pos, id, gen)));
  on(view, 'unbind', lifecycle((r, gen) => ctl.unbind(r, gen)));
  on(view, 'teardown', lifecycle((r) => ctl.teardown(r)));
  on(view, 'activate', (pos) => ctl.activate(pos));
  on(view, 'select', (pos) => {
    lastSelected = pos;
    selected(pos);
  });
  append($$anchor, view);
  ctl.attach(view);

  $.user_effect(() => ctl.update($$props.items));
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
