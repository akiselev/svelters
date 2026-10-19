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
/** `host.getProp` on an object. */
export const getProp = (self, name) => host().getProp(live(self), name);
/** `host.setProp` on an object. */
export const setProp = (self, name, value) => host().setProp(live(self), name, value);
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
