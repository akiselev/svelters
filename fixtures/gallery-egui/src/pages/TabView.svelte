<script>
  // WG4a: <tab-view> with bind:selected; the closable tab removes itself on close-requested.
  let selected = $state(0);
  let tabs = $state(['General', 'Advanced', 'Notes']);
</script>

<box gap={4} flex-grow={1}>
  <label>Selected tab: {selected}</label>
  <tab-view bind:selected flex-grow={1}>
    {#each tabs as t, i (t)}
      <tab title={t} closable={t === 'Notes'} onclose-requested={() => tabs.splice(i, 1)} padding={6}>
        <label>{t} page</label>
        {#if t === 'General'}
          <checkbox text="Snap to grid" />
        {/if}
      </tab>
    {/each}
  </tab-view>
  <label>Tabs: {tabs.length}</label>
</box>
