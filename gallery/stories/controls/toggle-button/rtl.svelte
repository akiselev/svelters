<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <toggle-button anchor="subject" text="غامق" checked enabled={!disabled} />
</box>
