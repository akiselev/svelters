// Svelte's tests/animation-helpers.js for the native runtime (TR1): a virtual clock for Svelte's
// `raf` (the object the native animation player and Svelte's `loop` schedule on). `raf.tick(ms)`
// sets the time, flushes, and runs the frame callbacks. Svelte's helper also mocks
// `HTMLElement.prototype.animate`; the native player is the runtime node's own `animate`
// (src/motion.js), so no mock is needed: its styles reach the DOM through the host's style sink.
import { flushSync } from 'svelte';
import { raf as svelte_raf } from 'svelte/internal/client';

export const raf = {
  /** @type {Set<(n: number) => void>} */
  ticks: new Set(),
  tick,
  time: 0,
  reset() {
    this.time = 0;
    this.ticks.clear();
    svelte_raf.tick = (f) => {
      raf.ticks.add(f);
    };
    svelte_raf.now = () => raf.time;
    svelte_raf.tasks.clear();
  },
};

/** @param {number} time */
function tick(time) {
  raf.time = time;
  flushSync();
  for (const t of raf.ticks) t(raf.time);
}
