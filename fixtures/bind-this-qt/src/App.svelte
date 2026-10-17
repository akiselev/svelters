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

<main-window window-title="bind:this" bind:this={win}>
  <box-layout direction="top-to-bottom" spacing="6">
    <line-edit bind:this={entry} text="seed" ontext-changed={() => (edited = 'yes')} />
    <push-button onclicked={readEntry}>Read</push-button>
    <push-button onclicked={hide}>Hide</push-button>
    <push-button onclicked={probe}>Probe</push-button>
    <label>{read}</label>
    <label>reads: {reads}</label>
    <label>edited: {edited}</label>
    <label>stale: {stale}</label>
    {#if show}
      <line-edit bind:this={gone} text="temp" />
    {/if}
  </box-layout>
</main-window>
