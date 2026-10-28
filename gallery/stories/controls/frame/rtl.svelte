<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={8}>
  <frame anchor="subject" label="تصدير" gap={4} enabled={!disabled}>
    <label>المحتوى</label>
  </frame>
</box>
