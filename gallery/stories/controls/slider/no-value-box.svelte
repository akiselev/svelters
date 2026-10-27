<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <slider anchor="subject" accessible-name="Volume" value={60} show-value={false} enabled={!disabled} />
</box>
