// @svelters/types core: the hand-written object model under the generated `svelters:<toolkit>/<ns>`
// modules (plans/research/NON-GUI-TYPES.md, section 2). Generated code only calls what this file
// exports and the host ops (`globalThis.__svelters_host`): `construct`, `callStatic`, `call`,
// `getProp`, `setProp`, `listen`, `destroy`.
//
// - `NativeObject`: one JS object per native object (identity), backed by a SHARED handle. Dropping
//   the wrapper lets the renderer's finalizer release our reference; `dispose()` releases it now.
// - `NativeValue`: an immutable VALUE handle (a boxed struct): methods, `equals`, `toString`.
// - No registration: a returned object is wrapped as the class the generated signature declares, so
//   a bundler can drop every class nobody imports. Nothing in here runs at import time.
//
// Plain ES2022, no dependencies.

/** Passed as the first constructor argument by `wrapObject` / `wrapValue`: "wrap this handle". */
export const WRAP = Symbol.for('svelters.types.wrap');
/** Static hook every generated class defines: builds the native instance for `new Class(...)`. */
export const CONSTRUCT = Symbol.for('svelters.types.construct');

const NODE = Symbol('svelters.types.node');
const BOX = Symbol('svelters.types.box');
const HOOKED = Symbol.for('svelters.types.dispatch-hook');

const host = () => {
  const h = globalThis.__svelters_host;
  if (!h) throw new Error('@svelters/types: no native host is installed (globalThis.__svelters_host)');
  return h;
};

/** Thrown when an object or value is used after `dispose()` (or after its handle went stale). */
export class DisposedError extends Error {
  constructor(what) {
    super(`${what} was disposed`);
    this.name = 'StaleHandleError';
    this.reason = 'disposed';
  }
}

/** The numeric handle behind a node, a wrapper or a number. */
export function handleOf(x) {
  if (typeof x === 'number') return x;
  if (x && typeof x === 'object') {
    const h = x.svlHandle;
    if (typeof h === 'number') return h;
  }
  return null;
}

// ---- identity -------------------------------------------------------------------------------------

// handle -> WeakRef<wrapper>. The renderer keeps one node per handle; this keeps one wrapper per node.
const wrappers = new Map();
const swept = typeof FinalizationRegistry === 'function' ? new FinalizationRegistry((h) => {
  const ref = wrappers.get(h);
  if (ref && !ref.deref()) wrappers.delete(h);
}) : null;

/** The wrapper of the live native object `raw` (a node, or a bare handle number), as `Class` or one of its bases. */
export function wrapObject(Class, raw) {
  if (raw === null || raw === undefined) return null;
  const h = handleOf(raw);
  if (h === null) return null;
  const ref = wrappers.get(h);
  const have = ref?.deref();
  if (have) {
    // The same object seen through a more specific declared type: keep the wrapper (identity) and
    // give it the richer class.
    if (Class !== have.constructor && !(have instanceof Class) && Class.bases?.includes(have.constructor.native)) {
      Object.setPrototypeOf(have, Class.prototype);
    }
    return have;
  }
  const w = new Class(WRAP, raw);
  wrappers.set(h, typeof WeakRef === 'function' ? new WeakRef(w) : { deref: () => w });
  swept?.register(w, h);
  return w;
}

/** Wraps a VALUE (`NativeBoxed`) as `Class`; every crossing is a new value. */
export function wrapValue(Class, box) {
  if (box === null || box === undefined) return null;
  return new Class(WRAP, box);
}

// ---- host calls -----------------------------------------------------------------------------------

function live(self) {
  const n = self[NODE];
  if (n === null || n === undefined) throw new DisposedError(self.constructor.native ?? 'object');
  const h = handleOf(n);
  if (h === null) throw new DisposedError(self.constructor.native ?? 'object');
  return h;
}

function valueHandle(self) {
  const b = self[BOX];
  const m = b?.__svlBoxed;
  if (m) return m[0];
  // A host without boxed values (the Node TestHost) answers a value constructor with a plain handle.
  const h = b && !('__svlBoxed' in b) ? handleOf(b) : null;
  if (h === null) throw new DisposedError(self.constructor.native ?? 'value');
  return h;
}

const handleFor = (self) => (BOX in self ? valueHandle(self) : live(self));

/** `host.call` on an object or value. */
export const call = (self, method, args) => host().call(handleFor(self), method, args);
/** `host.getProp` on an object: a snapshot, never reactive. */
export const getProp = (self, name) => host().getProp(live(self), name);
/**
 * `host.setProp` on an object. Readers of the property re-run in the current flush (a local
 * invalidation): the host drops the native echo of our own write (REACTIVITY.md section 5.2).
 */
export const setProp = (self, name, value) => {
  const h = live(self);
  host().setProp(h, name, value);
  tracker()?.invalidate(h, name);
};

// ---- reactivity (REACTIVITY.md section 7) ---------------------------------------------------------

const TRACKER = Symbol.for('svelters.types.tracker');
const tracker = () => globalThis[TRACKER];

/**
 * Installs the reactivity tracker generated getters use (`@svelters/native-runtime` installs one built
 * on Svelte's `createSubscriber`): `{ track(h, prop), local(h, prop), invalidate(h, prop), untrack(fn) }`.
 * `null` uninstalls it. Without a tracker every getter is a plain read.
 */
export function installTracker(t) {
  globalThis[TRACKER] = t ?? undefined;
}

/**
 * A reactive read of a property the platform announces changes of (`exact`/`partial`): inside an
 * effect it subscribes to native changes of (object, prop), then reads live. Outside an effect, or
 * inside `untrack`, it is `getProp`.
 */
export function observed(self, name) {
  const h = live(self);
  tracker()?.track(h, name);
  return host().getProp(h, name);
}

/**
 * A reactive read of a `set_only` property: only our own setters change it, so it re-runs on local
 * writes (no native observation), then reads live.
 */
export function observedLocal(self, name) {
  const h = live(self);
  tracker()?.local(h, name);
  return host().getProp(h, name);
}

/**
 * A reactive accessor with no native property behind it (Qt `QTextDocument.plainText` is `toPlainText()`):
 * inside an effect it re-runs when the object emits `signal` (or after `invalidate(self, prop)`), then
 * returns `read()`.
 */
export function observedSignal(self, prop, signal, read) {
  tracker()?.signal?.(self, prop, signal);
  return read();
}

/** Re-runs the readers of (object, prop) now: what every setter does after writing. */
export function invalidate(self, name) {
  const n = self?.[NODE];
  const h = n === null || n === undefined ? null : handleOf(n);
  if (h !== null) tracker()?.invalidate(h, name);
}

/** `fn()` with no reactive subscription (Svelte's `untrack` when a tracker is installed). */
export function untracked(fn) {
  const t = tracker();
  return t?.untrack ? t.untrack(fn) : fn();
}
/** `host.construct`: a new instance of the native type with construct-time properties. */
export const construct = (native, props) => host().construct(native, props);
/** `host.callStatic`: a static function or constructor function of the native type `native`. */
export const callStatic = (native, method, args) => host().callStatic(native, method, args);

// ---- async (N1) -----------------------------------------------------------------------------------

// wrapper -> number of its async calls in flight. While a call runs, the object it was made on stays
// reachable, so the garbage collector cannot release the handle (which would cancel the native
// operation and reject the Promise with `disposed`): `await File.newForPath(p).loadContentsAsync()` works.
const inflight = new Map();

function settle(self, p, shape) {
  if (self !== null) inflight.set(self, (inflight.get(self) ?? 0) + 1);
  const done = () => {
    if (self === null) return;
    const n = inflight.get(self) - 1;
    if (n > 0) inflight.set(self, n);
    else inflight.delete(self);
  };
  return Promise.resolve(p).then(
    (r) => {
      done();
      return shape ? shape(r) : r;
    },
    (e) => {
      done();
      throw e;
    },
  );
}

function withOptions(args, options) {
  if (options === undefined || options === null) return args;
  if (typeof options !== 'object') throw new TypeError('async options must be an object ({ signal })');
  return [...args, options];
}

/**
 * An async method (`_async` + `_finish` folded, `SVL_M_ASYNC`): resolves with the finish result shaped by
 * `shape`, rejects with a `NativeError`. `options.signal` (an `AbortSignal`) cancels the operation.
 */
export function callAsync(self, method, args, options, shape) {
  return settle(self, host().call(handleFor(self), method, withOptions(args, options)), shape);
}

/** An async static function or constructor. */
export function callStaticAsync(native, method, args, options, shape) {
  return settle(null, host().callStatic(native, method, withOptions(args, options)), shape);
}

// ---- streams (E3) ----------------------------------------------------------------------------------

const STREAM = Symbol.for('svelters.stream');
const END = Symbol('svelters.types.stream-end');
const weakly = (x) => (typeof WeakRef === 'function' ? new WeakRef(x) : { deref: () => x });
// A stream dropped unread is closed when collected, so its producer is not left blocked on a full buffer.
// The registry holds only `{ close }`: the host side (the op table) reaches a stream weakly.
const unread = typeof FinalizationRegistry === 'function' ? new FinalizationRegistry((held) => held.close?.()) : null;

/**
 * The JS side of a Rust stream (a `Sink<T>` parameter or a `Stream<T>` result of an app export): an async
 * iterable. Items arrive at host checkpoints; the producer may run ahead by at most its `buffer` of
 * items this iterator has not handed out yet. Leaving a `for await` early (or `close()`) closes the
 * stream, and the producer's next `send` fails. An error of the call rejects `next()` after the items
 * sent before it.
 */
export class NativeStream {
  #buf = [];
  #waiters = [];
  #end = null;
  #thrown = false;
  #closed = false;
  #ack = null;
  #close = null;
  #held = { close: null };
  #shape;

  constructor(shape) {
    this.#shape = typeof shape === 'function' ? shape : null;
    const ref = weakly(this);
    const held = this.#held;
    // What the host's op table talks to (the prelude binds the ack and close functions of the call).
    this[STREAM] = {
      bind(ack, close) {
        held.close = close;
        const s = ref.deref();
        if (s) {
          s.#ack = ack;
          s.#close = close;
        }
      },
      items(vs) {
        ref.deref()?.#push(vs);
      },
    };
    unread?.register(this, held);
  }

  #push(vs) {
    if (this.#closed || !Array.isArray(vs)) return;
    for (const raw of vs) this.#buf.push(this.#shape ? this.#shape(raw) : raw);
    this.#wake();
  }

  #wake() {
    const ws = this.#waiters;
    this.#waiters = [];
    for (const w of ws) w();
  }

  #wait() {
    if (this.#buf.length || this.#end) return Promise.resolve();
    return new Promise((r) => this.#waiters.push(r));
  }

  [END](error) {
    if (this.#end) return;
    this.#held.close = null;
    this.#end = error === undefined ? { done: true } : { error };
    this.#wake();
  }

  async next() {
    await this.#wait();
    if (this.#buf.length) {
      const value = this.#buf.shift();
      this.#ack?.(1);
      return { value, done: false };
    }
    if (this.#end && 'error' in this.#end && !this.#thrown) {
      this.#thrown = true;
      throw this.#end.error;
    }
    return { value: undefined, done: true };
  }

  /** Closes the stream (what leaving a `for await` early does). */
  async return() {
    this.close();
    return { value: undefined, done: true };
  }

  /** Closes the stream: the producer's next `send` fails; buffered items are dropped. */
  close() {
    if (this.#closed) return;
    this.#closed = true;
    this.#buf = [];
    if (!this.#end) {
      this.#close?.();
      this[END]();
    }
  }

  [Symbol.asyncIterator]() {
    return this;
  }

  /** The items as arrays, one per host checkpoint that delivered some (for bulk consumers). */
  async *batches() {
    try {
      for (;;) {
        await this.#wait();
        if (this.#buf.length) {
          const b = this.#buf;
          this.#buf = [];
          this.#ack?.(b.length);
          yield b;
          continue;
        }
        if (this.#end && 'error' in this.#end && !this.#thrown) {
          this.#thrown = true;
          throw this.#end.error;
        }
        return;
      }
    } finally {
      this.close();
    }
  }
}

function streamCall(self, start, args, options, shape) {
  if (options !== undefined && options !== null && typeof options !== 'object') throw new TypeError('stream options must be an object ({ signal })');
  const s = new NativeStream(shape);
  const opts = { ...(options ?? {}), [STREAM]: s[STREAM] };
  const ref = weakly(s);
  settle(self, start([...args, opts])).then(
    () => ref.deref()?.[END](),
    (e) => ref.deref()?.[END](e),
  );
  return s;
}

/** A stream method of an app object: returns a `NativeStream` at once; `options.signal` closes it. */
export function stream(self, method, args, options, shape) {
  return streamCall(self, (a) => host().call(handleFor(self), method, a), args, options, shape);
}

/** A stream function of an app module or a static stream method. */
export function streamStatic(native, method, args, options, shape) {
  return streamCall(null, (a) => host().callStatic(native, method, a), args, options, shape);
}

// ---- persist (E3) ----------------------------------------------------------------------------------

const classes = new Map();

/** Lets `persist` give a persisted object back as its class (generated app modules call it). */
export function registerClass(...cs) {
  for (const c of cs) if (c && typeof c.native === 'string') classes.set(c.native, c);
}

/**
 * The app object persisted under `key`: the one an earlier runtime made, which survives a hot reload
 * (`persist('sim', () => new Simulation(p))` is the same Rust object after every reload), else
 * `make()`'s result, persisted now. Objects not persisted die with their runtime. Without a host that
 * persists (the Node TestHost), this is `make()`.
 */
export function persist(key, make) {
  const h = globalThis.__svelters_host;
  if (!h || typeof h.persist !== 'function') return make();
  const got = h.persist(String(key));
  if (got) return wrapObject(classes.get(got[1]) ?? NativeObject, got[0]);
  const obj = make();
  h.persist(String(key), obj);
  return obj;
}

/** Forgets the object persisted under `key` (it lives on while anything else holds it). */
export function unpersist(key) {
  const h = globalThis.__svelters_host;
  if (h && typeof h.persist === 'function') h.persist(String(key), null);
}

/** A GType argument: a generated class (its native name) or a native type name. */
export function typeArg(x) {
  if (x === null || x === undefined) return null;
  if (typeof x === 'string') return x;
  if (typeof x === 'function' && typeof x.native === 'string') return x.native;
  throw new TypeError('expected a class of @svelters/types or a native type name');
}

/**
 * The prop table of a class maps a JS property name to `[nativeName, kind]` (`kind` is 'gtype' for
 * type-valued properties). `props` is the constructor argument.
 */
export function constructProps(Class, props, map) {
  const out = {};
  if (props === null || props === undefined) return out;
  if (typeof props !== 'object') throw new TypeError(`new ${Class.native}(props): props must be an object`);
  for (const key of Object.keys(props)) {
    const d = map[key] ?? map[key.replace(/-([a-z])/g, (_, c) => c.toUpperCase())];
    if (!d) throw new TypeError(`${Class.native} has no property '${key}'`);
    out[d[0]] = d[1] === 'gtype' ? typeArg(props[key]) : props[key];
  }
  return out;
}

/**
 * Constructor arguments that are a props object: none, or one plain object (not an array, a wrapper, a Date
 * or bytes). Qt classes with overloaded constructors take either that or the constructor's arguments.
 */
export function isProps(args) {
  if (args.length === 0) return true;
  if (args.length !== 1) return false;
  const x = args[0];
  if (x === null || typeof x !== 'object' || Array.isArray(x)) return false;
  const proto = Object.getPrototypeOf(x);
  return proto === Object.prototype || proto === null;
}

/**
 * `construct` with a props object whose keys are properties (`map`, as for `constructProps`) or setter-backed
 * keys (`setters`: key -> setter method, for Qt values that have a setter but no Q_PROPERTY, such as
 * QTextDocument `plainText`): the properties go to `construct`, the setters are called on the new object.
 */
export function constructObject(native, Class, props, map, setters) {
  const own = {};
  const later = [];
  if (props !== null && props !== undefined) {
    for (const key of Object.keys(props)) {
      const k = key.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      if (!map[key] && !map[k] && setters[k]) later.push([setters[k], props[key]]);
      else own[key] = props[key];
    }
  }
  const raw = construct(native, constructProps(Class, own, map));
  const h = handleOf(raw);
  for (const [setter, v] of later) host().call(h, setter, [v]);
  return raw;
}

// ---- NativeObject ---------------------------------------------------------------------------------

// handle -> { obj, signals: Map<signal, Set<fn>> }. The table holds the wrapper strongly while it has
// listeners: native code never holds JS values, so this is what keeps a listened object alive.
const listening = new Map();

function hookDispatch() {
  const g = globalThis;
  const desc = Object.getOwnPropertyDescriptor(g, '__svelters_dispatch');
  if (desc?.get?.[HOOKED]) return;
  // The runtime (re)installs its dispatcher on every mount, a list row's included: keep ours in front
  // of whatever it installs later, so listeners added before that mount keep receiving their signals.
  let prev = g.__svelters_dispatch;
  const dispatch = function (h, signal, args, ...rest) {
    const entry = listening.get(h);
    const fns = entry?.signals.get(signal);
    if (fns) {
      const shaped = shapeArgs(entry.obj.constructor, signal, args);
      for (const fn of [...fns]) {
        try {
          fn(...shaped);
        } catch (e) {
          console.error(e);
        }
      }
      return undefined;
    }
    return prev ? prev.call(this, h, signal, args, ...rest) : undefined;
  };
  const get = () => dispatch;
  get[HOOKED] = true;
  Object.defineProperty(g, '__svelters_dispatch', {
    configurable: true,
    enumerable: true,
    get,
    set(v) {
      if (v !== dispatch) prev = v;
    },
  });
}

function shapeArgs(Class, signal, args) {
  const spec = Class.signalArgs?.[signal];
  if (!spec || !Array.isArray(args)) return args ?? [];
  return args.map((a, i) => {
    const s = spec[i];
    if (!s) return a;
    const C = s[1]();
    return s[0] === 'o' ? wrapObject(C, a) : wrapValue(C, a);
  });
}

function hasGetter(obj, key) {
  for (let p = Object.getPrototypeOf(obj); p && p !== Object.prototype; p = Object.getPrototypeOf(p)) {
    const d = Object.getOwnPropertyDescriptor(p, key);
    if (d) return typeof d.get === 'function';
  }
  return false;
}

const kebabSignal = (s) => (s.includes('-') || !/[A-Z]/.test(s) ? s : s.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase()));

/** Base class of every generated object class. */
export class NativeObject {
  constructor(...args) {
    const a = args[0];
    if (a === WRAP) {
      this[NODE] = args[1];
      return;
    }
    const make = this.constructor[CONSTRUCT];
    if (typeof make !== 'function') {
      throw new TypeError(`${this.constructor.native ?? 'this class'} cannot be constructed with new`);
    }
    const raw = make.call(this.constructor, ...args);
    const h = handleOf(raw);
    if (h === null) throw new Error(`${this.constructor.native}: the host returned no object`);
    this[NODE] = raw;
    wrappers.set(h, typeof WeakRef === 'function' ? new WeakRef(this) : { deref: () => this });
    swept?.register(this, h);
  }

  /** What the host converts an object argument to; null once disposed. */
  get svlHandle() {
    const n = this[NODE];
    return n === null || n === undefined ? null : handleOf(n);
  }

  get disposed() {
    return this.svlHandle === null;
  }

  /** The native type name of the class this wrapper was made as. */
  get nativeType() {
    return this.constructor.native;
  }

  /**
   * Listens to a native signal (`'items-changed'`, or `itemsChanged`). Returns the disposer; the
   * object stays reachable while it has listeners.
   */
  on(signal, fn) {
    if (typeof fn !== 'function') throw new TypeError('on(signal, fn): fn must be a function');
    signal = kebabSignal(String(signal));
    const h = live(this);
    hookDispatch();
    let entry = listening.get(h);
    if (!entry) listening.set(h, (entry = { obj: this, signals: new Map() }));
    let fns = entry.signals.get(signal);
    if (!fns) {
      entry.signals.set(signal, (fns = new Set()));
      host().listen(h, signal, true);
    }
    fns.add(fn);
    return () => {
      const cur = listening.get(h);
      const set = cur?.signals.get(signal);
      if (!set || !set.delete(fn)) return;
      if (set.size === 0) {
        cur.signals.delete(signal);
        try {
          host().listen(h, signal, false);
        } catch {
          // the object is gone: nothing left to disconnect
        }
        if (cur.signals.size === 0) listening.delete(h);
      }
    };
  }

  /**
   * A snapshot of a property: the getter's value (`obj.peek('text')` is `obj.text`) without subscribing
   * the running effect to its changes. A name the class has no getter for (a native spelling such as
   * `cursor-position`) is read with `getProp`.
   */
  peek(prop) {
    const name = String(prop);
    const camel = name.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    const key = hasGetter(this, camel) ? camel : hasGetter(this, name) ? name : null;
    return untracked(() => (key === null ? getProp(this, name) : this[key]));
  }

  /** Rewraps this object as `Class` (an unchecked cast: a wrong class fails natively on first use). */
  static from(obj) {
    if (obj === null || obj === undefined) return null;
    if (!(obj instanceof NativeObject)) throw new TypeError('from(obj): expected a native object');
    if (!(obj instanceof this)) Object.setPrototypeOf(obj, this.prototype);
    return obj;
  }

  /** Releases our reference now. The native object lives on while anything else holds it. */
  dispose() {
    const n = this[NODE];
    if (n === null || n === undefined) return;
    const h = handleOf(n);
    this[NODE] = null;
    if (h === null) return;
    listening.delete(h);
    wrappers.delete(h);
    try {
      host().destroy(h);
    } catch {
      // already released by the host
    }
  }
}

if (typeof Symbol.dispose === 'symbol') {
  Object.defineProperty(NativeObject.prototype, Symbol.dispose, {
    value() {
      this.dispose();
    },
    configurable: true,
    writable: true,
  });
}

// ---- NativeValue ----------------------------------------------------------------------------------

/** Base class of every generated value class: an immutable native value behind a VALUE handle. */
export class NativeValue {
  constructor(...args) {
    const a = args[0];
    if (a === WRAP) {
      this[BOX] = args[1];
      return;
    }
    const make = this.constructor[CONSTRUCT];
    if (typeof make !== 'function') {
      throw new TypeError(`${this.constructor.native ?? 'this class'} cannot be constructed with new`);
    }
    this[BOX] = make.call(this.constructor, ...args);
  }

  /** What the host converts a value argument to; null once disposed. */
  get __svlBoxed() {
    return this[BOX]?.__svlBoxed ?? null;
  }

  /** A host without boxed values (the Node TestHost) holds the value as a plain handle: that handle. */
  get svlHandle() {
    const b = this[BOX];
    return b && !('__svlBoxed' in b) ? (handleOf(b) ?? undefined) : undefined;
  }

  get disposed() {
    return this.__svlBoxed === null && this.svlHandle === undefined;
  }

  get nativeType() {
    return this.constructor.native;
  }

  /** Native equality when the type has an `equal` function, else the same handle. */
  equals(other) {
    if (other === this) return true;
    if (!(other instanceof NativeValue)) return false;
    if (typeof this.equal === 'function') return this.equal(other);
    const a = this.__svlBoxed;
    const b = other.__svlBoxed;
    return a !== null && b !== null && a[0] === b[0];
  }

  dispose() {
    const b = this[BOX];
    this[BOX] = null;
    b?.dispose?.();
  }

  /** Rewraps this value as `Class`. */
  static from(v) {
    if (v === null || v === undefined) return null;
    if (!(v instanceof NativeValue)) throw new TypeError('from(value): expected a native value');
    if (!(v instanceof this)) Object.setPrototypeOf(v, this.prototype);
    return v;
  }
}

if (typeof Symbol.dispose === 'symbol') {
  Object.defineProperty(NativeValue.prototype, Symbol.dispose, {
    value() {
      this.dispose();
    },
    configurable: true,
    writable: true,
  });
}
