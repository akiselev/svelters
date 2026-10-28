<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12}>
  <progress-bar anchor="subject" fraction={0.4} text="جارٍ التحميل" enabled={!disabled} />
</box>
