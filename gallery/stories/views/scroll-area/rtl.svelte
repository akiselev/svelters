<script>
  const rows = Array.from({ length: 30 }, (_, i) => `الصف ${i + 1} — a row wider than the area so it scrolls both ways`);
</script>

<box dir="rtl" padding={8}>
  <scroll-area anchor="subject" direction="both" scroll-y={200} height={120} width={260}>
    {#each rows as row}
      <label wrap-mode="extend">{row}</label>
    {/each}
  </scroll-area>
</box>
