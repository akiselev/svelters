// Runes in a .svelte.js module: module-level state shared by every importer.
let count = $state(0);
export const history = $state([]);

export function increment(by = 1) {
  count += by;
  history.push(count);
}

export function current() {
  return count;
}

export class Doubler {
  #factor = $state(2);
  value = $derived(count * this.#factor);
  bump() {
    this.#factor++;
  }
}
