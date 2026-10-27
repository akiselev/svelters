<script>
  let { index = 0 } = $props();
</script>

<box padding={8} align-items="start">
  <stack anchor="subject" visible-index={index}>
    <frame frame-style="group"><label>First page</label></frame>
    <frame frame-style="group"><label>Second page</label></frame>
    <frame frame-style="group"><label>Third page</label></frame>
  </stack>
</box>
