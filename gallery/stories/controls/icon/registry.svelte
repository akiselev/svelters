<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <icon anchor="subject" name="file.save" frame tooltip="Save" enabled={!disabled} />
</box>
