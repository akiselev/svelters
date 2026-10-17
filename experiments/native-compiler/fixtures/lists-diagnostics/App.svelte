<!-- Compile-time checks of virtualized lists (<ListView> from @svelters/lists, lower/lists.rs). -->
<script>
  import { ListView as List } from '@svelters/lists';

  let items = $state([]);
  let x = $state(0);
</script>

<window title="List diagnostics">
  <box orientation="vertical">
    <List {items} layout="table" selection="multiple" bind:x>
      {#snippet row(item, index, extra)}
        <window title="not a row" />
      {/snippet}
      <label>not rendered</label>
    </List>
    <List {items} />
  </box>
</window>
