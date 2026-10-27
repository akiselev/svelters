<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <progress-bar anchor="subject" accessible-name="Progress" fraction={0.62} show-percentage enabled={!disabled} />
</box>
