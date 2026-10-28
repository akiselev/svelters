<script>
  let { disabled = false } = $props();
  const rows = [['مقاومة', '١٠ كيلو'], ['مكثف', '١٠٠ نانو'], ['ملف', '٤٫٧ ميكرو']];
  let table = $state();
  let range = $state([0, -1]);
  $effect(() => {
    const [first, last] = range;
    if (table && last >= first) table.call('set-rows', first, rows.slice(first, last + 1));
  });
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <table
    bind:this={table}
    anchor="subject"
    accessible-name="القطع"
    row-count={rows.length}
    flex-grow={1}
    enabled={!disabled}
    onrange={(first, last) => (range = [first, last])}
  >
    <table-column title="الاسم" width={140} />
    <table-column title="القيمة" width={120} />
  </table>
</box>
