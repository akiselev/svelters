<script>
  const rows = Array.from({ length: 30 }, (_, i) => `Row ${i + 1}`);
</script>

<box padding={8}>
  <scroll-area anchor="subject" scroll-y={200} height={120} width={260}>
    {#each rows as row}
      <label>{row}</label>
    {/each}
  </scroll-area>
</box>
