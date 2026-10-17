<script>
  // One mounted row: renders the app's row snippet with the slot's reactive item and index.
  let { row, cell } = $props();
</script>

{@render row(cell.item, cell.index)}
