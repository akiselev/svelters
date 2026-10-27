<script>
  let { value = 35, disabled = false } = $props();
</script>

<box padding={12}>
  <slider anchor="subject" accessible-name="Opacity" min={0} max={100} {value} enabled={!disabled} />
</box>
