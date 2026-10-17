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
  Object.defineProperty(global, '__svelters_internal', { value: { fire, frame: runFrame }, enumerable: false });
})
