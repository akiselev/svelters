<script>
  let { value = '2026-10-14', disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <date-picker anchor="subject" {value} open accessible-name="تاريخ الإصدار" enabled={!disabled} />
</box>
