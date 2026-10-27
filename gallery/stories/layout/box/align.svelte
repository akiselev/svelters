<script>
  let { align = 'start' } = $props();
</script>

<box anchor="subject" flex-direction="row" align-items={align} gap={6} padding={8} height={100}>
  <frame frame-style="group"><label>Short</label></frame>
  <frame frame-style="group"><label font-weight={700}>Tall text</label><label>second line</label></frame>
  <frame frame-style="group"><label>Mid</label></frame>
</box>
