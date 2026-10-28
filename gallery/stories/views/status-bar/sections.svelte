<script>
  let { message = '' } = $props();
</script>

<box flex-grow={1} justify-content="end">
  <status-bar anchor="subject" {message}>
    <label status-section="start">master</label>
    <label>2 warnings</label>
    <label status-section="end">Ln 12, Col 4</label>
    <label status-section="end">UTF-8</label>
  </status-bar>
</box>
