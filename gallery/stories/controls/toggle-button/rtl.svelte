<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <toggle-button anchor="subject" text="غامق" checked enabled={!disabled} />
</box>
