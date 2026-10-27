<script>
  let { justify = 'start' } = $props();
</script>

<box anchor="subject" flex-direction="row" justify-content={justify} padding={8} width={304}>
  <frame frame-style="group"><label>One</label></frame>
  <frame frame-style="group"><label>Two</label></frame>
  <frame frame-style="group"><label>Three</label></frame>
</box>
