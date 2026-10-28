<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <color-picker anchor="subject" value="#3a7bd5" accessible-name="لون الطبقة" enabled={!disabled} />
</box>
