<!-- A class-based store: $state/$derived class fields (public and #private), a constructor
     state field, methods mutating proxied state. -->
<script>
  class Todos {
    items = $state([]);
    #nextId = $state(1);
    remaining = $derived(this.items.filter((t) => !t.done).length);

    constructor(title) {
      this.title = $state(title);
    }

    add(text) {
      this.items.push({ id: this.#nextId++, text, done: false });
    }

    toggleFirst() {
      const first = this.items[0];
      if (first) first.done = !first.done;
    }

    rename() {
      this.title += '!';
    }

    get summary() {
      return `${this.title}: ${this.remaining}/${this.items.length} left (next #${this.#nextId})`;
    }
  }

  const todos = new Todos('Todos');
</script>

<window title={todos.title}>
  <box orientation="vertical" spacing="4">
    <label>{todos.summary}</label>
    <button onclicked={() => todos.add(`task ${todos.items.length + 1}`)}>Add</button>
    <button onclicked={() => todos.toggleFirst()}>Toggle first</button>
    <button onclicked={() => todos.rename()}>Rename</button>
  </box>
</window>
