<script>
  let { direction = 'row' } = $props();
</script>

<box anchor="subject" flex-direction={direction} gap={6} padding={8}>
  <frame frame-style="group"><label>One</label></frame>
  <frame frame-style="group"><label>Two</label></frame>
  <frame frame-style="group"><label>Three</label></frame>
</box>
