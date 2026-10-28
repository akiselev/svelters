<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <split-view anchor="subject" sizes={[100, 180, 100]} flex-grow={1} enabled={!disabled}>
    <box padding={8}><label>المشروع</label></box>
    <box padding={8}><label>مخطط مزود الطاقة ومنظماته</label></box>
    <box padding={8}><label>الخصائص</label></box>
  </split-view>
</box>
