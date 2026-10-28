<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} gap={4} align-items="stretch">
  <label anchor="subject" enabled={!disabled}>مرحبا بالعالم، هذا نص من اليمين إلى اليسار.</label>
  <label>שלום עולם — mixed with English 123 words.</label>
</box>
