<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start" width={336}>
  <toggle-button anchor="subject" text="Show hidden files and folders in every explorer view" enabled={!disabled} />
</box>
