<script>
  let { homogeneous = true } = $props();
</script>

<box padding={8} flex-direction="row" gap={6} align-items="start">
  <stack anchor="subject" {homogeneous}>
    <frame frame-style="group"><label>Small</label></frame>
    <frame frame-style="group"><label>A much larger page</label><label>with two lines</label></frame>
  </stack>
  <label>← sized</label>
</box>
