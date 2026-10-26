const host = globalThis.__svelters_host;

const fail = (e) => {
  const m = /^([a-z]+\/[a-z]+): ([\s\S]*)$/.exec(String(e && e.message));
  if (!m) return e;
  const err = new Error(m[2]);
  err.code = m[1];
  return err;
};
const call = (op, a = '', b = '') => {
  try {
    return host.dx(op, a, b);
  } catch (e) {
    throw fail(e);
  }
};

// Reactivity rides the runtime's tracker (`@svelters/types/core` `installTracker`, installed by
// `@svelters/native-runtime`): `local(h, prop)` subscribes the running effect, `invalidate(h, prop)`
// re-runs it. Here `h` is a fixed tag and `prop` names the key. Without a tracker reads are plain.
const TRACKER = Symbol.for('svelters.types.tracker');
const TAG = 'svelters:settings';
const watch = (prop) => globalThis[TRACKER]?.local(TAG, prop);
const touch = (prop) => globalThis[TRACKER]?.invalidate(TAG, prop);
const keys = new Set();

/**
 * Re-reads the given keys (all that were read when omitted) after the backend changed them from outside.
 * @param {string[]} [list]
 */
export function refresh(list) {
  for (const k of list ?? [...keys]) touch('s:' + k);
}

// SV63: the host calls this at a checkpoint with the keys its settings backend reported changed.
globalThis.__svelters_settings_changed = (list) => refresh(list);

/**
 * A reactive setting. `value` reads the backend (tracked); assigning writes it and throws an error
 * with `code === 'settings/locked'` when the key is locked. `source` is `{ layer, locked }` or null;
 * `locked` is true when writes are rejected.
 * @template T
 * @param {string} key
 * @param {T} [fallback] returned while the backend has no value
 */
export function setting(key, fallback) {
  if (typeof key !== 'string' || !key) throw new TypeError('setting(key): key must be a non-empty string');
  keys.add(key);
  return {
    get key() {
      return key;
    },
    get value() {
      watch('s:' + key);
      const s = call('settings.get', key);
      return s === '' ? fallback : JSON.parse(s);
    },
    set value(v) {
      call('settings.set', key, JSON.stringify(v));
      touch('s:' + key);
    },
    get source() {
      watch('s:' + key);
      const s = call('settings.source', key);
      return s === '' ? null : JSON.parse(s);
    },
    get locked() {
      return this.source?.locked === true;
    },
  };
}

const DEBOUNCE_MS = 250;
const roots = new Map(); // root -> { state, timer, subs }

const rootState = (root) => {
  let r = roots.get(root);
  if (!r) {
    r = { state: JSON.parse(call('ui.load', root)), timer: null };
    roots.set(root, r);
  }
  return r;
};

const save = (root) => {
  const r = roots.get(root);
  if (r.timer !== null) clearTimeout(r.timer);
  r.timer = null;
  call('ui.save', root, JSON.stringify(r.state));
};

/** Writes every root with pending changes now (call before quitting). */
export function flushUiState() {
  for (const [root, r] of roots) if (r.timer !== null) save(root);
}

const proxy = (r, root, anchor, value, path) =>
  new Proxy(value, {
    get(t, p, rc) {
      watch('u:' + root);
      const v = Reflect.get(t, p, rc);
      return v !== null && typeof v === 'object' ? proxy(r, root, anchor, v, path.concat(p)) : v;
    },
    set(t, p, v) {
      const ok = Reflect.set(t, p, v);
      r.timer ??= setTimeout(() => save(root), DEBOUNCE_MS);
      touch('u:' + root);
      return ok;
    },
  });

/**
 * Persisted UI state (split positions, column widths, collapsed sections): a plain object whose
 * fields are reactive and survive restarts. Keyed by `anchor` within `root`. Defaults fill the
 * fields the stored state lacks. Saves are debounced; `flushUiState()` writes at once.
 * @template T extends object
 * @param {T} defaults
 * @param {string} anchor stable name of the component or panel instance
 * @param {{ root?: string }} [opts] `root` defaults to `"app"`
 * @returns {T}
 */
export function uiState(defaults, anchor, opts = {}) {
  if (typeof anchor !== 'string' || !anchor) throw new TypeError('uiState(defaults, anchor): anchor must be a non-empty string');
  const root = opts.root ?? 'app';
  const r = rootState(root);
  const stored = r.state[anchor];
  const merged = { ...JSON.parse(JSON.stringify(defaults)), ...(stored && typeof stored === 'object' ? stored : {}) };
  r.state[anchor] = merged;
  return proxy(r, root, anchor, merged, []);
}
