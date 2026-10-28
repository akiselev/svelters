<script>
  import { TreeView } from '@svelters/lists';
  let { disabled = false } = $props();
  const roots = [
    { id: 'a', label: 'Schematics', kids: [{ id: 'a/1', label: 'Power' }] },
    { id: 'b', label: 'Boards', kids: [{ id: 'b/1', label: 'Main' }] },
    { id: 'c', label: 'Outputs' },
  ];
</script>

<box padding={8} flex-grow={1}>
  <TreeView anchor="subject" accessible-name="Project" {roots} children={(n) => n.kids ?? []} hasChildren={(n) => !!n.kids} flex-grow={1} enabled={!disabled}>
    {#snippet row(node)}
      <label>{node.label}</label>
    {/snippet}
  </TreeView>
</box>
