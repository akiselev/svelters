<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <spinner anchor="subject" accessible-name="Loading" enabled={!disabled} />
</box>
