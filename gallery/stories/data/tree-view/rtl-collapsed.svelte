<script>
  import { TreeView } from '@svelters/lists';
  let { disabled = false } = $props();
  const roots = [
    { id: 'r', label: 'اللوحة الرئيسية', kids: [{ id: 'r/1', label: 'المنظم', kids: [{ id: 'r/1/a', label: 'U1' }] }, { id: 'r/2', label: 'المعالج' }] },
    { id: 's', label: 'المكتبة', kids: [{ id: 's/1', label: 'R0603' }] },
  ];
  let expanded = $state(['r']);
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <TreeView anchor="subject" accessible-name="التصميم" {roots} children={(n) => n.kids ?? []} hasChildren={(n) => !!n.kids} bind:expanded row-height={22} flex-grow={1} enabled={!disabled}>
    {#snippet row(node)}
      <label>{node.label}</label>
    {/snippet}
  </TreeView>
</box>
