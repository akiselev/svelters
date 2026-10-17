<!-- TypeScript: `<script lang="ts">` with an interface, a type alias, typed props, typed $state /
     $derived, a generic function, `as` / `satisfies` / `!`, typed event handlers in the template and
     a typed child component. The compiler removes the types (Svelte's remove_typescript_nodes) and
     strips their text from the emitted JavaScript. -->
<script lang="ts">
  import Item from './Item.svelte';
  import type { Snippet } from 'svelte';

  interface Todo {
    id: number;
    label: string;
    done: boolean;
  }
  type Filter = 'all' | 'open';

  let { title = 'Typed' }: { title?: string; extra?: Snippet } = $props();

  let todos: Todo[] = $state([
    { id: 1, label: 'Milk', done: false },
    { id: 2, label: 'Eggs', done: true },
  ]);
  let filter = $state<Filter>('all');
  let nextId: number = 3;
  let shown = $derived(todos.filter((t: Todo): boolean => filter === 'all' || !t.done));
  let count = $derived(shown.length as number);

  function add<T extends string>(label: T): void {
    todos.push({ id: nextId++, label, done: false } satisfies Todo);
  }

  const toggle = (id: number): void => {
    const todo = todos.find((t) => t.id === id)!;
    todo.done = !todo.done;
  };
</script>

<window title={title}>
  <box orientation="vertical" spacing="4">
    <label>{count} shown ({filter as string})</label>
    {#each shown as todo (todo.id)}
      <Item label={todo.label} done={todo.done} ontoggle={() => toggle(todo.id)} />
    {/each}
    <button onclick={() => add<string>('Bread')}>Add</button>
    <button onclick={(e?: unknown) => (filter = filter === 'all' ? ('open' as Filter) : 'all')}>Filter</button>
  </box>
</window>
