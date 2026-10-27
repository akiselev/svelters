<script>
  let { disabled = false } = $props();
</script>

<box padding={8} width={300}>
  <collapsing-header anchor="subject" text="Settings that apply to every board in the project, including the ones not open" enabled={!disabled}>
    <label>Content</label>
  </collapsing-header>
</box>
