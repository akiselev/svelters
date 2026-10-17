<script>
  let count = $state(0);
</script>
<box>
  <button disabled={count > 2} onclick={() => count++}>Clicked {count}</button>
</box>
