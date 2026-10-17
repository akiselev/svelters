<!-- Qt-vocabulary copy of ../App.svelte (the qt compile target). -->
<!-- Compile-time checks of virtualized lists (<ListView> from @svelters/lists, lower/lists.rs). -->
<script>
  import { ListView as List } from '@svelters/lists';

  let items = $state([]);
  let x = $state(0);
</script>

<main-window window-title="List diagnostics">
  <box-layout direction="top-to-bottom">
    <List {items} layout="table" selection="multiple" bind:x>
      {#snippet row(item, index, extra)}
        <main-window window-title="not a row" />
      {/snippet}
      <label>not rendered</label>
    </List>
    <List {items} />
  </box-layout>
</main-window>
