<script>
  // bind:this gives the renderer's node: call() and getProp() reach the native widget, and the node
  // goes stale (StaleHandleError) once its widget is destroyed.
  let win = $state();
  let entry = $state();
  let gone = $state();
  let show = $state(true);
  let kept;
  let read = $state('(none)');
  let reads = $state(0);
  let edited = $state('no');
  let stale = $state('(none)');

  function readEntry() {
    read = entry.getProp('text');
    win.call('present');
    reads++;
  }

  function hide() {
    kept = gone;
    show = false;
  }

  function probe() {
    try {
      kept.getProp('text');
      stale = 'alive';
    } catch (e) {
      stale = `${e.name}: ${e.reason}`;
    }
  }
</script>

<window title="bind:this" bind:this={win}>
  <box orientation="vertical" spacing="6">
    <entry bind:this={entry} text="seed" onchanged={() => (edited = 'yes')} />
    <button onclick={readEntry}>Read</button>
    <button onclick={hide}>Hide</button>
    <button onclick={probe}>Probe</button>
    <label>{read}</label>
    <label>reads: {reads}</label>
    <label>edited: {edited}</label>
    <label>stale: {stale}</label>
    {#if show}
      <entry bind:this={gone} text="temp" />
    {/if}
  </box>
</window>
