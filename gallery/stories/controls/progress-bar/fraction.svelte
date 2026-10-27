<script>
  let { fraction = 0.4, disabled = false } = $props();
</script>

<box padding={12}>
  <progress-bar anchor="subject" accessible-name="Progress" {fraction} enabled={!disabled} />
</box>
