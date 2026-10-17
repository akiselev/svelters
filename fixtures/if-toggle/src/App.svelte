<script>
  let show = $state(false);
</script>

<window title="If toggle">
  <headerbar data-slot="titlebar" title="Toggle" />
  <box orientation="vertical" spacing="8">
    <button onclick={() => (show = !show)}>Toggle</button>
    {#if show}
      <label>Shown</label>
    {:else}
      <label>Hidden</label>
    {/if}
  </box>
</window>
