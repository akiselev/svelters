<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <date-picker anchor="subject" accessible-name="Release date" enabled={!disabled} />
</box>
