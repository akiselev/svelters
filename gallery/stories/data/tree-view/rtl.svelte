<script>
  import { TreeView } from '@svelters/lists';
  let { disabled = false } = $props();
  const roots = [{ id: 'r', label: 'اللوحة الرئيسية', kids: [{ id: 'r/1', label: 'المنظم' }, { id: 'r/2', label: 'المعالج' }] }];
  let expanded = $state(['r']);
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <TreeView anchor="subject" accessible-name="التصميم" {roots} children={(n) => n.kids ?? []} hasChildren={(n) => !!n.kids} bind:expanded flex-grow={1} enabled={!disabled}>
    {#snippet row(node)}
      <label>{node.label}</label>
    {/snippet}
  </TreeView>
</box>
