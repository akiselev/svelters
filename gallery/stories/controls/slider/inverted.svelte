<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <slider anchor="subject" accessible-name="Distance" value={30} inverted enabled={!disabled} />
</box>
