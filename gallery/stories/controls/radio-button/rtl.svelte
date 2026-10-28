<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={8} gap={2} align-items="start">
  <radio-button text="مسافات" checked />
  <radio-button anchor="subject" text="علامات الجدولة" enabled={!disabled} />
</box>
