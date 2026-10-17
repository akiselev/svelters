<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- Components: props, $bindable two-way props, callback props, keyed each of components,
     teardown of a component's natives when its item is removed. -->
<script>
  import Row from './Row.svelte';

  let items = $state([
    { id: 1, label: 'Milk', done: false },
    { id: 2, label: 'Eggs', done: true },
  ]);
  let remaining = $derived(items.filter((i) => !i.done).length);
</script>

<main-window window-title="Components">
  <box-layout direction="top-to-bottom" spacing="4">
    <label>{remaining} left</label>
    {#each items as item (item.id)}
      <Row label={item.label} bind:done={item.done} onremove={() => (items = items.filter((i) => i.id !== item.id))} />
    {/each}
  </box-layout>
</main-window>
