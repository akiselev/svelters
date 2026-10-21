// @svelters/testhost: a JS SveltersHost that records the op log (plans/architecture/backend-abi.md,
// "Op-log format"), a virtual-time timer queue, and a runner for fixture bundles.
import vm from 'node:vm';
import fs from 'node:fs';

const readSchema = (file) => JSON.parse(fs.readFileSync(new URL(file, import.meta.url), 'utf8'));
/** Coarse prop types per toolkit vocabulary: `gtk` (the default) and `qt` (Qt-vocabulary fixtures). */
export const schemas = { gtk: readSchema('./schema.json'), qt: readSchema('./schema.qt.json') };
const defaultSchema = schemas.gtk;

/**
 * `schema` maps element tags to `{ prop: coarse type }` (`integer`, `number`, `boolean`, `string`,
 * `enum`, `flags`, `record`, ...). It stands in for the descriptors a real backend publishes, so the
 * renderer coerces attribute strings the same way on this host as on the native one. The default is
 * generated from the GTK metadata (`tools/metadata/gtk/gen_backend.py`); `toolkit: 'qt'` picks the one
 * generated from the Qt metadata (`tools/metadata/qt/gen_cpp.py --schema`).
 */
const COARSE = { i32: 'integer', u32: 'integer', i64: 'integer', f64: 'number', bool: 'boolean', string: 'string', enum: 'enum', flags: 'flags', record: 'record' };

/**
 * A toolkit's member tables from its semantic spec (metadata/semantics/<toolkit>.json): per tag,
 * the props, signals and methods of the kind chain, plus the coarse prop types (`schema`).
 */
export function toolkitTables(toolkit) {
  const spec = JSON.parse(fs.readFileSync(new URL(`../../../metadata/semantics/${toolkit}.json`, import.meta.url), 'utf8'));
  const tags = {};
  const schema = {};
  for (const [tag, kid] of Object.entries(spec.tags)) {
    const t = (tags[tag] = { props: new Set(), signals: new Set(), methods: new Set() });
    const types = (schema[tag] = {});
    for (const c of spec.kinds[kid].chain) {
      const k = spec.kinds[c];
      if (!k) continue;
      for (const p of Object.values(k.props)) {
        for (const n of [p.js, p.native]) {
          t.props.add(n);
          if (COARSE[p.type] && !(n in types)) types[n] = COARSE[p.type];
        }
      }
      for (const sg of Object.values(k.signals)) t.signals.add(sg.name).add(sg.js_event);
      for (const m of Object.keys(k.methods ?? {})) t.methods.add(m);
    }
  }
  return { tags, schema };
}

/**
 * `toolkit` (`gtk`, `qt`, `appkit`, `winui`): resolve names like that toolkit's backend does
 * (toolkitTables): `propId`/`signalId`/`methodId` answer null for a name the kind does not have,
 * so the renderer picks the toolkit's own candidates (text props, binding variants, `click` vs
 * `clicked`). Without it every name resolves (the GTK-shaped default).
 */
export function createTestHost({ schema: schemaOpt, toolkit } = {}) {
  const tables = toolkit ? toolkitTables(toolkit) : null;
  const schema = schemaOpt ?? tables?.schema ?? defaultSchema;
  // A kind the toolkit's spec does not know (synthetic: virtual-list, canvas, GTK menus) resolves any name.
  const knows = (kid, set, name) => !tables || !tables.tags[kindNames[kid]] || tables.tags[kindNames[kid]][set].has(name);
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
  // ABI v2.6 observations: `${h}:${prop}` -> { count, name, last }. Like the QuickJS host, only the
  // 0->1 and 1->0 transitions reach the "backend" (the log), and a native change is delivered only
  // when its value differs from `last`: the value at the first observation, after our own setProp,
  // or as last delivered (REACTIVITY.md section 5.2; the TestHost has no proportional props).
  const observed = new Map();
  const forgetObserved = (h) => {
    for (const k of [...observed.keys()]) if (k.startsWith(`${h}:`)) observed.delete(k);
  };
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

  let nodeFor = null;
  const asNode = (h) => (nodeFor ? nodeFor(h) : h);
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
      if (!knows(kid, 'props', name)) return null;
      const key = `${kid}:${name}`;
      if (!propIdOf.has(key)) {
        const id = propIdOf.size + 1;
        propIdOf.set(key, id);
        propNames.set(`${kid}:${id}`, name);
      }
      return propIdOf.get(key);
    },
    methodId(kid, name) {
      if (!knows(kid, 'methods', name)) return null;
      const key = `${kid}:${name}`;
      if (!methodIdOf.has(key)) {
        const id = methodIdOf.size + 1;
        methodIdOf.set(key, id);
        methodNames.set(`${kid}:${id}`, name);
      }
      return methodIdOf.get(key);
    },
    // Only with a toolkit: signals have no ids, so this answers whether the kind has `name`.
    ...(tables ? { signalId: (kid, name) => (knows(kid, 'signals', name) ? name : null) } : {}),
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
    // ABI v2.5: construct and call_static. A handle is returned as the renderer's node when the bundle
    // installed one (like the QuickJS host), else as the bare number.
    construct(kindRef, props) {
      const kind = typeof kindRef === 'number' ? kindNames[kindRef] : kindRef;
      const h = next++;
      widgets.set(h, { kind, kid: null, parent: null, children: [], props: new Map(Object.entries(props ?? {})) });
      lines.push(`construct #${h} ${kind} ${json(props ?? {})}`);
      return asNode(h);
    },
    callStatic(kindRef, methodRef, args) {
      const kind = typeof kindRef === 'number' ? kindNames[kindRef] : kindRef;
      const method = typeof methodRef === 'number' ? `#${methodRef}` : methodRef;
      const line = `static ${kind}.${method} ${json(args)}`;
      // A constructor-shaped name (an ObjC `init…:` too) answers a fresh handle (the log backend does the same); the rest null.
      if (method === 'new' || method.startsWith('new_') || method.startsWith('from_') || (method.startsWith('init') && method.endsWith(':'))) {
        const h = next++;
        widgets.set(h, { kind, kid: null, parent: null, children: [], props: new Map() });
        lines.push(`${line} #${h}`);
        return asNode(h);
      }
      lines.push(line);
      return null;
    },
    typeChain() {
      return [];
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
      forgetObserved(h);
      lines.push(`destroy #${h}`);
    },
    setProp(h, nameRef, value) {
      if (dead.has(h)) return;
      const name = nameOf(propNames, w(h).kid, nameRef);
      w(h).props.set(name, value);
      lines.push(`set #${h} ${name} ${json(value)}`);
      // The set path: the read-back becomes `last`, so the echo of our own write is dropped.
      const o = observed.get(`${h}:${name}`);
      if (o) o.last = value ?? null;
    },
    getProp(h, nameRef) {
      return w(h).props.get(nameOf(propNames, w(h).kid, nameRef)) ?? null;
    },
    call(h, methodRef, args) {
      const method = nameOf(methodNames, w(h).kid, methodRef);
      lines.push(`call #${h} ${method} ${json(args)}`);
      return null;
    },
    // ABI v2.6: refcounted per (h, prop); answers whether (h, prop) is observed afterwards.
    observe(h, propRef, on) {
      if (!on && dead.has(h)) return false;
      const name = nameOf(propNames, w(h).kid, propRef);
      const key = `${h}:${name}`;
      const o = observed.get(key);
      const n = o?.count ?? 0;
      if (on) {
        if (o) o.count++;
        else {
          observed.set(key, { count: 1, name: typeof propRef === 'string' ? propRef : name, last: w(h).props.get(name) ?? null });
          lines.push(`observe #${h} ${name} on`);
        }
        return true;
      }
      if (n === 0) return false;
      if (n > 1) {
        o.count--;
        return true;
      }
      observed.delete(key);
      lines.push(`observe #${h} ${name} off`);
      return false;
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
    // Styling (ABI v2.3). The op numbers and payload formats are in plans/architecture/backend-abi.md.
    // The test host speaks the CSS dialect (target 1), like the log backend.
    style(op, h, text) {
      const t = text === undefined || text === null ? 'null' : JSON.stringify(String(text));
      switch (op) {
        case 0:
          return 1;
        case 1:
        case 2:
        case 3:
        case 4:
          if (h !== 0 && dead.has(h)) return 0;
          w(h);
          lines.push(`style #${h} ${['classes', 'name', 'inline', 'opacity'][op - 1]} ${t}`);
          return 0;
        case 5: {
          const s = text === undefined || text === null ? null : String(text);
          const i = s === null ? -1 : s.indexOf('\u0001');
          const id = s === null ? '*' : i < 0 ? s : s.slice(0, i);
          const body = i < 0 ? 'null' : JSON.stringify(s.slice(i + 1));
          lines.push(`style sheet ${JSON.stringify(id)} ${body}`);
          return 0;
        }
        case 6: {
          const parts = String(text ?? 'light').split('\u0001');
          lines.push(`style scheme ${JSON.stringify(parts[0])} ${parts.length - 1}`);
          return 0;
        }
        default:
          throw new Error(`unknown style op ${op}`);
      }
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
      forgetObserved(x);
    }
    return out;
  }

  return {
    host,
    lines,
    text: () => lines.map((l) => l + '\n').join(''),
    widgets,
    killNative,
    /** Whether JS observes `prop` of `h` (ABI v2.6): the host delivers `changed` steps only then. */
    isObserved: (h, prop) => observed.has(`${h}:${prop}`),
    /**
     * A scripted native change (ABI v2.6): `value` (when not undefined) becomes the native value, the
     * `changed` line is logged, and the `__svelters_changed` batch the host would deliver is returned:
     * `[h, prop, value]` when JS observes the prop and the value differs from the last one, else null.
     */
    nativeChange(h, prop, value) {
      if (value !== undefined) w(h).props.set(prop, value);
      lines.push(value === undefined ? `changed #${h} ${prop}` : `changed #${h} ${prop} ${json(value)}`);
      const o = observed.get(`${h}:${prop}`);
      if (!o) return null;
      const v = w(h).props.get(prop) ?? null;
      if (sameValue(o.last, v)) return null;
      o.last = v;
      return [h, o.name, v];
    },
    /** Hands results that are handles to `fn` (the vm context's `__svelters_node`). */
    setNodeFor: (fn) => void (nodeFor = fn),
  };
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
 * (no Node globals), then play `script`. Returns the op-log text. `toolkit` (`gtk` or `qt`) picks the
 * prop schema and the style dialect the host answers (`css` / `qss`); `styleDialect` overrides the latter.
 *
 * Step semantics (the contract P2 must reproduce):
 *   { event: [h, signal, args] }  append `event #h signal args` to the log, then call
 *                                 globalThis.__svelters_dispatch(h, signal, args) synchronously.
 *   { sync: [h, signal, args] }  append `sync #h signal args`, call globalThis.__svelters_dispatch_sync(h, signal, args)
 *                                 synchronously (no checkpoint), append `result #h <json of the result>`.
 *   { deferred: [h, signal, args] } a sync signal the toolkit could not deliver synchronously (JS was
 *                                 busy) and queued instead: append `deferred #h signal args`, call
 *                                 globalThis.__svelters_dispatch(h, signal, args, true) (result ignored).
 *   { changed: [h, prop, value?] } ABI v2.6: prop of h changed natively to `value` (stored when given)
 *                                 and the toolkit announces it: append `changed #h prop <json>`, then,
 *                                 if JS observes it (host.observe) and the value differs from the last
 *                                 one delivered or written, call globalThis.__svelters_changed([h, prop,
 *                                 value]) synchronously (the host's batch of one).
 *   { gone: [h, reason] }         the native object behind h died: log `gone #h reason` for h and each
 *                                 descendant (pre-order, reason `parent`), then call
 *                                 globalThis.__svelters_gone(handle, reason) for each, synchronously.
 *   { checkpoint: true }          loop { drain all promise/microtask jobs; if timers are pending,
 *                                 fire the earliest one (advancing virtual time); else stop }.
 * The bundle is evaluated before step 1, with no implicit checkpoint.
 */
/** The host's compare (REACTIVITY.md section 5.2): numbers by value with NaN equal to NaN, the rest deeply. */
export function sameValue(a, b) {
  if (Object.is(a, b)) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  return ka.length === kb.length && ka.every((k) => Object.hasOwn(b, k) && sameValue(a[k], b[k]));
}

export async function runBundle({ bundlePath, script, toolkit, styleDialect = toolkit === 'qt' ? 'qss' : 'css' }) {
  const th = createTestHost({ toolkit });
  if (styleDialect === 'qss') {
    // answer the Qt dialect (style target 2), so the bundle applies the Qt style table
    const style = th.host.style;
    th.host.style = (op, h, text) => (op === 0 ? 2 : style(op, h, text));
  }
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
  th.setNodeFor((h) => (typeof ctx.__svelters_node === 'function' ? ctx.__svelters_node(h) : h));

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
      } else if (step.deferred) {
        const [h, signal, args] = step.deferred;
        th.lines.push(`deferred #${h} ${signal} ${JSON.stringify(args)}`);
        ctx.__svelters_dispatch(h, signal, args, true, 0);
      } else if (step.changed) {
        const [h, prop, value] = step.changed;
        const batch = th.nativeChange(h, prop, value);
        if (batch) ctx.__svelters_changed?.(batch);
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
