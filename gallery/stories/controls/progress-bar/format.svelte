<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <progress-bar anchor="subject" accessible-name="Copied" min={0} max={250} value={75} format="%v of %m MB" enabled={!disabled} />
</box>
