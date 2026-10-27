<script>
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={12} align-items="center">
  <radio-button text="mm" checked />
  <radio-button anchor="subject" text="in" enabled={!disabled} />
  <radio-button text="mil" />
</box>
