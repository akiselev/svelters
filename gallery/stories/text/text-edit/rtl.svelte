<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <text-edit anchor="subject" text="مقاومة ١٠ كيلو" accessible-name="Name" enabled={!disabled} />
</box>
