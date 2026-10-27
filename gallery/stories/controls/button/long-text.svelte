<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start" width={336}>
  <button anchor="subject" text="Apply these settings to every open project in the workspace" enabled={!disabled} />
</box>
