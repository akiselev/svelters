<script>
  let { disabled = false } = $props();
</script>

<box padding={8} gap={2}>
  <collapsing-header text="General" accordion="settings">
    <label>General settings</label>
  </collapsing-header>
  <collapsing-header anchor="subject" text="Editor" accordion="settings" open enabled={!disabled}>
    <label>Editor settings</label>
  </collapsing-header>
  <collapsing-header text="Terminal" accordion="settings">
    <label>Terminal settings</label>
  </collapsing-header>
</box>
