// TR1: transitions and animations on native elements (plans/milestones/TR1-transitions.md).
//
// Svelte's own `$.transition` / `$.animation` and `pause_effect` / `resume_effect` do the work (the
// core adapter, ./core/svelte.js). They talk to an `Element`; the runtime node provides the slice
// of it they use (defineFacade): `animate()` (a WAAPI-style player, NativeAnimation), `inert`,
// `style`, `dispatchEvent`, `getBoundingClientRect`, `getAnimations`.
//
// The player maps each frame's CSS declarations onto four visual channels, delivered through the
// per-element inline style sink (`host.style`, ABI v2.3 ops + v2.8 additions):
//   opacity   -> SVL_STYLE_OPACITY   (4)  "0".."1"
//   transform -> SVL_STYLE_TRANSFORM (8)  "tx ty sx sy ox oy" (px, px, factors, origin as fractions)
//   clip      -> SVL_STYLE_CLIP      (9)  "w h" px, -1 = that axis unclipped
//   inert     -> SVL_STYLE_INERT     (10)  "1"
// Declarations without a channel are dropped (one dev warning per property). The frame clock is
// Svelte's `raf` (requestAnimationFrame on the host's frame clock): a frame is requested only while
// an animation runs.
import { DEV } from 'esm-env';
import { loop, queue_task, raf } from './core/internal.js';

export const OP_OPACITY = 4;
export const OP_TRANSFORM = 8;
export const OP_CLIP = 9;
export const OP_INERT = 10;

const CHANNELS = [
  ['opacity', OP_OPACITY],
  ['transform', OP_TRANSFORM],
  ['clip', OP_CLIP],
];

// ---------------------------------------------------------------------------------------------
// Host sink

/** host -> Set of style ops the host refused (a backend without the channel). */
const refused = new WeakMap();
const warned = new Set();

export function warnOnce(key, msg) {
  if (warned.has(key)) return;
  warned.add(key);
  if (DEV) console.warn(`[svelters/motion] ${msg}`);
}

/** Sends one channel op; a host or backend without it drops the channel (warned once, in dev). */
function send(node, op, text) {
  const hs = node.host;
  const no = refused.get(hs);
  if (no?.has(op)) return;
  if (typeof hs.style !== 'function') {
    refused.set(hs, new Set([OP_OPACITY, OP_TRANSFORM, OP_CLIP, OP_INERT]));
    warnOnce('style', 'this host has no style sink: transitions are not shown (their timing and events still run)');
    return;
  }
  try {
    hs.style(op, node.h, text);
  } catch (e) {
    (no ?? refused.set(hs, new Set()).get(hs)).add(op);
    const name = op === OP_OPACITY ? 'opacity' : op === OP_TRANSFORM ? 'transform' : op === OP_CLIP ? 'clip' : 'inert';
    warnOnce(`op${op}`, `the backend does not animate ${name}: ${e?.message ?? e} (the channel is dropped)`);
  }
}

// ---------------------------------------------------------------------------------------------
// CSS declarations -> channel values

const round = (n) => Math.round(n * 1e6) / 1e6;

/** px value of a CSS length (`12px`, `0`, `12`); NaN for anything else (`%`, `em`). */
function px(v) {
  const m = /^\s*(-?[\d.]+(?:e-?\d+)?)(px)?\s*$/i.exec(String(v));
  return m ? parseFloat(m[1]) : NaN;
}

const ORIGIN = { left: 0, top: 0, center: 0.5, right: 1, bottom: 1 };

/** `transform-origin` as fractions of the size ([ox, oy]); px components are not supported. */
function origin(v) {
  const parts = String(v).trim().split(/\s+/);
  let ox = 0.5;
  let oy = 0.5;
  const frac = (p) => (p in ORIGIN ? ORIGIN[p] : p.endsWith('%') ? parseFloat(p) / 100 : NaN);
  if (parts.length === 1) {
    const p = parts[0];
    if (p === 'top' || p === 'bottom') oy = ORIGIN[p];
    else ox = frac(p);
  } else {
    // keywords may come in either order (`top left`)
    let [a, b] = parts;
    if (a === 'top' || a === 'bottom' || b === 'left' || b === 'right') [a, b] = [b, a];
    ox = frac(a);
    oy = frac(b);
  }
  if (!Number.isFinite(ox) || !Number.isFinite(oy)) {
    warnOnce('origin', `transform-origin '${v}' is not supported natively (keywords and % only): using center`);
    return [0.5, 0.5];
  }
  return [ox, oy];
}

/**
 * A CSS `transform` list as translate + scale ([tx, ty, sx, sy]), composed left to right as CSS
 * does (`translate(a) scale(s)`: tx = a; `scale(s) translate(a)`: tx = s * a). null for `none` or
 * empty; unsupported functions (rotate, skew, matrix, % translations) are dropped with a warning.
 */
export function parseTransform(text) {
  let tx = 0;
  let ty = 0;
  let sx = 1;
  let sy = 1;
  let any = false;
  const s = String(text).trim();
  if (s === '' || s === 'none') return null;
  for (const m of s.matchAll(/([a-zA-Z0-9]+)\(([^)]*)\)/g)) {
    const fn = m[1];
    const args = m[2].split(/\s*,\s*|\s+/).filter(Boolean);
    if (fn === 'translate' || fn === 'translateX' || fn === 'translateY' || fn === 'translate3d') {
      let a = fn === 'translateY' ? 0 : px(args[0]);
      let b = fn === 'translateX' ? 0 : fn === 'translateY' ? px(args[0]) : args.length > 1 ? px(args[1]) : 0;
      if (!Number.isFinite(a) || !Number.isFinite(b)) {
        warnOnce(`transform:${fn}%`, `transform ${fn}(${m[2]}): only px translations are animated natively`);
        a = Number.isFinite(a) ? a : 0;
        b = Number.isFinite(b) ? b : 0;
      }
      tx += sx * a;
      ty += sy * b;
      any = true;
    } else if (fn === 'scale' || fn === 'scaleX' || fn === 'scaleY' || fn === 'scale3d') {
      const a = parseFloat(args[0]);
      const c = fn === 'scaleX' ? 1 : fn === 'scaleY' ? a : args.length > 1 ? parseFloat(args[1]) : a;
      sx *= fn === 'scaleY' ? 1 : a;
      sy *= c;
      any = true;
    } else {
      warnOnce(`transform:${fn}`, `transform ${fn}() is not animated natively (translate and scale are)`);
    }
  }
  return any ? [tx, ty, sx, sy] : null;
}

// Declarations a native element has no channel for, but that are harmless to ignore: what the
// built-ins (`slide`) and Svelte itself emit next to the ones that matter.
const IGNORED = new Set([
  'overflow',
  'minHeight',
  'minWidth',
  'paddingTop',
  'paddingBottom',
  'paddingLeft',
  'paddingRight',
  'marginTop',
  'marginBottom',
  'marginLeft',
  'marginRight',
  'borderTopWidth',
  'borderBottomWidth',
  'borderLeftWidth',
  'borderRightWidth',
  'transformOrigin',
  'position',
]);

const camel = (p) => (p.startsWith('--') ? p : p.replace(/-([a-z])/g, (_, c) => c.toUpperCase()));

/**
 * One keyframe (Svelte's `css_to_keyframe` object, camelCase keys, or a `{prop: value}` map) as
 * channel values: { opacity?: number, transform?: [tx, ty, sx, sy, ox, oy], clip?: [w, h] }.
 */
export function parseFrame(frame) {
  const out = {};
  let origin_ = null;
  let overflow = null;
  let w;
  let h;
  for (const [k0, raw] of Object.entries(frame)) {
    const k = camel(k0);
    const v = raw == null ? '' : String(raw).trim();
    if (k === 'opacity') {
      const n = parseFloat(v);
      if (v !== '' && Number.isFinite(n)) out.opacity = Math.max(0, Math.min(1, v.endsWith('%') ? n / 100 : n));
    } else if (k === 'transform') {
      const t = parseTransform(v);
      out.transform = t ?? [0, 0, 1, 1];
    } else if (k === 'transformOrigin') {
      origin_ = origin(v);
    } else if (k === 'overflow') {
      overflow = v;
    } else if (k === 'height') {
      h = px(v);
    } else if (k === 'width') {
      w = px(v);
    } else if (k === 'filter') {
      if (v && v !== 'none') warnOnce('filter', `filter (${v}) is not animated natively yet: only the other declarations play`);
    } else if (!IGNORED.has(k)) {
      warnOnce(`prop:${k}`, `'${k0}' has no native animation channel and is dropped (animated natively: opacity, transform, height/width with overflow: hidden)`);
    }
  }
  if (out.transform) out.transform = [...out.transform, ...(origin_ ?? [0.5, 0.5])];
  if ((w !== undefined || h !== undefined) && overflow === 'hidden') {
    out.clip = [Number.isFinite(w) ? Math.max(0, w) : -1, Number.isFinite(h) ? Math.max(0, h) : -1];
  } else if (w !== undefined || h !== undefined) {
    warnOnce('size', 'height/width are animated natively only together with overflow: hidden (as a clip)');
  }
  return out;
}

const lerp = (a, b, p) => a + (b - a) * p;

function mix(a, b, p) {
  if (p === 0 || b === undefined) return a;
  if (a === undefined) return p < 0.5 ? a : b;
  if (typeof a === 'number') return lerp(a, b, p);
  return a.map((x, i) => (x < 0 || b[i] < 0 ? (p < 0.5 ? x : b[i]) : lerp(x, b[i], p)));
}

const fmt = (ch, v) => (ch === 'opacity' ? String(round(v)) : v.map(round).join(' '));

// ---------------------------------------------------------------------------------------------
// Per-node composition: the base (inline style), `node.style` writes, then running animations
// (the newest wins per channel), written only when the result changes

function vis(node) {
  return (node.vis ??= { sent: {}, styled: {}, anims: [] });
}

/** Recomputes and sends the node's channels. */
export function commit(node) {
  const v = node.vis;
  if (!v || !node.alive()) return;
  for (const [ch, op] of CHANNELS) {
    let value;
    for (let i = v.anims.length - 1; i >= 0 && value === undefined; i--) value = v.anims[i].cur?.[ch];
    if (value === undefined) value = v.styled[ch];
    let text = value === undefined ? null : fmt(ch, value);
    // no animated value: back to the element's own (inline) opacity, which the style runtime sent
    if (text === null && ch === 'opacity' && node.sc?.opacity) text = node.sc.opacity;
    const before = v.sent[ch] ?? null;
    if (text === before) continue;
    v.sent[ch] = text;
    // nothing was ever animated on this channel and nothing is: no op
    send(node, op, text);
  }
}

/** Whether an animation or a `node.style` write currently owns the opacity channel (style.js defers to it). */
export function animatesOpacity(node) {
  const v = node.vis;
  return !!v && (v.styled.opacity !== undefined || v.anims.some((a) => a.cur?.opacity !== undefined));
}

// ---------------------------------------------------------------------------------------------
// The player: the slice of WAAPI's Animation that Svelte's transitions.js uses

let ids = 0;

export class NativeAnimation {
  /**
   * `keyframes`: Svelte's keyframe objects (evenly spaced); `options`: `{ duration, delay }` or a
   * number. Applies the first frame at once (fill), then one interpolated frame per `raf` tick.
   */
  constructor(node, keyframes, options) {
    this.id = ++ids;
    this.target = node;
    this.frames = (Array.isArray(keyframes) ? keyframes : []).map(parseFrame);
    let duration = 0;
    let delay = 0;
    if (typeof options === 'number') duration = options;
    else if (options) {
      duration = typeof options.duration === 'number' ? options.duration : 0;
      delay = options.delay ?? 0;
    }
    this.duration = Math.max(0, Math.round(+duration || 0));
    this.delay = +delay || 0;
    this.start = raf.now();
    this.playState = 'running';
    this.cur = null;
    this.effect = {};
    this._onfinish = null;
    this._finished = false;
    this.task = null;
    vis(node).anims.push(this);
    this._apply(0);
    if (this.duration > 0) {
      this.task = loop((now) => this._tick(now));
    }
  }

  /** ms since the start, without the delay, clamped to the duration (read live by Svelte's `tick` loop). */
  get currentTime() {
    if (this.playState === 'idle') return null;
    return Math.max(0, Math.min(this.duration, raf.now() - this.start - this.delay));
  }

  set onfinish(fn) {
    this._onfinish = fn;
    // a zero-length animation (Svelte's delay placeholder with delay 0) finishes in Svelte's task
    // queue, before the next frame (as the test helper's WAAPI mock does)
    if (this.duration === 0 && !this._finished) {
      this._finished = true;
      queue_task(() => {
        if (this.playState === 'idle') return;
        this.playState = 'finished';
        this._onfinish?.();
      });
    }
  }

  get onfinish() {
    return this._onfinish;
  }

  _tick(now) {
    if (this.playState !== 'running') return false;
    const t = now - this.start - this.delay;
    if (t < 0) return true;
    if (t >= this.duration) {
      this._apply(1);
      this.playState = 'finished';
      this._finished = true;
      this._onfinish?.();
      return false;
    }
    this._apply(t / this.duration);
    return true;
  }

  /** Applies the keyframes at progress `p` (0..1), interpolating the two adjacent ones. */
  _apply(p) {
    const f = this.frames;
    if (f.length === 0) return;
    const n = Math.min(1, Math.max(0, p)) * (f.length - 1);
    const lo = f[Math.floor(n)];
    const hi = f[Math.ceil(n)];
    const q = n % 1;
    const cur = {};
    for (const [ch] of CHANNELS) {
      const v = mix(lo[ch], hi[ch], q);
      if (v !== undefined) cur[ch] = v;
    }
    this.cur = cur;
    commit(this.target);
  }

  cancel() {
    if (this.playState === 'idle') return;
    this.playState = 'idle';
    this.task?.abort();
    this.task = null;
    const v = this.target.vis;
    const i = v ? v.anims.indexOf(this) : -1;
    if (i >= 0) v.anims.splice(i, 1);
    commit(this.target);
  }

  finish() {
    if (this.playState !== 'running') return;
    this._tick(this.start + this.delay + this.duration);
  }
}

// ---------------------------------------------------------------------------------------------
// `node.style`: a CSSStyleDeclaration over the channels (Svelte reads/writes `overflow`; a `tick`
// written for the DOM writes `opacity` / `transform`)

class NativeStyle {
  #node;
  #decl = new Map();
  constructor(node) {
    this.#node = node;
  }

  getPropertyValue(name) {
    return this.#decl.get(camel(name)) ?? '';
  }

  setProperty(name, value) {
    const k = camel(name);
    const v = value == null ? '' : String(value);
    if (v === '') this.#decl.delete(k);
    else this.#decl.set(k, v);
    this.#sync();
  }

  removeProperty(name) {
    const k = camel(name);
    const old = this.#decl.get(k) ?? '';
    this.#decl.delete(k);
    this.#sync();
    return old;
  }

  get cssText() {
    return [...this.#decl].map(([k, v]) => `${k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}: ${v};`).join(' ');
  }

  set cssText(text) {
    this.#decl.clear();
    for (const part of String(text ?? '').split(';')) {
      const i = part.indexOf(':');
      if (i > 0) this.#decl.set(camel(part.slice(0, i).trim()), part.slice(i + 1).trim());
    }
    this.#sync();
  }

  #sync() {
    const v = vis(this.#node);
    v.styled = parseFrame(Object.fromEntries(this.#decl));
    commit(this.#node);
  }
}

// property accessors (`style.opacity = t`, `style.overflow`) for the common names
for (const k of ['opacity', 'transform', 'transformOrigin', 'overflow', 'height', 'width', 'filter', 'position', 'minHeight', 'minWidth']) {
  Object.defineProperty(NativeStyle.prototype, k, {
    get() {
      return this.getPropertyValue(k);
    },
    set(v) {
      this.setProperty(k, v);
    },
    enumerable: true,
  });
}

// ---------------------------------------------------------------------------------------------
// Geometry (ABI v2.8 `bounds`)

/**
 * The node's layout rect in its toplevel's coordinates as a DOMRect-like object, or null when the
 * host has no geometry. Not laid out yet: x = y = 0 and the natural size.
 */
export function bounds(node) {
  const hs = node.host;
  if (typeof hs.bounds !== 'function' || !node.alive()) return null;
  let r;
  try {
    r = hs.bounds(node.h);
  } catch (e) {
    warnOnce('bounds', `bounds: ${e?.message ?? e}`);
    return null;
  }
  if (!r) return null;
  const [x, y, width, height] = r;
  return { x, y, left: x, top: y, width, height, right: x + width, bottom: y + height };
}

const NO_RECT = Object.freeze({ x: 0, y: 0, left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0 });

/**
 * What Svelte's built-in transitions read from `getComputedStyle(node)`, natively: the inline
 * opacity (else 1), no transform, the size from `bounds`, zero padding/margin/border.
 */
export function computed(node) {
  // a host that knows the element's computed style (the DOM conformance host: the browser's
  // getComputedStyle) answers it itself
  if (typeof node.host.computedStyle === 'function' && node.alive()) {
    const cs = node.host.computedStyle(node.h);
    if (cs) return cs;
  }
  const b = bounds(node);
  const op = node.sc?.opacity;
  return {
    opacity: op ? op : '1',
    transform: 'none',
    filter: 'none',
    display: 'block',
    transformOrigin: b ? `${b.width / 2}px ${b.height / 2}px` : '50% 50%',
    height: b ? `${b.height}px` : 'auto',
    width: b ? `${b.width}px` : 'auto',
    paddingTop: '0px',
    paddingBottom: '0px',
    paddingLeft: '0px',
    paddingRight: '0px',
    marginTop: '0px',
    marginBottom: '0px',
    marginLeft: '0px',
    marginRight: '0px',
    borderTopWidth: '0px',
    borderBottomWidth: '0px',
    borderLeftWidth: '0px',
    borderRightWidth: '0px',
  };
}

// ---------------------------------------------------------------------------------------------
// The facade on the runtime node

if (typeof globalThis.CustomEvent !== 'function') {
  // QuickJS has none; Svelte's transitions dispatch `new CustomEvent('introstart')`
  globalThis.CustomEvent = class CustomEvent {
    constructor(type, init) {
      this.type = type;
      this.detail = init?.detail ?? null;
    }
  };
}

/** Events the runtime dispatches itself (never `host.listen`ed). */
export const RUNTIME_EVENTS = new Set(['introstart', 'introend', 'outrostart', 'outroend']);

/**
 * Adds the `Element` slice Svelte's transition code uses to the runtime node class. `handlers(node,
 * type)` gives the node's handlers for a runtime event.
 */
export function defineFacade(proto, { handlers }) {
  Object.defineProperties(proto, {
    inert: {
      get() {
        return !!this.inertFlag;
      },
      set(v) {
        v = !!v;
        if (v === !!this.inertFlag) return;
        this.inertFlag = v;
        if (this.alive()) send(this, OP_INERT, v ? '1' : null);
      },
    },
    style: {
      get() {
        return (this.styleDecl ??= new NativeStyle(this));
      },
    },
  });
  proto.animate = function (keyframes, options) {
    return new NativeAnimation(this, keyframes, options);
  };
  proto.getAnimations = function () {
    return [...(this.vis?.anims ?? [])];
  };
  proto.dispatchEvent = function (event) {
    for (const fn of handlers(this, event.type)) fn.call(this, event);
    return true;
  };
  proto.getBoundingClientRect = function () {
    return bounds(this) ?? NO_RECT;
  };
}
