(function (native, global) {
  'use strict';

  const fmt = (v) => {
    if (typeof v === 'string') return v;
    if (v instanceof Error) return v.stack ? String(v.stack) : String(v);
    if ((typeof v === 'object' && v !== null) || typeof v === 'function') {
      try {
        const j = JSON.stringify(v);
        if (j !== undefined) return j;
      } catch (_) {}
    }
    return String(v);
  };
  const logger = (level) => (...args) => native.log(level, args.map(fmt).join(' '));
  global.console = {
    log: logger('log'),
    info: logger('info'),
    debug: logger('debug'),
    warn: logger('warn'),
    error: logger('error'),
    trace: logger('trace'),
  };

  if (typeof global.performance === 'undefined') {
    global.performance = { now: () => native.now() };
  }

  if (typeof global.queueMicrotask !== 'function') {
    global.queueMicrotask = (cb) => {
      if (typeof cb !== 'function') throw new TypeError('queueMicrotask: callback must be a function');
      Promise.resolve().then(cb);
    };
  }

  const timers = new Map();
  let nextId = 1;
  const add = (cb, delay, args, repeat) => {
    if (typeof cb !== 'function') throw new TypeError('timer callback must be a function');
    let d = Number(delay);
    if (!(d >= 0) || d > 2147483647) d = 0;
    d = Math.floor(d);
    const id = nextId++;
    timers.set(id, { cb, args, repeat });
    native.schedule(id, d, repeat);
    return id;
  };
  const clear = (id) => {
    if (timers.delete(id)) native.cancel(id);
  };
  global.setTimeout = (cb, delay, ...args) => add(cb, delay, args, false);
  global.setInterval = (cb, delay, ...args) => add(cb, delay, args, true);
  global.clearTimeout = clear;
  global.clearInterval = clear;
  // Animation frames run on the toolkit's frame clock: the first request asks the host for a
  // frame and every callback requested before it fires runs in that frame, in request order,
  // with the frame's performance.now() timestamp. Scripted hosts have no frame clock
  // (`frame_request` answers false) and use a virtual 16 ms timer.
  const frames = new Map();
  let frameId = 1;
  let frameArmed = false;
  const runFrame = () => {
    frameArmed = false;
    const batch = [...frames.values()];
    frames.clear();
    const ts = native.now();
    let failed = false;
    let first;
    for (const cb of batch) {
      try {
        cb(ts);
      } catch (e) {
        if (failed) global.console.error(e);
        else {
          failed = true;
          first = e;
        }
      }
    }
    if (failed) throw first;
  };
  global.requestAnimationFrame = (cb) => {
    if (typeof cb !== 'function') throw new TypeError('requestAnimationFrame: callback must be a function');
    const id = frameId++;
    frames.set(id, cb);
    if (!frameArmed) {
      frameArmed = true;
      if (!native.frame_request()) add(runFrame, 16, [], false);
    }
    return id;
  };
  global.cancelAnimationFrame = (id) => {
    frames.delete(id);
  };

  const fire = (id) => {
    const t = timers.get(id);
    if (!t) return;
    if (!t.repeat) timers.delete(id);
    t.cb(...t.args);
  };
  // ---- AbortController / AbortSignal (QuickJS has neither) ----

  if (typeof global.AbortController === 'undefined') {
    class AbortSignal {
      constructor() {
        this.aborted = false;
        this.reason = undefined;
        this._listeners = [];
      }
      addEventListener(type, fn, opts) {
        if (type === 'abort') this._listeners.push({ fn, once: !!(opts && opts.once) });
      }
      removeEventListener(type, fn) {
        if (type === 'abort') this._listeners = this._listeners.filter((l) => l.fn !== fn);
      }
      throwIfAborted() {
        if (this.aborted) throw this.reason;
      }
    }
    class AbortController {
      constructor() {
        this.signal = new AbortSignal();
      }
      abort(reason) {
        const s = this.signal;
        if (s.aborted) return;
        s.aborted = true;
        if (reason === undefined) {
          reason = new Error('This operation was aborted');
          reason.name = 'AbortError';
        }
        s.reason = reason;
        const ls = s._listeners;
        s._listeners = ls.filter((l) => !l.once);
        for (const l of ls) l.fn.call(s, { type: 'abort', target: s });
      }
    }
    Object.defineProperty(global, 'AbortController', { value: AbortController, writable: true, configurable: true });
    Object.defineProperty(global, 'AbortSignal', { value: AbortSignal, writable: true, configurable: true });
  }

  // ---- native error and boxed-value wrappers (ABI v3 value model) ----

  class NativeError extends Error {
    constructor(domain, code, message) {
      super(message);
      this.name = 'NativeError';
      this.domain = domain;
      this.code = code;
    }
  }
  Object.defineProperty(global, 'NativeError', { value: NativeError, writable: true, configurable: true });

  // A wrapper owns one backend handle. `dispose()` releases it at once and is idempotent. If the
  // wrapper is collected without being disposed, the registry callback (a job, not the GC
  // finalizer itself) only queues the handle; the host releases the queue at its next checkpoint.
  const registry =
    typeof FinalizationRegistry === 'function' ? new FinalizationRegistry((h) => native.queueRelease(h)) : null;
  class NativeBoxed {
    #h;
    #t;
    #name;
    constructor(h, t, name) {
      this.#h = h;
      this.#t = t;
      this.#name = name;
      // No unregister token: QuickJS holds a token strongly, which would keep the wrapper alive.
      // A handle released by dispose() is simply ignored when its finalization callback fires.
      if (registry) registry.register(this, h);
    }
    get disposed() {
      return this.#h === 0;
    }
    get typeName() {
      return this.#name;
    }
    get __svlBoxed() {
      return this.#h === 0 ? null : [this.#h, this.#t];
    }
    dispose() {
      if (this.#h === 0) return;
      const h = this.#h;
      this.#h = 0;
      native.releaseBoxed(h);
    }
  }
  if (typeof Symbol.dispose === 'symbol') {
    Object.defineProperty(NativeBoxed.prototype, Symbol.dispose, {
      value() {
        this.dispose();
      },
      configurable: true,
      writable: true,
    });
  }
  // A value with an exact GVariant type: `variant('(ii)', [1, 2])`, `variant('a{ss}', {a: 'b'})`.
  // Without it a variant's type is inferred from the JS value (integers `x`, floats `d`, ...).
  class NativeVariant {
    #sig;
    #value;
    constructor(signature, value) {
      if (typeof signature !== 'string' || signature === '') {
        throw new TypeError('variant(signature, value): the signature must be a non-empty GVariant type string');
      }
      this.#sig = signature;
      this.#value = value;
    }
    get signature() {
      return this.#sig;
    }
    get value() {
      return this.#value;
    }
    get __svlVariant() {
      return [this.#sig, this.#value];
    }
  }
  Object.defineProperty(global, 'NativeVariant', { value: NativeVariant, writable: true, configurable: true });
  Object.defineProperty(global, 'variant', {
    value: (signature, value) => new NativeVariant(signature, value),
    writable: true,
    configurable: true,
  });
  const makeBoxed = (h, t, name) => new NativeBoxed(h, t, name);
  const makeNativeError = (domain, code, message) => new NativeError(domain, code, message);
  // An async native method returns a Promise; the backend completes it through a one-shot callback.
  const makeCompletion = () => {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    // Settle one microtask later: a backend that fails to start, or an already aborted signal,
    // completes inside the call, before the caller could attach `.catch`, and the rejection tracker
    // would report it as unhandled.
    const settle = (v) => (v instanceof NativeError ? reject(v) : resolve(v));
    return [promise, (v) => void Promise.resolve().then(() => settle(v))];
  };
  const watchAbort = (signal, id) => {
    signal.addEventListener('abort', () => native.cancelAsync(id), { once: true });
  };
  // E1: offloaded export calls. The host settles them in batches at checkpoint start (`settleOps`:
  // kind 0 resolve, 1 reject, 2 progress); an abort of `options.signal` rejects at once.
  const ops = new Map();
  const makeOp = (id, options) => {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    const onProgress = options && typeof options.onProgress === 'function' ? options.onProgress : null;
    const op = { resolve, reject, onProgress, unwatch: null };
    ops.set(id, op);
    const signal = options ? options.signal : null;
    if (signal && typeof signal.addEventListener === 'function' && !signal.aborted) {
      const onAbort = () => {
        if (ops.get(id) === op) {
          ops.delete(id);
          reject(new NativeError('svelters', 'cancelled', 'The operation was aborted'));
        }
        native.cancelOp(id);
      };
      signal.addEventListener('abort', onAbort, { once: true });
      op.unwatch = () => signal.removeEventListener?.('abort', onAbort);
    }
    return promise;
  };
  const finishOp = (id) => {
    const op = ops.get(id);
    if (op) {
      ops.delete(id);
      if (op.unwatch) op.unwatch();
    }
    return op;
  };
  const progressOp = (op, v) => {
    if (!op || !op.onProgress) return;
    try {
      op.onProgress(v);
    } catch (e) {
      queueMicrotask(() => {
        throw e;
      });
    }
  };
  const settleOps = (batch) => {
    for (let i = 0; i < batch.length; i += 3) {
      const kind = batch[i];
      const id = batch[i + 1];
      const v = batch[i + 2];
      if (kind === 2) {
        progressOp(ops.get(id), v);
        continue;
      }
      const op = finishOp(id);
      if (op) (kind === 0 ? op.resolve : op.reject)(v);
    }
  };
  const settleOpSoon = (kind, id, v) => {
    if (kind === 2) {
      progressOp(ops.get(id), v);
      return;
    }
    const op = finishOp(id);
    if (op) Promise.resolve().then(() => (kind === 0 ? op.resolve : op.reject)(v));
  };
  const opsLive = () => ops.size;
  // E2: typed arrays crossing to Rust exports. The intrinsics are captured before user code runs.
  // `transfers` keeps the JS buffers whose memory Rust holds (moved with `transfer()`): nothing else
  // can reach them until Rust hands them back (`transferOut`) or drops them (`transferDrop`).
  const apply = Reflect.apply;
  const getter = (o, k) => {
    const d = o ? Object.getOwnPropertyDescriptor(o, k) : undefined;
    return d ? d.get : undefined;
  };
  const taProto = Object.getPrototypeOf(Uint8Array.prototype);
  const taBuffer = getter(taProto, 'buffer');
  const taOffset = getter(taProto, 'byteOffset');
  const taLength = getter(taProto, 'length');
  const abDetached = getter(ArrayBuffer.prototype, 'detached');
  const abResizable = getter(ArrayBuffer.prototype, 'resizable');
  const sabLength = typeof SharedArrayBuffer === 'function' ? getter(SharedArrayBuffer.prototype, 'byteLength') : undefined;
  const abTransfer = ArrayBuffer.prototype.transfer;
  const isSab = (b) => {
    if (!sabLength) return false;
    try {
      apply(sabLength, b, []);
      return true;
    } catch (_) {
      return false;
    }
  };
  // [buffer, byteOffset, length, state]: 0 usable, 1 detached, 3 SharedArrayBuffer, 4 resizable.
  const viewInfo = (v) => {
    const b = apply(taBuffer, v, []);
    if (isSab(b)) return [b, 0, 0, 3];
    if (abDetached && apply(abDetached, b, [])) return [b, 0, 0, 1];
    const state = abResizable && apply(abResizable, b, []) ? 4 : 0;
    return [b, apply(taOffset, v, []), apply(taLength, v, []), state];
  };
  const transfers = new Map();
  const transferIn = (b, id) => {
    const t = apply(abTransfer, b, []);
    transfers.set(id, t);
    return t;
  };
  const transferOut = (id) => {
    const t = transfers.get(id);
    if (t === undefined) return null;
    transfers.delete(id);
    return t;
  };
  const transferDrop = (ids) => {
    let n = 0;
    for (const id of ids) if (transfers.delete(id)) n++;
    return n;
  };
  const transfersLive = () => transfers.size;
  const isView = (v) => ArrayBuffer.isView(v);
  const dateMs = (v) => (v instanceof Date ? v.getTime() : null);
  const makeDate = (ms) => new Date(ms);

  Object.defineProperty(global, '__svelters_internal', {
    value: { fire, frame: runFrame, makeBoxed, makeNativeError, isView, dateMs, makeDate, makeCompletion, watchAbort, makeOp, settleOps, settleOpSoon, opsLive, viewInfo, transferIn, transferOut, transferDrop, transfersLive },
    enumerable: false,
  });
})
