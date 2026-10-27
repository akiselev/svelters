<script>
  let { direction = 'vertical' } = $props();
  const rows = Array.from({ length: 20 }, (_, i) => `Row ${i + 1}: a line of sample text wide enough to overflow the area sideways`);
</script>

<box padding={8} flex-grow={1}>
  <scroll-area anchor="subject" {direction} height={120} width={260}>
    {#each rows as row}
      <label wrap-mode="extend">{row}</label>
    {/each}
  </scroll-area>
</box>
