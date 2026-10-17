<script>
  let n = $state(1);
  export function inc() { n++; }
</script>
{#snippet cell(x)}<cell v={x}>{x}</cell>{/snippet}
<box>
  {@render cell(n)}
  {@render cell(n * 10)}
</box>
