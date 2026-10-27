<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <label anchor="subject" enabled={!disabled}>The quick brown fox jumps over the lazy dog.</label>
</box>
