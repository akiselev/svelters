<script>
  // WG5b: <TreeView> (@svelters/lists) on egui's `tree-view`. A part tree: "Assembly" loads its
  // children lazily (a Promise, when it first expands), "Library" has them inline, "Big" holds
  // 100k parts (only the rows in view are bound). Multi-selection by key survives the filter; rows
  // drag as `cad/part` and take `cad/*` drops (before, after or into a row). Keyboard: arrows,
  // Right/Left expand and collapse, Space, Enter, type-ahead, Ctrl+C. The kittest is
  // crates/svelters-egui/tests/treeview.rs (through TreeWindow.svelte).
  import { TreeView } from '@svelters/lists';

  let { big = 100000 } = $props();

  const assembly = [
    { id: 'asm/frame', label: 'Frame', kids: [{ id: 'asm/frame/bolt', label: 'Bolt' }, { id: 'asm/frame/nut', label: 'Nut' }] },
    { id: 'asm/board', label: 'Board' },
  ];
  const library = [
    { id: 'lib/r', label: 'Resistor' },
    { id: 'lib/c', label: 'Capacitor' },
    { id: 'lib/l', label: 'Inductor' },
  ];
  const roots = [
    { id: 'asm', label: 'Assembly', lazy: true },
    { id: 'lib', label: 'Library', kids: library },
    { id: 'big', label: `Big (${big})`, big: true },
  ];
  let loads = $state(0);
  let bigParts = null;
  function children(node) {
    if (node.lazy) {
      loads++;
      return Promise.resolve(assembly);
    }
    if (node.big) return (bigParts ??= Array.from({ length: big }, (_, i) => ({ id: `big/${i}`, label: `Part ${i}` })));
    return node.kids ?? [];
  }
  const hasChildren = (node) => !!(node.lazy || node.big || node.kids);

  let selected = $state([]);
  let expanded = $state([]);
  let query = $state('');
  let dropped = $state('-');
  let activated = $state('-');
  const filter = $derived(query ? (node) => node.label.toLowerCase().includes(query.toLowerCase()) : undefined);
</script>

<box gap={4} flex-grow={1}>
  <box flex-direction="row" gap={8}>
    <text-edit accessible-name="filter" hint-text="filter" bind:text={query} />
    <label>{`loads: ${loads}`}</label>
    <label>{`activated: ${activated}`}</label>
  </box>
  <label>{`selected: ${selected.join(',')}`}</label>
  <label>{`expanded: ${expanded.join(',')}`}</label>
  <label>{`dropped: ${dropped}`}</label>
  <TreeView
    accessible-name="Parts"
    {roots}
    {children}
    {hasChildren}
    {filter}
    selection="multi"
    bind:selected
    bind:expanded
    row-height={22}
    rowDragType="cad/part"
    rowDropTypes="cad/*"
    flex-grow={1}
    onactivate={(node) => (activated = node.label)}
    ondroprow={(key, where, type, data) => (dropped = `${where} ${key} ${type} ${data}`)}
  >
    {#snippet row(node, level)}
      <box flex-direction="row" gap={6} tooltip={`level ${level}`}>
        <label>{node.label}</label>
      </box>
    {/snippet}
  </TreeView>
</box>
