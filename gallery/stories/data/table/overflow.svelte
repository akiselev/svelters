<script>
  let { disabled = false } = $props();
  const rows = Array.from({ length: 30 }, (_, i) => [`U${i + 1}`, `Microcontroller with a long description number ${i + 1}`, 'LQFP-64_10x10mm_P0.5mm', `${i * 3} pcs`]);
  let table = $state();
  let range = $state([0, -1]);
  $effect(() => {
    const [first, last] = range;
    if (table && last >= first) table.call('set-rows', first, rows.slice(first, last + 1));
  });
</script>

<box padding={8} flex-grow={1}>
  <table
    bind:this={table}
    anchor="subject"
    accessible-name="Parts"
    row-count={rows.length}
    sticky-columns={1}
    stretch-last={false}
    flex-grow={1}
    enabled={!disabled}
    onrange={(first, last) => (range = [first, last])}
  >
    <table-column title="Ref" width={60} />
    <table-column title="Description" width={180} elide="middle" />
    <table-column title="Footprint" width={160} />
    <table-column title="Stock" width={80} align="end" />
  </table>
</box>
