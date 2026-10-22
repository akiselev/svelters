<script>
  // A panel between the toolbar and the app: it forwards the child's `accepted` callback prop
  // (bare `on:accepted`) and passes its own handler for `rejected` (`on:rejected={h}` is the
  // prop `onrejected={h}`).
  import Child from './Child.svelte';
  let n = $state(0);
</script>

<box>
  <Child title="t" on:accepted on:rejected={() => n++} />
  <svelte:component this={Child} on:accepted />
</box>
