// H1: the JS side of `import.meta.hot` (Vite's client API subset). Evaluated once per runtime when the
// provider compiles for HMR. Native code calls `ctx(id)` when a module is evaluated and, after a new
// version of a module evaluated without throwing, `update(id, ns)`, which commits that version
// (running the old version's dispose callbacks) and calls its accept callbacks. Only self-accepting
// modules are boundaries (`accept(cb)` / `acceptExports(names, cb)`).
(function () {
  const recs = new Map();
  const rec = (id) => {
    let r = recs.get(id);
    if (!r) recs.set(id, (r = { id, data: {}, cur: null, next: null, invalid: false }));
    return r;
  };
  globalThis.__svelters_hot = {
    ctx(id) {
      const r = rec(id);
      const v = { cbs: [], dispose: [] };
      if (r.cur) r.next = v;
      else r.cur = v;
      const noop = () => {};
      return {
        get data() {
          return r.data;
        },
        accept(a) {
          if (typeof a === 'function' || a === undefined) v.cbs.push(a ?? noop);
        },
        acceptExports(_names, cb) {
          v.cbs.push(cb ?? noop);
        },
        dispose(cb) {
          v.dispose.push(cb);
        },
        prune: noop,
        on: noop,
        off: noop,
        send: noop,
        invalidate() {
          r.invalid = true;
        },
      };
    },
    accepts(id) {
      const r = recs.get(id);
      return !!(r && r.cur && r.cur.cbs.length > 0);
    },
    // Returns null when the version was committed and its callbacks ran, else the reason to reload.
    update(id, ns) {
      const r = rec(id);
      if (r.invalid) {
        r.invalid = false;
        return `${id} called import.meta.hot.invalidate()`;
      }
      const v = r.next;
      if (!v) return `${id}: no new version was evaluated`;
      // the previous version's callbacks receive the new namespace (Vite's contract: the module
      // that was running accepts its replacement); the new version's replace them for the next one
      const old = r.cur;
      for (const d of old.dispose) d(r.data);
      r.cur = v;
      r.next = null;
      for (const cb of old.cbs) cb(ns);
      return null;
    },
  };
})();
