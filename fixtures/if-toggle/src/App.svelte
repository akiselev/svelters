<script>
  let show = $state(false);
</script>

<window title="If toggle">
  <header-bar data-slot="titlebar" title="Toggle" />
  <box orientation="vertical" spacing="8">
    <button onclicked={() => (show = !show)}>Toggle</button>
    {#if show}
      <label>Shown</label>
    {:else}
      <label>Hidden</label>
    {/if}
  </box>
</window>
