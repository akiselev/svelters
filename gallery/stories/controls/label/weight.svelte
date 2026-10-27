<script>
  let { weight = 400, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <label anchor="subject" font-weight={weight} enabled={!disabled}>Project settings</label>
</box>
