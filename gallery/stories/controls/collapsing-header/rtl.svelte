<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={8}>
  <collapsing-header anchor="subject" text="إعدادات متقدمة" open enabled={!disabled}>
    <label>المحتوى</label>
  </collapsing-header>
</box>
