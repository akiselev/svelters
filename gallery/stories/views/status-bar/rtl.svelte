<script>
  let { message = '' } = $props();
</script>

<box dir="rtl" flex-grow={1} justify-content="end">
  <status-bar anchor="subject" {message}>
    <label status-section="start">master</label>
    <label>تحذيران</label>
    <label status-section="end">سطر ١٢، عمود ٤</label>
    <label status-section="end">UTF-8</label>
  </status-bar>
</box>
