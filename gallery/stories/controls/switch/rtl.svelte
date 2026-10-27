<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <switch anchor="subject" text="حفظ تلقائي" active enabled={!disabled} />
</box>
