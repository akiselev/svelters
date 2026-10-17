<script>
  class Counter {
    count = $state(0);
    #step = $state(1);
    items = $state.raw([]);
    double = $derived(this.count * 2);
    label = $derived.by(() => `n=${this.count}`)
    'odd-key' = $state({ a: 1 });

    constructor(step) {
      this.#step = step;
      this.total = $state(this.#step * 10);
      this.#hidden = $state(step);
      console.log(this.#step, this.#hidden);
    }

    #hidden;

    increment() {
      this.count += this.#step;
      this.#step++;
      --this.#hidden;
      this.#step ??= 2;
      this.#step = { nested: true };
      return this.#hidden + this.total;
    }
  }

  const counter = new Counter(2);
</script>

<label label={`${counter.count} / ${counter.double}`} />
