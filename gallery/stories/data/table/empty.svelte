<script>
  let { disabled = false } = $props();
</script>

<box padding={8} flex-grow={1}>
  <table anchor="subject" accessible-name="Empty parts list" row-count={0} flex-grow={1} enabled={!disabled}>
    <table-column title="Reference" width={100} />
    <table-column title="Value" width={80} />
  </table>
</box>
