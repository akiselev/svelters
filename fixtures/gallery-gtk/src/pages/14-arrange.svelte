<!-- kinds: ListBox FlowBox -->
<script>
  // Sort, filter and headers for ListBox/FlowBox (X8, G8): the native sort/filter/header funcs are not exposed.
  // JS arranges the data (`arrange`) and renders the result keyed; the first row of each group carries
  // `parent:header` (the row's section header). GTK only sees inserts, removes and moves.
  import { arrange } from '@svelters/lists/arrange';
  const food = [
    { name: 'pear', kind: 'Fruit' },
    { name: 'leek', kind: 'Vegetables' },
    { name: 'apple', kind: 'Fruit' },
    { name: 'beet', kind: 'Vegetables' },
    { name: 'plum', kind: 'Fruit' },
  ];
  let desc = $state(false);
  let onlyFruit = $state(false);
  const rows = $derived(
    arrange(food, {
      filter: (f) => !onlyFruit || f.kind === 'Fruit',
      compare: (a, b) => a.kind.localeCompare(b.kind) || (desc ? b.name.localeCompare(a.name) : a.name.localeCompare(b.name)),
      group: (f) => f.kind,
    }),
  );
</script>

<box orientation="vertical" spacing={6}>
  <label>rows: {rows.map((r) => r.item.name).join(' ')}</label>
  <box orientation="horizontal" spacing={6}>
    <button label={desc ? 'ascending' : 'descending'} onclick={() => (desc = !desc)} />
    <button label={onlyFruit ? 'all' : 'only fruit'} onclick={() => (onlyFruit = !onlyFruit)} />
  </box>
  <listbox selectionMode="none">
    {#each rows as r (r.item.name)}
      <label parent:header={r.header}>{r.item.name}</label>
    {/each}
  </listbox>
  <flowbox maxChildrenPerLine={5}>
    {#each rows as r (r.item.name)}
      <label>{r.item.name}</label>
    {/each}
  </flowbox>
</box>
