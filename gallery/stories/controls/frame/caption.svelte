<script>
  let { disabled = false } = $props();
</script>

<box padding={8}>
  <frame anchor="subject" label="Export" gap={4} enabled={!disabled}>
    <radio-button text="Gerber" checked />
    <radio-button text="ODB++" />
  </frame>
</box>
