<script>
  let open = $state(true);
  let body = $state();
  let reason = $state('');

  // Toggling after a native close reads the bound child label: its node is dead and carries the
  // ABI v2 death reason ('parent': its window was closed), not a generic 'destroyed'.
  function toggle() {
    if (open) {
      try {
        body.getProp('text');
        reason = 'alive';
      } catch (e) {
        reason = e.reason;
      }
    }
    open = !open;
  }
</script>

<main-window window-title="Main">
  <box-layout direction="top-to-bottom" spacing="8">
    <label>{open ? 'child open' : `child closed (${reason})`}</label>
    <push-button onclicked={toggle}>Toggle child</push-button>
  </box-layout>
</main-window>

{#if open}
  <main-window window-title="Child">
    <label bind:this={body}>Child body</label>
  </main-window>
{/if}
