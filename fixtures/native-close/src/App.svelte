<script>
  let open = $state(true);
  let body = $state();
  let reason = $state('');

  // Toggling after a native close reads the bound child label: its node is dead and carries the
  // ABI v2 death reason ('parent': its window was closed), not a generic 'destroyed'.
  function toggle() {
    if (open) {
      try {
        body.getProp('label');
        reason = 'alive';
      } catch (e) {
        reason = e.reason;
      }
    }
    open = !open;
  }
</script>

<window title="Main">
  <box orientation="vertical" spacing="8">
    <label>{open ? 'child open' : `child closed (${reason})`}</label>
    <button onclicked={toggle}>Toggle child</button>
  </box>
</window>

{#if open}
  <window title="Child">
    <label bind:this={body}>Child body</label>
  </window>
{/if}
