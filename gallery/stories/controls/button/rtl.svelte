<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <button anchor="subject" text="حفظ التغييرات" enabled={!disabled} />
</box>
