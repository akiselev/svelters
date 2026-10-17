<script>
  let n = $state(0);
  export function inc() { n++; }
</script>
<box>
  {#if n % 2}
    <label>odd</label>
  {:else if n === 2}
    <label>two</label>
  {:else}
    <label>even</label>
  {/if}
</box>
