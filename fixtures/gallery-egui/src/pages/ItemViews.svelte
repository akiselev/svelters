<script>
  // WG5a: a keyboard-driven, editable table (the shape of cadatomic's property grid / BOM).
  // - rows are keyed by part id (`set-ids`, with names for type-ahead), so selection and the cursor
  //   follow the parts through the filter;
  // - Qty edits with a column `editor` snippet (realised once, rebound per edit), Note with the
  //   built-in text field; `oncommit` validates and can reject with a reason.
  // Keys: arrows, PageUp/PageDown, Home/End, Shift/Ctrl, Space, Ctrl+A, type a name (type-ahead),
  // Left/Right for the column, F2 or typing to edit, Enter/Escape/Tab in the editor.
  let { count = 1000 } = $props();
  let parts = $state.raw(Array.from({ length: count }, (_, i) => ({ id: i + 1, name: `R${i + 1}`, qty: (i % 9) + 1, note: '' })));
  let filter = $state('');
  const shown = $derived(filter ? parts.filter((p) => p.name.includes(filter)) : parts);
  let table = $state();
  let range = $state([0, -1]);
  let selected = $state([]);
  let current = $state(-1);
  let committed = $state('');

  // the row keys (and type-ahead labels) whenever the shown rows change
  $effect(() => {
    if (table) table.call('set-ids', shown.map((p) => p.id), shown.map((p) => p.name));
  });
  // the data window
  $effect(() => {
    const [first, last] = range;
    if (!table || last < first) return;
    table.call('set-rows', first, shown.slice(first, last + 1).map((p) => [p.name, String(p.qty), p.note]));
  });

  function commit(e, id, column, value) {
    if (column === 1) {
      const q = Number(value);
      if (!Number.isInteger(q) || q < 0 || q > 99) {
        e.result = 'Qty must be a whole number from 0 to 99';
        return;
      }
      parts = parts.map((p) => (p.id === id ? { ...p, qty: q } : p));
    } else {
      parts = parts.map((p) => (p.id === id ? { ...p, note: String(value) } : p));
    }
    committed = `R${id}: ${value}`;
  }
</script>

{#snippet quantity(edit)}
  <text-edit accessible-name="Quantity" flex-grow={1} bind:text={edit.value} />
{/snippet}

<box gap={4} flex-grow={1}>
  <box flex-direction="row" gap={8}>
    <text-edit hint-text="Filter" accessible-name="Filter" bind:text={filter} />
    <label>Selected: {selected.join(', ')}</label>
    <label>Current: {current}</label>
    <label>Committed: {committed}</label>
  </box>
  <table
    bind:this={table}
    accessible-name="Parts"
    selection="multi"
    flex-grow={1}
    bind:selected
    bind:current
    onrange={(first, last) => (range = [first, last])}
    oncommit={commit}
  >
    <table-column title="Name" width={90} />
    <table-column title="Qty" width={70} align="end" editable editor={quantity} />
    <table-column title="Note" width={160} editable />
  </table>
</box>
