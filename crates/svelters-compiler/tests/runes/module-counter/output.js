import * as $ from 'svelte/internal/client';
let count = $.state(0);
export const doubled = () => $.get(count) * 2;
export const settings = $.proxy({ step: 1 });
const frozen = { big: true };

export function increment() {
  $.set(count, $.get(count) + (settings.step));
}

export function getCount() {
  return $.get(count);
}

export function reset() {
  $.set(count, 0);
  settings.step = 1;
  return frozen.big;
}

export default class Store {
  #n = $.state(0);
  get n() { return $.get(this.#n); }
  bump() { $.update(this.#n); }
}
