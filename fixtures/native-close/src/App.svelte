<script>
  let open = $state(true);
</script>

<window title="Main">
  <box orientation="vertical" spacing="8">
    <label>{open ? 'child open' : 'child closed'}</label>
    <button onclick={() => (open = !open)}>Toggle child</button>
  </box>
</window>

{#if open}
  <window title="Child">
    <label>Child body</label>
  </window>
{/if}
