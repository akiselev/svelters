<script>
  let { checked = false, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <checkbox anchor="subject" accessible-name="Select row" {checked} enabled={!disabled} />
</box>
