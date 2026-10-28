<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12}>
  <text-edit anchor="subject" text="مقاومة ١٠ كيلو" accessible-name="Name" enabled={!disabled} />
</box>
