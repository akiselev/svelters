<script>
  let { justify = 'start' } = $props();
</script>

<grid anchor="subject" grid-template-columns="1fr 1fr" justify-items={justify} gap={4} padding={8}>
  <frame frame-style="group"><label>One</label></frame>
  <frame frame-style="group"><label>Two</label></frame>
  <frame frame-style="group"><label>Three</label></frame>
  <frame frame-style="group"><label>Four</label></frame>
</grid>
