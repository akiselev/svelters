<script>
  // WG4a: <table> over a JS array of 10k parts. The table asks for the rows it shows (`onrange`);
  // the page answers with that slice (`set-rows`). A header click sorts by that column.
  let { count = 10000 } = $props();
  // raw: the array is replaced, never changed in place (a proxy per part makes sorting 10k slow)
  let parts = $state.raw(
    Array.from({ length: count }, (_, i) => ({ name: `R${i}`, value: `${(i % 97) + 1}k`, footprint: i % 2 ? '0402' : '0603' })),
  );
  const keys = ['name', 'value', 'footprint'];
  let table = $state();
  let range = $state([0, -1]);
  let selected = $state(-1);
  let sortBy = $state(-1);
  let activated = $state(-1);

  const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

  // the data window: re-sent when the range or the order changes
  $effect(() => {
    const [first, last] = range;
    if (!table || last < first) return;
    const rows = parts.slice(first, last + 1).map((p) => [p.name, p.value, p.footprint]);
    table.call('set-rows', first, rows);
  });

  function sort(col) {
    sortBy = col;
    // keys computed once (no state read per comparison)
    const key = (p) => (col === 0 ? Number(p.name.slice(1)) : col === 1 ? Number.parseInt(p.value, 10) : p[keys[col]]);
    parts = parts
      .map((p) => [key(p), p])
      .sort((a, b) => cmp(a[0], b[0]))
      .map((e) => e[1]);
  }
</script>

<box gap={4} flex-grow={1}>
  <label>Selected row: {selected}</label>
  <label>Activated row: {activated}</label>
  <table
    bind:this={table}
    accessible-name="Parts"
    row-count={parts.length}
    flex-grow={1}
    bind:selected
    onrange={(first, last) => (range = [first, last])}
    onactivated={(row) => (activated = row)}
    onheader-clicked={sort}
  >
    <table-column title="Name" width={90} sort={sortBy === 0 ? 'ascending' : 'none'} />
    <table-column title="Value" width={80} align="end" sort={sortBy === 1 ? 'ascending' : 'none'} />
    <table-column title="Footprint" width={110} sort={sortBy === 2 ? 'ascending' : 'none'} />
  </table>
</box>
