// `svelte/transition` for native elements (TR1; the build aliases `svelte/transition` here, as it
// aliases `svelte` to ./svelte.js). Svelte 5.57.2's built-ins (svelte/src/transition/index.js,
// MIT, Copyright (c) 2016-2025 Svelte Contributors) with the same signatures, defaults and CSS
// text; only what they read from the DOM changed: `getComputedStyle(node)` is the node's native
// computed visuals (./motion.js `computed`: inline opacity, no transform, size from the host's
// `bounds`), `getBoundingClientRect()` is the runtime node's (ABI v2.8 `bounds`), and `draw`'s
// `getTotalLength()` is the node's own when it has one (0 otherwise).
//
// MOTION-CSS: `nativeTransition(kind)` is the built-in with its parameters, easing name and the
// computed style it read attached for a backend that evaluates the formulas natively
// (./motion.js `describeNative`; the compiler emits `$n.transition_native` for these on a backend
// with the `native_transitions` capability).
import { computed, describeNative, easingName, nameEasing } from './motion.js';

/** @param {number} x */
const linear = (x) => x;

/** @param {number} t */
function cubic_out(t) {
  const f = t - 1.0;
  return f * f * f + 1.0;
}

/** @param {number} t */
function cubic_in_out(t) {
  return t < 0.5 ? 4.0 * t * t * t : 0.5 * Math.pow(2.0 * t - 2.0, 3.0) + 1.0;
}

nameEasing(linear, 'linear');
nameEasing(cubic_out, 'cubicOut');
nameEasing(cubic_in_out, 'cubicInOut');

/** The node and the computed style the last built-in read (for its native descriptor). */
let lastRead = null;

/** `getComputedStyle(node)`, remembered for `nativeTransition`. */
function read(node) {
  const style = computed(node);
  lastRead = { node, style };
  return style;
}

/** @param {number | string} value @returns {[number, string]} */
function split_css_unit(value) {
  const split = typeof value === 'string' && value.match(/^\s*(-?[\d.]+)([^\s]*)\s*$/);
  return split ? [parseFloat(split[1]), split[2] || 'px'] : [/** @type {number} */ (value), 'px'];
}

/** @param {string} property @param {number} value @param {number} t */
function css_dimension(property, value, t) {
  return Number.isNaN(value) ? '' : `${property}: ${t * value}px;`;
}

/** Svelte's `blur`. Natively the blur channel is not animated yet: the opacity part plays. */
export function blur(node, { delay = 0, duration = 400, easing = cubic_in_out, amount = 5, opacity = 0 } = {}) {
  const style = read(node);
  const target_opacity = +style.opacity;
  const f = style.filter === 'none' ? '' : style.filter;
  const od = target_opacity * (1 - opacity);
  const [value, unit] = split_css_unit(amount);
  return {
    delay,
    duration,
    easing,
    css: (_t, u) => `opacity: ${target_opacity - od * u}; filter: ${f} blur(${u * value}${unit});`,
  };
}

/** Svelte's `fade`: the opacity channel (every backend). */
export function fade(node, { delay = 0, duration = 400, easing = linear } = {}) {
  const o = +read(node).opacity;
  return {
    delay,
    duration,
    easing,
    css: (t) => `opacity: ${t * o}`,
  };
}

/** Svelte's `fly`: the transform and opacity channels. */
export function fly(node, { delay = 0, duration = 400, easing = cubic_out, x = 0, y = 0, opacity = 0 } = {}) {
  const style = read(node);
  const target_opacity = +style.opacity;
  const transform = style.transform === 'none' ? '' : style.transform;
  const od = target_opacity * (1 - opacity);
  const [x_value, x_unit] = split_css_unit(x);
  const [y_value, y_unit] = split_css_unit(y);
  return {
    delay,
    duration,
    easing,
    css: (t, u) => `
			transform: ${transform} translate(${(1 - t) * x_value}${x_unit}, ${(1 - t) * y_value}${y_unit});
			opacity: ${target_opacity - od * u}`,
  };
}

/**
 * Svelte's `slide`: the clip channel (height or width with `overflow: hidden`) and opacity. The
 * size is the node's natural size from the host's `bounds`; without geometry only the opacity
 * part plays.
 */
export function slide(node, { delay = 0, duration = 400, easing = cubic_out, axis = 'y' } = {}) {
  const style = read(node);
  const opacity = +style.opacity;
  const primary_property = axis === 'y' ? 'height' : 'width';
  const primary_property_value = parseFloat(style[primary_property]);
  const secondary_properties = axis === 'y' ? ['top', 'bottom'] : ['left', 'right'];
  const capitalized_secondary_properties = secondary_properties.map((e) => `${e[0].toUpperCase()}${e.slice(1)}`);
  const padding_start_value = parseFloat(style[`padding${capitalized_secondary_properties[0]}`]);
  const padding_end_value = parseFloat(style[`padding${capitalized_secondary_properties[1]}`]);
  const margin_start_value = parseFloat(style[`margin${capitalized_secondary_properties[0]}`]);
  const margin_end_value = parseFloat(style[`margin${capitalized_secondary_properties[1]}`]);
  const border_width_start_value = parseFloat(style[`border${capitalized_secondary_properties[0]}Width`]);
  const border_width_end_value = parseFloat(style[`border${capitalized_secondary_properties[1]}Width`]);
  return {
    delay,
    duration,
    easing,
    css: (t) =>
      'overflow: hidden;' +
      `opacity: ${Math.min(t * 20, 1) * opacity};` +
      css_dimension(primary_property, primary_property_value, t) +
      css_dimension(`padding-${secondary_properties[0]}`, padding_start_value, t) +
      css_dimension(`padding-${secondary_properties[1]}`, padding_end_value, t) +
      css_dimension(`margin-${secondary_properties[0]}`, margin_start_value, t) +
      css_dimension(`margin-${secondary_properties[1]}`, margin_end_value, t) +
      css_dimension(`border-${secondary_properties[0]}-width`, border_width_start_value, t) +
      css_dimension(`border-${secondary_properties[1]}-width`, border_width_end_value, t) +
      `min-${primary_property}: 0`,
  };
}

/** Svelte's `scale`: the transform and opacity channels. */
export function scale(node, { delay = 0, duration = 400, easing = cubic_out, start = 0, opacity = 0 } = {}) {
  const style = read(node);
  const target_opacity = +style.opacity;
  const transform = style.transform === 'none' ? '' : style.transform;
  const sd = 1 - start;
  const od = target_opacity * (1 - opacity);
  return {
    delay,
    duration,
    easing,
    css: (_t, u) => `
			transform: ${transform} scale(${1 - sd * u});
			opacity: ${target_opacity - od * u}
		`,
  };
}

function assign(tar, src) {
  for (const k in src) tar[k] = src[k];
  return tar;
}

/**
 * Svelte's `crossfade`: a deferred pair of transitions; the element that is received flies from
 * where the sent one was (`getBoundingClientRect`: the host's `bounds`).
 */
/**
 * Svelte's `draw`: the stroke dash channels. The path length is the node's `getTotalLength()`
 * when the backend has one (presto's path kinds), else 0.
 */
export function draw(node, { delay = 0, speed, duration, easing = cubic_in_out } = {}) {
  let len = typeof node.getTotalLength === 'function' ? node.getTotalLength() : 0;
  const style = read(node);
  if (style.strokeLinecap !== 'butt') {
    len += parseInt(style.strokeWidth);
  }
  if (duration === undefined) {
    if (speed === undefined) {
      duration = 800;
    } else {
      duration = len / speed;
    }
  } else if (typeof duration === 'function') {
    duration = duration(len);
  }
  return {
    delay,
    duration,
    easing,
    css: (_, u) => `
			stroke-dasharray: ${len};
			stroke-dashoffset: ${u * len};
		`,
  };
}

export function crossfade({ fallback, ...defaults }) {
  const to_receive = new Map();
  const to_send = new Map();

  function crossfade(from_node, node, params) {
    const { delay = 0, duration = (d) => Math.sqrt(d) * 30, easing = cubic_out } = assign(assign({}, defaults), params);
    const from = from_node.getBoundingClientRect();
    const to = node.getBoundingClientRect();
    const dx = from.left - to.left;
    const dy = from.top - to.top;
    const dw = to.width ? from.width / to.width : 1;
    const dh = to.height ? from.height / to.height : 1;
    const d = Math.sqrt(dx * dx + dy * dy);
    const style = computed(node);
    const transform = style.transform === 'none' ? '' : style.transform;
    const opacity = +style.opacity;
    return {
      delay,
      duration: typeof duration === 'function' ? duration(d) : duration,
      easing,
      css: (t, u) => `
			   opacity: ${t * opacity};
			   transform-origin: top left;
			   transform: ${transform} translate(${u * dx}px,${u * dy}px) scale(${t + (1 - t) * dw}, ${t + (1 - t) * dh});
		   `,
    };
  }

  function transition(items, counterparts, intro) {
    return (node, params) => {
      items.set(params.key, node);
      return () => {
        if (counterparts.has(params.key)) {
          const other_node = counterparts.get(params.key);
          counterparts.delete(params.key);
          return crossfade(other_node, node, params);
        }
        // if the node is disappearing altogether (i.e. wasn't claimed by the other list) then we
        // need to supply an outro
        items.delete(params.key);
        return fallback && fallback(node, params, intro);
      };
    };
  }

  return [transition(to_send, to_receive, false), transition(to_receive, to_send, true)];
}

// ---------------------------------------------------------------------------------------------
// MOTION-CSS: the built-ins with their native descriptors

const BUILTINS = { fade, fly, slide, scale, blur, draw };

/** The JSON-able parameters the native formulas read (functions resolve in JS). */
const PARAM_KEYS = ['delay', 'duration', 'x', 'y', 'opacity', 'start', 'amount', 'axis', 'speed'];

/** What the built-ins read from the computed style. */
const STYLE_KEYS = [
  'opacity',
  'transform',
  'filter',
  'width',
  'height',
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
  'strokeLinecap',
  'strokeWidth',
];

const natives = new Map();

/**
 * The built-in `kind` (`fade`, `fly`, `slide`, `scale`, `blur`, `draw`) as a transition function
 * whose config carries its native descriptor. A custom easing keeps it a plain built-in.
 */
export function nativeTransition(kind) {
  let fn = natives.get(kind);
  if (fn) return fn;
  const builtin = BUILTINS[kind];
  if (!builtin) throw new Error(`[svelters] no native transition '${kind}' (svelte/transition's fade, fly, slide, scale, blur, draw)`);
  fn = (node, params) => {
    lastRead = null;
    const cfg = builtin(node, params);
    const easing = easingName(cfg.easing);
    if (easing === null || lastRead?.node !== node) return cfg;
    const p = { easing };
    for (const k of PARAM_KEYS) {
      const v = params?.[k];
      if (typeof v === 'number' || typeof v === 'string') p[k] = v;
    }
    const style = {};
    for (const k of STYLE_KEYS) {
      const v = lastRead.style[k];
      if (v != null) style[k] = String(v);
    }
    if (kind === 'draw' && typeof node.getTotalLength === 'function') style.totalLength = node.getTotalLength();
    return describeNative(node, cfg, { kind, params: p, computed: style });
  };
  natives.set(kind, fn);
  return fn;
}
