<script>
  import { ListView } from '@svelters/lists';

  // Driven by the toolkit tests (crates/svelters-gtk/tests/lists_e2e.rs and the Qt twin): they click
  // the buttons by label and watch the native model, the row slots and the status label.
  // Big lists use $state.raw: no per-row proxies, updates replace the array.
  let items = $state.raw([]);
  let selected = $state(-1);
  let activated = $state('-');
  let list = $state(null);
  let next = 0;
  let show = $state(true);

  const make = (n) => Array.from({ length: n }, () => ({ id: next, text: `Item ${next++}` }));
  const fill = (n) => {
    next = 0;
    items = make(n);
  };
  const append = () => (items = [...items, ...make(1)]);
  const prepend = () => (items = [...make(1), ...items]);
  const removeFirst = () => (items = items.slice(1));
  const removeMiddle = () => (items = items.filter((_, i) => i !== 5 && i !== 6 && i !== 7));
  const reverse = () => (items = items.toReversed());
  const rename = () => (items = items.map((it) => (it.id % 10 === 0 ? { ...it, text: `${it.text}!` } : it)));
  const sortDesc = () => (items = items.toSorted((a, b) => b.id - a.id));
  const odd = () => (items = items.filter((it) => it.id % 2 === 1));
  const clear = () => (items = []);
  const moveFirstToEnd = () => (items = [...items.slice(1), items[0]]);
</script>

<window title="Lists stress">
  <box orientation="vertical" spacing="4">
    <box orientation="horizontal" spacing="4">
      <button onclick={() => fill(1000)}>fill1k</button>
      <button onclick={() => fill(10000)}>fill10k</button>
      <button onclick={() => fill(100000)}>fill100k</button>
      <button onclick={append}>append</button>
      <button onclick={prepend}>prepend</button>
      <button onclick={removeFirst}>removeFirst</button>
      <button onclick={removeMiddle}>removeMiddle</button>
    </box>
    <box orientation="horizontal" spacing="4">
      <button onclick={reverse}>reverse</button>
      <button onclick={rename}>rename</button>
      <button onclick={sortDesc}>sortDesc</button>
      <button onclick={odd}>odd</button>
      <button onclick={clear}>clear</button>
      <button onclick={moveFirstToEnd}>moveFirst</button>
      <button onclick={() => list?.scrollTo(items.length - 1)}>scrollEnd</button>
      <button onclick={() => list?.scrollTo(0)}>scrollStart</button>
      <button onclick={() => list?.scrollTo(Math.floor(items.length / 2))}>scrollMid</button>
      <button onclick={() => (show = !show)}>toggle</button>
    </box>
    <label>{`items=${items.length} selected=${selected} activated=${activated}`}</label>
    {#if show}
    <ListView
      bind:this={list}
      {items}
      key={(it) => it.id}
      bind:selected
      vexpand="true"
      minHeight="300"
      onactivate={(item, pos) => (activated = `${pos}:${item.id}`)}
    >
      {#snippet row(item, index)}
        <box orientation="horizontal" spacing="6">
          <label>{index}</label>
          <label>{item.text}</label>
        </box>
      {/snippet}
    </ListView>
    {/if}
  </box>
</window>
