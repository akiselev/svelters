<script>
  import Child from './Child.svelte';
  function h() {}
</script>

<box>
  <Child on:accepted onaccepted={h} />
  <Child on:accepted={h} onaccepted={h} />
  <Child on:accepted={h} onrejected={h} />
</box>
