// `svelte/animate` for native elements (TR1; the build aliases `svelte/animate` here). Svelte
// 5.57.2's `flip` (svelte/src/animate/index.js, MIT, Copyright (c) 2016-2025 Svelte Contributors)
// with the computed style read natively (./motion.js `computed`): no transform, origin at the
// centre, no zoom. `from` / `to` come from the host's `bounds` (ABI v2.8).
import { computed } from './motion.js';

/** @param {number} t */
function cubicOut(t) {
  const f = t - 1.0;
  return f * f * f + 1.0;
}

/**
 * The FLIP technique: the element starts at `from` (translated and scaled) and moves to `to`, its
 * new layout rect.
 */
export function flip(node, { from, to }, params = {}) {
  const { delay = 0, duration = (d) => Math.sqrt(d) * 120, easing = cubicOut } = params;
  const style = computed(node);
  const transform = style.transform === 'none' ? '' : style.transform;
  // the transform origin as fractions of the size: the centre
  const ox = 0.5;
  const oy = 0.5;
  const sx = to.width ? to.width / to.width : 1;
  const sy = to.height ? to.height / to.height : 1;
  const fx = from.left + from.width * ox;
  const fy = from.top + from.height * oy;
  const tx = to.left + to.width * ox;
  const ty = to.top + to.height * oy;
  const dx = (fx - tx) * sx;
  const dy = (fy - ty) * sy;
  const dsx = to.width ? from.width / to.width : 1;
  const dsy = to.height ? from.height / to.height : 1;
  return {
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
}
