<script>
  // More rows than the cell holds: the overlay scroll bar over the rows (POL2-LAYOUT 1)
  import { TreeView } from '@svelters/lists';
  let { disabled = false } = $props();
  const roots = [
    { id: 'nets', label: 'Nets', kids: Array.from({ length: 24 }, (_, i) => ({ id: `nets/${i}`, label: `N${i + 1}` })) },
  ];
  let expanded = $state(['nets']);
</script>

<box padding={8} flex-grow={1}>
  <TreeView anchor="subject" accessible-name="Nets" {roots} children={(n) => n.kids ?? []} hasChildren={(n) => !!n.kids} bind:expanded row-height={22} flex-grow={1} enabled={!disabled}>
    {#snippet row(node)}
      <label>{node.label}</label>
    {/snippet}
  </TreeView>
</box>
