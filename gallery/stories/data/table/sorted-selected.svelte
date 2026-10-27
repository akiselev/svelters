<script>
  let { disabled = false } = $props();
  const parts = Array.from({ length: 40 }, (_, i) => [`C${i + 1}`, `${(i + 1) * 10} nF`, i % 3 ? 'X7R' : 'C0G']);
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
    accessible-name="Capacitors"
    row-count={parts.length}
    selection="multi"
    selected={[1, 2, 4]}
    current={2}
    sort-column={1}
    sort-order="ascending"
    sortable
    flex-grow={1}
    enabled={!disabled}
    onrange={(first, last) => (range = [first, last])}
  >
    <table-column title="Reference" width={100} />
    <table-column title="Capacitance" width={110} align="end" />
    <table-column title="Dielectric" width={90} header-align="center" align="center" />
  </table>
</box>
