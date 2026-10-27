<script>
  let { disabled = false } = $props();
</script>

<box padding={6} flex-direction="row" gap={12} height={64}>
  <progress-bar anchor="subject" accessible-name="Left level" orientation="vertical" fraction={0.7} enabled={!disabled} />
  <progress-bar accessible-name="Right level" orientation="vertical" fraction={0.3} inverted />
</box>
