<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <calendar anchor="subject" value="2026-10-14" first-day="saturday" accessible-name="تاريخ الإصدار" enabled={!disabled} />
</box>
