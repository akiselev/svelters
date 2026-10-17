// The JS half of a virtualized list: owns the id list that mirrors the native model, the keyed
// diff that keeps it in sync with the app's array, and the pool of mounted row components.
//
// Protocol (backend-abi.md, "Model-view protocol (M5)"): the native `virtuallist` holds only
// integer ids, one per item. `splice(pos, removed, ids)` changes them; the toolkit then asks for
// row content with `setup(row)`, `bind(row, pos, id, gen)`, `unbind(row, gen)`, `teardown(row)`.
import { flushSync, mount, unmount } from 'svelte';
import renderer, { outlet, releaseOutlet } from '@svelters/renderer';
import { diffKeys } from './diff.js';
import RowHost from './RowHost.svelte';

/** Reactive state of one row slot; the row component reads it, `bind` writes it. */
function createCell() {
  const cell = $state({ item: undefined, index: -1 });
  return cell;
}

/** `arr.splice` without spreading (a 100k-item insert would overflow the argument limit). */
function spliceArray(arr, pos, removed, add) {
  if (add.length < 4096) {
    arr.splice(pos, removed, ...add);
    return arr;
  }
  return arr.slice(0, pos).concat(add, arr.slice(pos + removed));
}

export class ListController {
  /**
   * @param {object} opts
   * @param {(item: any, index: number) => unknown} opts.key
   * @param {() => unknown} opts.row  current row snippet (a Svelte snippet)
   * @param {() => Map<any, any> | undefined} [opts.context]  context handed to row components
   * @param {(pos: number, item: any) => void} [opts.onactivate]
   * @param {(pos: number) => void} [opts.onselect]
   */
  constructor(opts) {
    this.opts = opts;
    this.view = null; // the <virtuallist> element
    this.keys = []; // key per position
    this.ids = []; // native id per position
    this.entries = new Map(); // key -> id
    this.list = []; // current items snapshot, by position
    this.nextId = 1;
    this.slots = new Map(); // row slot -> { outlet, cell, comp, id, pos, gen }
    this.destroyed = false;
    this.splices = 0; // native splice calls issued (tests, benchmarks)
  }

  /** Called with the `<virtuallist>` node once it exists. */
  attach(view) {
    this.view = view;
  }

  detach() {
    this.destroyed = true;
    for (const row of [...this.slots.keys()]) this.teardown(row);
    this.view = null;
  }

  /** Brings the native model in line with `items` using the smallest set of splices. */
  update(items) {
    const list = Array.isArray(items) ? items : Array.from(items ?? []);
    const key = this.opts.key;
    const newKeys = new Array(list.length);
    for (let i = 0; i < list.length; i++) newKeys[i] = key(list[i], i);
    this.list = list;
    if (this.view?.handle != null) {
      const ops = diffKeys(this.keys, newKeys);
      if (ops.length) this.#applyOps(ops, newKeys);
      this.keys = newKeys;
    } else {
      this.keys = newKeys;
    }
    this.#refreshBound();
  }

  #applyOps(ops, newKeys) {
    const present = new Set(newKeys);
    // Removed keys lose their id; moved keys keep theirs (their row is rebound at the new position).
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

  /** Bound slots at or after a splice move with it; slots inside the removed range lose their position. */
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

  /** Rows that stay bound pick up a new item object or a new index. */
  #refreshBound() {
    let dirty = false;
    for (const s of this.slots.values()) {
      if (s.pos < 0 || !s.comp) continue;
      const item = this.list[s.pos];
      if (s.cell.item !== item) {
        s.cell.item = item;
        dirty = true;
      }
      if (s.cell.index !== s.pos) {
        s.cell.index = s.pos;
        dirty = true;
      }
    }
    if (dirty) flushSync();
  }

  // ---- native events ----

  setup(row) {
    if (this.destroyed || this.slots.has(row)) return;
    const o = outlet('box');
    this.slots.set(row, { outlet: o, cell: createCell(), comp: null, id: -1, pos: -1, gen: 0 });
    this.view.call('setRow', row, o);
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
    s.cell.item = this.list[pos];
    s.cell.index = pos;
    if (!s.comp) {
      s.comp = mount(RowHost, {
        target: s.outlet,
        renderer,
        context: this.opts.context?.(),
        props: { row: this.opts.row(), cell: s.cell },
      });
    }
    flushSync();
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
    if (s.comp) unmount(s.comp);
    releaseOutlet(s.outlet);
  }

  activate(pos) {
    this.opts.onactivate?.(pos, this.list[pos]);
  }

  /** Position of the first item with `key`, or -1. */
  indexOfKey(key) {
    return this.keys.indexOf(key);
  }
}
