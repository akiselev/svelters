<script>
  let { wrap = 'wrap' } = $props();
  const tags = ['power', 'analog', 'digital', 'high speed', 'differential', 'ground', 'clock', 'reset'];
</script>

<box anchor="subject" flex-direction="row" flex-wrap={wrap} gap={4} padding={8} width={304}>
  {#each tags as tag}
    <frame frame-style="group"><label>{tag}</label></frame>
  {/each}
</box>
