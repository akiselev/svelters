<script>
  let { size = 20, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <spinner anchor="subject" accessible-name="Loading" {size} enabled={!disabled} />
</box>
