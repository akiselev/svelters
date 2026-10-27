<script>
  import { TreeView } from '@svelters/lists';
  let { decorated = true, disabled = false } = $props();
  const roots = [
    { id: 'pcb', label: 'Main board', kids: [
      { id: 'pcb/u1', label: 'U1 Microcontroller' },
      { id: 'pcb/u2', label: 'U2 Regulator', kids: [{ id: 'pcb/u2/c1', label: 'C1 Input' }, { id: 'pcb/u2/c2', label: 'C2 Output' }] },
    ] },
    { id: 'lib', label: 'Library', kids: [{ id: 'lib/r', label: 'Resistors' }] },
  ];
  let selected = $state(['pcb/u2/c1']);
  let expanded = $state(['pcb', 'pcb/u2']);
</script>

<box padding={8} flex-grow={1}>
  <TreeView
    anchor="subject"
    accessible-name="Design"
    {roots}
    children={(n) => n.kids ?? []}
    hasChildren={(n) => !!n.kids}
    bind:selected
    bind:expanded
    root-decorated={decorated}
    row-height={22}
    flex-grow={1}
    enabled={!disabled}
  >
    {#snippet row(node)}
      <label>{node.label}</label>
    {/snippet}
  </TreeView>
</box>
