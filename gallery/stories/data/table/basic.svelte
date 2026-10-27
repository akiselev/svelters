<script>
  let { striped = true, disabled = false } = $props();
  const parts = Array.from({ length: 40 }, (_, i) => [`R${i + 1}`, `${(i % 9) + 1}.${i % 10}k`, i % 2 ? '0402' : '0603']);
  let table = $state();
  let range = $state([0, -1]);
  $effect(() => {
    const [first, last] = range;
    if (table && last >= first) table.call('set-rows', first, parts.slice(first, last + 1));
  });
</script>

<box padding={8} flex-grow={1}>
  <table
    bind:this={table}
    anchor="subject"
    accessible-name="Parts"
    row-count={parts.length}
    {striped}
    flex-grow={1}
    enabled={!disabled}
    onrange={(first, last) => (range = [first, last])}
  >
    <table-column title="Reference" width={100} />
    <table-column title="Value" width={80} align="end" />
    <table-column title="Footprint" width={110} />
  </table>
</box>
