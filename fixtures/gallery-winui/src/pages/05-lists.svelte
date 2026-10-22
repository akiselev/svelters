<!-- kinds: ListView ListViewItem GridView GridViewItem -->
<script>
  const fruits = ['Apple', 'Banana', 'Cherry', 'Damson'];
</script>

<stack-panel orientation="vertical" spacing="6">
  <list-view header="List view" selection-mode="single" height={160}>
    {#each fruits as f}
      <list-view-item>{f}</list-view-item>
    {/each}
  </list-view>
  <grid-view header="Grid view">
    {#each fruits as f}
      <grid-view-item>{f}</grid-view-item>
    {/each}
  </grid-view>
</stack-panel>
