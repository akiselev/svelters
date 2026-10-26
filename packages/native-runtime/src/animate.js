// `svelte/animate` for native elements (TR1; the build aliases `svelte/animate` here). Svelte
// 5.57.2's `flip` (svelte/src/animate/index.js, MIT, Copyright (c) 2016-2025 Svelte Contributors)
// with the computed style read natively (./motion.js `computed`): the layout size is the
// computed width/height (Svelte's `clientWidth`/`clientHeight`), no zoom. `from` / `to` come from
// the host's `bounds` (ABI v2.8).
//
// MOTION-CSS: `flipNative` is `flip` with its native descriptor (./motion.js `describeNative`): a
// backend that evaluates it (skelter) plays the transform track from the rects and layout size.
import { computed, describeNative, easingName } from './motion.js';

/** @param {number} t */
function cubicOut(t) {
  const f = t - 1.0;
  return f * f * f + 1.0;
}

/** `transform-origin` in px (`50px 20px`; keywords and % of the layout size). */
function originPx(text, w, h) {
  const KEY = { left: 0, top: 0, center: 0.5, right: 1, bottom: 1 };
  const one = (p, size) => (p in KEY ? KEY[p] * size : p.endsWith('%') ? (parseFloat(p) / 100) * size : parseFloat(p));
  const parts = String(text ?? '').trim().split(/\s+/);
  const x = one(parts[0] ?? 'center', w);
  const y = one(parts[1] ?? 'center', h);
  return [Number.isFinite(x) ? x : w / 2, Number.isFinite(y) ? y : h / 2];
}

/** What `flip` computes before its config, plus what the native descriptor needs. */
function measure(node, from, to, params) {
  const { delay = 0, duration = (d) => Math.sqrt(d) * 120, easing = cubicOut } = params;
  const style = computed(node);
  const transform = style.transform === 'none' ? '' : style.transform;
  // the layout size without transforms (Svelte's clientWidth / clientHeight)
  const cw = parseFloat(style.width);
  const ch = parseFloat(style.height);
  const clientWidth = Number.isFinite(cw) && cw > 0 ? cw : to.width;
  const clientHeight = Number.isFinite(ch) && ch > 0 ? ch : to.height;
  const [opx, opy] = originPx(style.transformOrigin, clientWidth, clientHeight);
  const ox = clientWidth ? opx / clientWidth : 0.5;
  const oy = clientHeight ? opy / clientHeight : 0.5;
  // the parent transforms between the layout size and the measured rect
  const sx = to.width ? clientWidth / to.width : 1;
  const sy = to.height ? clientHeight / to.height : 1;
  const fx = from.left + from.width * ox;
  const fy = from.top + from.height * oy;
  const tx = to.left + to.width * ox;
  const ty = to.top + to.height * oy;
  const dx = (fx - tx) * sx;
  const dy = (fy - ty) * sy;
  const dsx = to.width ? from.width / to.width : 1;
  const dsy = to.height ? from.height / to.height : 1;
  const cfg = {
    delay,
    duration: typeof duration === 'function' ? duration(Math.sqrt(dx * dx + dy * dy)) : duration,
    easing,
    css: (t, u) => {
      const x = u * dx;
      const y = u * dy;
      const sx = t + u * dsx;
      const sy = t + u * dsy;
      return `transform: ${transform} translate(${x}px, ${y}px) scale(${sx}, ${sy});`;
    },
  };
  return { cfg, style, clientWidth, clientHeight, origin: [opx, opy] };
}

/**
 * The FLIP technique: the element starts at `from` (translated and scaled) and moves to `to`, its
 * new layout rect.
 */
export function flip(node, { from, to }, params = {}) {
  return measure(node, from, to, params).cfg;
}

/** `flip` with its native descriptor (`$n.flip_native`). A custom easing keeps it a plain `flip`. */
export function flipNative(node, { from, to }, params = {}) {
  const m = measure(node, from, to, params);
  const easing = easingName(m.cfg.easing);
  if (easing === null) return m.cfg;
  const rect = (r) => ({ left: r.left, top: r.top, width: r.width, height: r.height });
  return describeNative(node, m.cfg, {
    kind: 'flip',
    params: { easing },
    computed: { transform: String(m.style.transform ?? 'none') },
    flip: { from: rect(from), to: rect(to), clientWidth: m.clientWidth, clientHeight: m.clientHeight, origin: m.origin, zoom: 1 },
  });
}
