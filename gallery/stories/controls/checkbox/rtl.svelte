<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <checkbox anchor="subject" text="إظهار أرقام الأسطر" checked enabled={!disabled} />
</box>
