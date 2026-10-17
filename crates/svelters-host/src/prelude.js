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
  // Animation-frame stub: a ~60 Hz timer. Real frame clocks come with the backends.
  global.requestAnimationFrame = (cb) => add(() => cb(native.now()), 16, [], false);
  global.cancelAnimationFrame = clear;

  const fire = (id) => {
    const t = timers.get(id);
    if (!t) return;
    if (!t.repeat) timers.delete(id);
    t.cb(...t.args);
  };
  Object.defineProperty(global, '__svelters_internal', { value: { fire }, enumerable: false });
})
