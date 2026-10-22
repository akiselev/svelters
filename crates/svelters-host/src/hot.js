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
      const v = { cbs: [], dispose: [], all: false, names: new Set() };
      if (r.cur) r.next = v;
      else {
        r.cur = v;
        // the first version's exports, for acceptExports' "everything else unchanged" check; the module
        // has finished evaluating by the time this runs. `id` is a module id, not a specifier: the
        // host resolves an import from this script to that id as is (LD3), generated ids too.
        Promise.resolve()
          .then(() => import(id))
          .then((ns) => {
            if (!r.ns) r.ns = { ...ns };
          })
          .catch(() => {});
      }
      const noop = () => {};
      return {
        get data() {
          return r.data;
        },
        accept(a) {
          if (typeof a === 'function' || a === undefined) {
            // accept(cb) accepts every export (acceptExports of all names)
            v.all = true;
            v.cbs.push(a ?? noop);
          }
        },
        acceptExports(names, cb) {
          for (const n of [].concat(names ?? [])) v.names.add(n);
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
    // Whether the new namespace `ns` of `id` is covered by what the running version accepts: with
    // acceptExports(names), every other export must be unchanged (Vite's rule). Null when it is,
    // else the reason. Without side effects; asked for every boundary before any commit.
    check(id, ns) {
      const r = recs.get(id);
      const cur = r && r.cur;
      if (!cur || cur.all) return null;
      const extra = new Set([...Object.keys(ns), ...(r.ns ? Object.keys(r.ns) : [])].filter((k) => !cur.names.has(k)));
      if (extra.size === 0) return null;
      if (!r.ns) return `${id} exports ${[...extra].join(', ')} besides what it accepts, and its previous exports are unknown`;
      for (const k of extra) if (r.ns[k] !== ns[k]) return `${id}: export ${k} changed, but only ${[...cur.names].join(', ')} is accepted`;
      return null;
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
      r.ns = { ...ns };
      for (const d of old.dispose) d(r.data);
      r.cur = v;
      r.next = null;
      for (const cb of old.cbs) cb(ns);
      return null;
    },
  };
})();
