<script>
  let { value = 0, disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={24} flex-grow={1}>
  <scrollbar anchor="subject" {value} page={10} height={120} enabled={!disabled} />
  <box flex-grow={1} justify-content="end">
    <scrollbar anchor="subject" orientation="horizontal" {value} page={30} enabled={!disabled} />
  </box>
</box>
