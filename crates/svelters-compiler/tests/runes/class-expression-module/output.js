import * as $ from 'svelte/internal/client';
export const Box = class {
  #value = $.state(0); get value() { return $.get(this.#value); } set value(value) { $.set(this.#value, value, true); }
  constructor(v) {
    this.value = v;
  }
  get twice() {
    return this.value * 2;
  }
};

export function createTodo(text) {
  return new (class Todo {
    #text = $.state($.proxy(text)); get text() { return $.get(this.#text); } set text(value) { $.set(this.#text, value, true); }
    #done = $.state(false); get done() { return $.get(this.#done); } set done(value) { $.set(this.#done, value, true); }
    toggle = () => (this.done = !this.done);
  })();
}
