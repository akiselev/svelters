let count = $state(0);
export const doubled = () => count * 2;
export const settings = $state({ step: 1 });
const frozen = $state.raw({ big: true });

export function increment() {
  count += settings.step;
}

export function getCount() {
  return count;
}

export function reset() {
  count = 0;
  settings.step = 1;
  return frozen.big;
}

export default class Store {
  #n = $state(0);
  get n() { return this.#n; }
  bump() { this.#n++; }
}
