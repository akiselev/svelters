// @svelters/testhost: a JS SveltersHost that records the op log (plans/architecture/backend-abi.md,
// "Op-log format"), a virtual-time timer queue, and a runner for fixture bundles.
import vm from 'node:vm';
import fs from 'node:fs';

const defaultSchema = JSON.parse(fs.readFileSync(new URL('./schema.json', import.meta.url), 'utf8'));

/**
 * `schema` maps element tags to `{ prop: coarse type }` (`integer`, `number`, `boolean`, `string`,
 * `enum`, `flags`, `record`, ...). It stands in for the descriptors a real backend publishes, so the
 * renderer coerces attribute strings the same way on this host as on the native one. The default is
 * generated from the GTK metadata (`tools/metadata/gtk/gen_backend.py`).
 */
export function createTestHost({ schema = defaultSchema } = {}) {
  const lines = [];
  // Ids exist only to exercise the renderer's id path; the log always prints names.
  const kindNames = [null];
  const kindIds = new Map();
  const propNames = new Map(); // `${kid}:${id}` -> name
  const propIdOf = new Map(); // `${kid}:${name}` -> id
  const methodNames = new Map();
  const methodIdOf = new Map();
  const widgets = new Map(); // handle -> { kind, parent, children[], props: Map }
  const dead = new Set(); // handles that died (destroyed or gone): teardown-shaped ops on them are no-ops
  let next = 1;

  const w = (h) => {
    const x = widgets.get(h);
    if (!x) throw new Error(`unknown handle ${h}`);
    return x;
  };
  const json = (v) => {
    if (v === undefined) throw new TypeError('undefined is not a valid op value');
    return JSON.stringify(v, (_k, x) => (x !== null && typeof x === 'object' && typeof x.svlHandle === 'number' ? x.svlHandle : x));
  };

  const nameOf = (names, kid, ref) => (typeof ref === 'number' ? names.get(`${kid}:${ref}`) : ref);

  const host = {
    kindId(tag) {
      if (!kindIds.has(tag)) {
        kindNames.push(tag);
        kindIds.set(tag, kindNames.length - 1);
      }
      return kindIds.get(tag);
    },
    propId(kid, name) {
      const key = `${kid}:${name}`;
      if (!propIdOf.has(key)) {
        const id = propIdOf.size + 1;
        propIdOf.set(key, id);
        propNames.set(`${kid}:${id}`, name);
      }
      return propIdOf.get(key);
    },
    methodId(kid, name) {
      const key = `${kid}:${name}`;
      if (!methodIdOf.has(key)) {
        const id = methodIdOf.size + 1;
        methodIdOf.set(key, id);
        methodNames.set(`${kid}:${id}`, name);
      }
      return methodIdOf.get(key);
    },
    propType(kid, pid) {
      return schema[kindNames[kid]]?.[nameOf(propNames, kid, pid)] ?? null;
    },
    create(kindRef) {
      const kind = typeof kindRef === 'number' ? kindNames[kindRef] : kindRef;
      const h = next++;
      widgets.set(h, { kind, kid: typeof kindRef === 'number' ? kindRef : null, parent: null, children: [], props: new Map() });
      lines.push(`create ${kind} #${h}`);
      return h;
    },
    insert(parent, child, before, slot) {
      const p = w(parent);
      const c = w(child);
      if (c.parent != null) throw new Error(`insert: #${child} already has parent #${c.parent}`);
      let i = p.children.length;
      if (before != null) {
        i = p.children.indexOf(before);
        if (i < 0) throw new Error(`insert: #${before} is not a child of #${parent}`);
      }
      p.children.splice(i, 0, child);
      c.parent = parent;
      lines.push(`insert #${parent} #${child} before=${before == null ? 'end' : '#' + before} slot=${slot ?? 'default'}`);
    },
    remove(parent, child) {
      if (dead.has(parent) || dead.has(child)) return;
      const p = w(parent);
      const c = w(child);
      if (c.parent !== parent) throw new Error(`remove: #${child} is not a child of #${parent}`);
      p.children.splice(p.children.indexOf(child), 1);
      c.parent = null;
      c.attrs?.clear();
      lines.push(`remove #${parent} #${child}`);
    },
    destroy(h) {
      if (dead.has(h)) return;
      const x = w(h);
      // destroy only drops the handle; a child destroyed before its parent is dropped from the model
      if (x.parent != null) {
        const p = widgets.get(x.parent);
        if (p) p.children.splice(p.children.indexOf(h), 1);
      }
      widgets.delete(h);
      dead.add(h);
      lines.push(`destroy #${h}`);
    },
    setProp(h, nameRef, value) {
      if (dead.has(h)) return;
      const name = nameOf(propNames, w(h).kid, nameRef);
      w(h).props.set(name, value);
      lines.push(`set #${h} ${name} ${json(value)}`);
    },
    getProp(h, nameRef) {
      return w(h).props.get(nameOf(propNames, w(h).kid, nameRef)) ?? null;
    },
    call(h, methodRef, args) {
      const method = nameOf(methodNames, w(h).kid, methodRef);
      lines.push(`call #${h} ${method} ${json(args)}`);
      return null;
    },
    listen(h, signal, on, flags = 0) {
      if (!on && dead.has(h)) return;
      w(h);
      lines.push(`listen #${h} ${signal} ${on ? 'on' : 'off'}${flagsSuffix(flags)}`);
    },
    setChildAttr(parent, child, nameRef, value) {
      if (dead.has(parent) || dead.has(child)) return;
      if (w(child).parent !== parent) throw new Error(`setChildAttr: #${child} is not a child of #${parent}`);
      const name = typeof nameRef === 'string' ? nameRef : `attr#${nameRef}`;
      const attrs = (w(child).attrs ??= new Map());
      if (value === null || value === undefined) attrs.delete(name);
      else attrs.set(name, value);
      lines.push(`attr #${parent} #${child} ${name} ${json(value ?? null)}`);
    },
    getChildAttr(parent, child, nameRef) {
      return w(child).attrs?.get(nameRef) ?? null;
    },
  };

  /**
   * Simulates the native death of `h`: logs `gone #h <reason>` for it and then for every
   * descendant (pre-order, reason `parent`), and returns that list of [handle, reason].
   */
  function killNative(h, reason) {
    const root = w(h);
    if (root.parent != null) {
      const p = widgets.get(root.parent);
      if (p) p.children.splice(p.children.indexOf(h), 1);
    }
    const out = [];
    const walk = (x, why) => {
      out.push([x, why]);
      for (const c of widgets.get(x).children) walk(c, 'parent');
    };
    walk(h, reason);
    for (const [x, why] of out) {
      lines.push(`gone #${x} ${why}`);
    }
    for (const [x] of out) {
      widgets.delete(x);
      dead.add(x);
    }
    return out;
  }

  return { host, lines, text: () => lines.map((l) => l + '\n').join(''), widgets, killNative };
}

const FLAG_WORDS = { after: 1, capture: 2 };
/** Listener flags as the trailing op-log word (` after`, ` capture`, ` after+capture`); empty for 0. */
const flagsSuffix = (flags) => (flags ? ` ${[flags & 1 ? 'after' : '', flags & 2 ? 'capture' : ''].filter(Boolean).join('+')}` : '');
const flagsOfWord = (word) => (word ? word.split('+').reduce((f, x) => f | (FLAG_WORDS[x] ?? 0), 0) : 0);

/** Virtual-time timers: deterministic, ordered by (due time, creation order). */
export function createTimers() {
  let now = 0;
  let seq = 0;
  const pending = new Map(); // id -> { due, seq, fn, every }
  let nextId = 1;

  const add = (fn, ms, every) => {
    const id = nextId++;
    const delay = Math.max(0, Number(ms) || 0);
    pending.set(id, { due: now + delay, seq: seq++, fn, every: every ? Math.max(1, delay) : 0 });
    return id;
  };

  return {
    setTimeout: (fn, ms) => add(fn, ms, false),
    setInterval: (fn, ms) => add(fn, ms, true),
    clearTimeout: (id) => void pending.delete(id),
    clearInterval: (id) => void pending.delete(id),
    get size() { return pending.size; },
    /** Fire the earliest timer; false when none are pending. */
    fireNext() {
      let best = null;
      let bestId = 0;
      for (const [id, t] of pending) {
        if (!best || t.due < best.due || (t.due === best.due && t.seq < best.seq)) {
          best = t;
          bestId = id;
        }
      }
      if (!best) return false;
      now = best.due;
      if (best.every) {
        best.due = now + best.every;
        best.seq = seq++;
      } else {
        pending.delete(bestId);
      }
      best.fn();
      return true;
    },
  };
}

const MAX_TIMER_FIRINGS = 10_000;

/**
 * Run a fixture bundle in a fresh vm context that only has the globals a QuickJS host provides
 * (no Node globals), then play `script`. Returns the op-log text.
 *
 * Step semantics (the contract P2 must reproduce):
 *   { event: [h, signal, args] }  append `event #h signal args` to the log, then call
 *                                 globalThis.__svelters_dispatch(h, signal, args) synchronously.
 *   { sync: [h, signal, args] }  append `sync #h signal args`, call globalThis.__svelters_dispatch_sync(h, signal, args)
 *                                 synchronously (no checkpoint), append `result #h <json of the result>`.
 *   { gone: [h, reason] }         the native object behind h died: log `gone #h reason` for h and each
 *                                 descendant (pre-order, reason `parent`), then call
 *                                 globalThis.__svelters_gone(handle, reason) for each, synchronously.
 *   { checkpoint: true }          loop { drain all promise/microtask jobs; if timers are pending,
 *                                 fire the earliest one (advancing virtual time); else stop }.
 * The bundle is evaluated before step 1, with no implicit checkpoint.
 */
export async function runBundle({ bundlePath, script }) {
  const th = createTestHost();
  const timers = createTimers();
  const errors = [];
  const onRejection = (e) => errors.push(e);
  process.on('unhandledRejection', onRejection);

  const ctx = vm.createContext({
    console,
    queueMicrotask,
    setTimeout: timers.setTimeout,
    setInterval: timers.setInterval,
    clearTimeout: timers.clearTimeout,
    clearInterval: timers.clearInterval,
    __svelters_host: th.host,
  });

  const drain = () => new Promise((r) => setImmediate(r));
  const fail = () => {
    if (errors.length) throw errors[0];
  };

  try {
    const mod = new vm.SourceTextModule(fs.readFileSync(bundlePath, 'utf8'), { context: ctx, identifier: bundlePath });
    await mod.link((spec) => {
      throw new Error(`bundle must be self-contained, found import of ${spec}`);
    });
    await mod.evaluate();

    for (const step of script) {
      if (step.event) {
        const [h, signal, args, fw] = step.event;
        const flags = flagsOfWord(fw);
        th.lines.push(`event #${h} ${signal} ${JSON.stringify(args)}${flagsSuffix(flags)}`);
        ctx.__svelters_dispatch(h, signal, args, false, flags);
      } else if (step.sync) {
        const [h, signal, args, fw] = step.sync;
        const flags = flagsOfWord(fw);
        th.lines.push(`sync #${h} ${signal} ${JSON.stringify(args)}${flagsSuffix(flags)}`);
        const r = ctx.__svelters_dispatch_sync(h, signal, args, flags);
        th.lines.push(`result #${h} ${JSON.stringify(r ?? null)}`);
      } else if (step.gone) {
        const [h, reason] = step.gone;
        for (const [x, why] of th.killNative(h, reason)) ctx.__svelters_gone?.(x, why);
      } else if (step.checkpoint) {
        let fired = 0;
        for (;;) {
          await drain();
          fail();
          if (!timers.fireNext()) break;
          if (++fired > MAX_TIMER_FIRINGS) throw new Error('checkpoint: timer storm');
        }
      } else {
        throw new Error(`unknown script step ${JSON.stringify(step)}`);
      }
    }
    await drain();
    fail();
  } finally {
    process.off('unhandledRejection', onRejection);
  }
  return th.text();
}
