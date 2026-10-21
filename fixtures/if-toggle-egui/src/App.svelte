<script>
  // Egui twin of if-toggle (no header bar: egui windows have no titlebar slot).
  let show = $state(false);
</script>

<window title="If toggle">
  <box gap={8}>
    <button onclicked={() => (show = !show)}>Toggle</button>
    {#if show}
      <label>Shown</label>
    {:else}
      <label>Hidden</label>
    {/if}
  </box>
</window>
