<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <switch anchor="subject" text="حفظ تلقائي" active enabled={!disabled} />
</box>
