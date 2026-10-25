// `svelters:motion`: svelte/motion's API (`Spring`, `Tween`, `spring`, `tweened`,
// `prefersReducedMotion`) on a backend with the `native_motion` capability, where the compiler
// imports it in place of `svelte/motion` (plans/research/presto/compiler-and-js.md §3.6).
//
// `Spring` follows svelte/motion's spring in closed form (crates/presto-motion `shim::spring`, the
// same math): Svelte's per-frame integrator at 60 Hz is the linear recurrence
// `e[n+1] = (2 - k - c) e[n] - (1 - c) e[n-1]` on the error `e = x - target`, so `e[n]` is a sum
// of powers of its characteristic roots, evaluated here at `n = 60 * seconds`. Values equal
// Svelte's at every 60 Hz frame and stay correct at any other frame rate (Svelte's integrator
// changes with it). It settles where Svelte does. `preserveMomentum` ramps the inverse mass per
// frame, which has no closed form: such a `set` runs Svelte's integrator (`tick_spring`, svelte
// 5.57.2 src/motion/spring.js, MIT, Copyright (c) 2016-2025 Svelte Contributors).
//
// `Tween` and `tweened` are Svelte's own (they are frame-rate independent already); the legacy
// `spring` store is Svelte's too.
import { state, get, set, tag } from './core/index.js';
import { loop, raf } from './core/internal.js';
import { render_effect } from './core/svelte.js';
import { DEV } from 'esm-env';
import { prefersReducedMotion as reduced } from './motion.js';

export { Tween, tweened, spring_store as spring } from './core/svelte.js';

/** `prefersReducedMotion.current`, as svelte/motion's MediaQuery gives it. */
export const prefersReducedMotion = {
  get current() {
    return reduced();
  },
};

const is_date = (v) => Object.prototype.toString.call(v) === '[object Date]';

/**
 * One spring motion of a number from `x0` (the frame before: `xPrev`) to `target`, in frames:
 * `{ raw(n), settled(n) }`.
 */
export function springMotion({ stiffness: k, damping: c, precision }, x0, xPrev, target) {
  const e0 = x0 - target;
  const em1 = xPrev - target;
  const sum = 2 - k - c;
  const prod = 1 - c;
  const disc = sum * sum - 4 * prod;
  let error;
  if (prod <= 0) {
    const r = Math.max(1 - k, 0);
    error = (n) => (n <= 0 ? e0 : e0 * Math.pow(r, n));
  } else if (Math.abs(disc) < 1e-14) {
    const r = sum / 2;
    const b = e0 - r * em1;
    error = (n) => (e0 + b * n) * Math.pow(r, n);
  } else if (disc < 0) {
    const rho = Math.sqrt(prod);
    const lnRho = Math.log(rho);
    const theta = Math.atan2(Math.sqrt(-disc), sum);
    const d = (e0 * Math.cos(theta) - rho * em1) / Math.sin(theta);
    error = (n) => Math.exp(lnRho * n) * (e0 * Math.cos(n * theta) + d * Math.sin(n * theta));
  } else {
    const s = Math.sqrt(disc);
    const r1 = (sum + s) / 2;
    const r2 = (sum - s) / 2;
    const a = (em1 - e0 / r2) / (1 / r1 - 1 / r2);
    const b = e0 - a;
    error = (n) => a * Math.pow(r1, n) + b * Math.pow(r2, n);
  }
  // the first frame whose error and next step are both under `precision` (Svelte writes the target
  // one frame later)
  let settleFrame = Infinity;
  let e = error(0);
  for (let m = 0; m < 100000; m++) {
    const next = error(m + 1);
    if (Math.abs(next - e) < precision && Math.abs(e) < precision) {
      settleFrame = m;
      break;
    }
    e = next;
  }
  // the frame count comes from a clock in ms: allow its rounding (n = 41.9999... is frame 42)
  const settled = (n) => n >= settleFrame + 1 - 1e-6;
  return { target, raw: (n) => target + error(n), settled, value: (n) => (settled(n) ? target : target + error(n)) };
}

/** A number leaf of a spring's value: its motion, and whether the value is a Date. */
class Leaf {
  constructor(motion, date) {
    this.motion = motion;
    this.date = date;
  }
}

const num = (v) => (is_date(v) ? v.getTime() : v);

/** The motions of each number in `cur` (arrays, plain objects and Dates, as Svelte's tick_spring recurses). */
function build(params, cur, prev, target) {
  if (typeof cur === 'number' || is_date(cur)) return new Leaf(springMotion(params, num(cur), num(prev ?? cur), num(target)), is_date(cur));
  if (Array.isArray(cur)) return cur.map((v, i) => build(params, v, prev?.[i], target?.[i]));
  if (cur !== null && typeof cur === 'object') {
    const out = {};
    for (const k in cur) out[k] = build(params, cur[k], prev?.[k], target?.[k]);
    return out;
  }
  throw new Error(`Cannot spring ${typeof cur} values`);
}

/** The value shaped like the motions, each number `fn(motion)`. */
function evaluate(tree, fn) {
  if (tree instanceof Leaf) {
    const v = fn(tree.motion);
    return tree.date ? new Date(v) : v;
  }
  if (Array.isArray(tree)) return tree.map((t) => evaluate(t, fn));
  const out = {};
  for (const k in tree) out[k] = evaluate(tree[k], fn);
  return out;
}

/** Svelte's `tick_spring` (one frame of its integrator, for `preserveMomentum`). */
function tick_spring(ctx, last_value, current_value, target_value) {
  if (typeof current_value === 'number' || is_date(current_value)) {
    const delta = target_value - current_value;
    const velocity = (current_value - last_value) / (ctx.dt || 1 / 60);
    const spring = ctx.opts.stiffness * delta;
    const damper = ctx.opts.damping * velocity;
    const acceleration = (spring - damper) * ctx.inv_mass;
    const d = (velocity + acceleration) * ctx.dt;
    if (Math.abs(d) < ctx.opts.precision && Math.abs(delta) < ctx.opts.precision) {
      return target_value;
    }
    ctx.settled = false;
    return is_date(current_value) ? new Date(current_value.getTime() + d) : current_value + d;
  } else if (Array.isArray(current_value)) {
    return current_value.map((_, i) => tick_spring(ctx, last_value[i], current_value[i], target_value[i]));
  } else if (typeof current_value === 'object') {
    const next_value = {};
    for (const k in current_value) {
      next_value[k] = tick_spring(ctx, last_value[k], current_value[k], target_value[k]);
    }
    return next_value;
  }
  throw new Error(`Cannot spring ${typeof current_value} values`);
}

const clamp = (n, min, max) => Math.max(min, Math.min(max, n));

/** svelte/motion's `Spring`, in closed form (see the top of this file). */
export class Spring {
  #stiffness = state(0.15);
  #damping = state(0.8);
  #precision = state(0.01);
  #current;
  #target;
  /** The leaves' motions (same shape as the value), the start time, and the frame before. */
  #motions = null;
  #start = 0;
  #last = undefined;
  #task = null;
  #deferred = null;
  /** preserveMomentum: Svelte's integrator state. */
  #inverse_mass = 1;
  #momentum = 0;

  constructor(value, options = {}) {
    this.#current = state(value);
    this.#target = state(value);
    if (typeof options.stiffness === 'number') this.#stiffness.v = clamp(options.stiffness, 0, 1);
    if (typeof options.damping === 'number') this.#damping.v = clamp(options.damping, 0, 1);
    if (typeof options.precision === 'number') this.#precision.v = options.precision;
    if (DEV) {
      tag(this.#stiffness, 'Spring.stiffness');
      tag(this.#damping, 'Spring.damping');
      tag(this.#precision, 'Spring.precision');
      tag(this.#current, 'Spring.current');
      tag(this.#target, 'Spring.target');
    }
  }

  static of(fn, options) {
    const spring = new Spring(fn(), options);
    render_effect(() => {
      spring.set(fn());
    });
    return spring;
  }

  #params() {
    return { stiffness: this.#stiffness.v, damping: this.#damping.v, precision: this.#precision.v };
  }

  /** Frames since the motions started. */
  #frame(now) {
    return ((now - this.#start) * 60) / 1000;
  }

  /** Starts closed-form motions from where the value is now (and was a frame ago) toward `target`. */
  #retarget(target) {
    const now = raf.now();
    const params = this.#params();
    let cur = this.#current.v;
    let prev = this.#last ?? cur;
    if (this.#motions) {
      const n = this.#frame(now);
      cur = evaluate(this.#motions, (m) => m.value(n));
      prev = evaluate(this.#motions, (m) => (m.settled(n) ? m.value(n) : m.raw(n - 1)));
    }
    this.#motions = build(params, cur, prev, target);
    this.#start = now;
  }

  #update(value) {
    set(this.#target, value);
    this.#current.v ??= value;
    if (this.#momentum > 0) return this.#integrate();
    this.#retarget(value);
    if (!this.#task) {
      this.#task = loop((now) => {
        const n = this.#frame(now);
        let done = true;
        const next = evaluate(this.#motions, (m) => {
          if (!m.settled(n)) done = false;
          return m.value(n);
        });
        this.#last = this.#current.v;
        set(this.#current, next);
        if (done) {
          this.#task = null;
          this.#motions = null;
        }
        return !done;
      });
    }
    return this.#task.promise;
  }

  /** Svelte's integrator, for a `set` with `preserveMomentum` (the inverse mass ramps up per frame). */
  #integrate() {
    this.#task?.abort();
    this.#motions = null;
    this.#last ??= this.#current.v;
    let last_time = raf.now();
    const recovery = 1000 / (this.#momentum * 60);
    this.#task = loop((now) => {
      this.#inverse_mass = Math.min(this.#inverse_mass + recovery, 1);
      const elapsed = Math.min(now - last_time, 1000 / 30);
      const ctx = { inv_mass: this.#inverse_mass, opts: this.#params(), settled: true, dt: (elapsed * 60) / 1000 };
      const next = tick_spring(ctx, this.#last, this.#current.v, this.#target.v);
      this.#last = this.#current.v;
      last_time = now;
      set(this.#current, next);
      if (ctx.settled) {
        this.#task = null;
        this.#momentum = 0;
      }
      return !ctx.settled;
    });
    return this.#task.promise;
  }

  set(value, options) {
    this.#deferred?.reject(new Error('Aborted'));
    if (options?.instant || this.#current.v === undefined) {
      this.#task?.abort();
      this.#task = null;
      this.#motions = null;
      set(this.#current, set(this.#target, value));
      this.#last = value;
      return Promise.resolve();
    }
    if (options?.preserveMomentum) {
      this.#inverse_mass = 0;
      this.#momentum = options.preserveMomentum;
    } else if (this.#momentum > 0) {
      // a plain set after a momentum one: continue in closed form from the integrator's state
      this.#task?.abort();
      this.#task = null;
      this.#momentum = 0;
    }
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    const d = (this.#deferred = { promise, resolve, reject });
    promise.catch(() => {});
    this.#update(value).then(() => {
      if (d !== this.#deferred) return;
      d.resolve(undefined);
    });
    return promise;
  }

  get current() {
    return get(this.#current);
  }

  get damping() {
    return get(this.#damping);
  }

  set damping(v) {
    set(this.#damping, clamp(v, 0, 1));
  }

  get precision() {
    return get(this.#precision);
  }

  set precision(v) {
    set(this.#precision, v);
  }

  get stiffness() {
    return get(this.#stiffness);
  }

  set stiffness(v) {
    set(this.#stiffness, clamp(v, 0, 1));
  }

  get target() {
    return get(this.#target);
  }

  set target(v) {
    this.set(v);
  }
}
