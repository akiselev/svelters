<script>
  let count = $state(0);
</script>
<box version={2}>
  <button disabled={count > 2} onclick={() => count++}>Count is {count}</button>
</box>
