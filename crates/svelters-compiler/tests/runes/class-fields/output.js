import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function input($$anchor, $$props) {
	$.push($$props, true);
	class Counter { #total; get total() { return $.get(this.#total); } set total(value) { $.set(this.#total, value, true); }
    #count = $.state(0); get count() { return $.get(this.#count); } set count(value) { $.set(this.#count, value, true); }
    #step = $.state(1);
    #items = $.state([]); get items() { return $.get(this.#items); } set items(value) { $.set(this.#items, value); }
    #double = $.derived(() => (this.count * 2)); get double() { return $.get(this.#double); } set double(value) { $.set(this.#double, value); }
    #label = $.derived(() => `n=${this.count}`); get label() { return $.get(this.#label); } set label(value) { $.set(this.#label, value); }
    #odd_key = $.state($.proxy({ a: 1 })); get 'odd-key'() { return $.get(this.#odd_key); } set 'odd-key'(value) { $.set(this.#odd_key, value, true); }

    constructor(step) {
      $.set(this.#step, step, true);
      this.#total = $.state(this.#step.v * 10);
      this.#hidden = $.state($.proxy(step));
      console.log(this.#step.v, this.#hidden.v);
    }

    #hidden;

    increment() {
      this.count += $.get(this.#step);
      $.update(this.#step);
      $.update_pre(this.#hidden, -1);
      $.get(this.#step) ?? $.set(this.#step, 2);
      $.set(this.#step, { nested: true }, true);
      return $.get(this.#hidden) + this.total;
    }
  }
	const counter = new Counter(2);
	const $$n1 = $n.element("label");
	$.template_effect(() => $n.set($$n1, "label", `${counter.count} / ${counter.double}`));
	$n.append($$anchor, $$n1);
	$.pop();
}
