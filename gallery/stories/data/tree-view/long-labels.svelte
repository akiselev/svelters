<script>
  import { TreeView } from '@svelters/lists';
  let { disabled = false } = $props();
  const roots = [
    { id: 'x', label: 'A folder whose name is much longer than the tree is wide', kids: [
      { id: 'x/y', label: 'A nested file with an equally long name that has to be cut or clipped.kicad_sch' },
    ] },
  ];
  let expanded = $state(['x']);
</script>

<box padding={8} flex-grow={1}>
  <TreeView anchor="subject" accessible-name="Files" {roots} children={(n) => n.kids ?? []} hasChildren={(n) => !!n.kids} bind:expanded flex-grow={1} enabled={!disabled}>
    {#snippet row(node)}
      <label elide="end">{node.label}</label>
    {/snippet}
  </TreeView>
</box>
