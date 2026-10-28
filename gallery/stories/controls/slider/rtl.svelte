<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} gap={12}>
  <slider anchor="subject" accessible-name="الجودة" min={0} max={100} value={30} marks="0:منخفضة,50:متوسطة,100:عالية" enabled={!disabled} />
</box>
