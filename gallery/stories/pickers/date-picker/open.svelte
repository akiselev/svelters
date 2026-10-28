<script>
  let { value = '2026-10-14', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <date-picker anchor="subject" {value} open accessible-name="Release date" enabled={!disabled} />
</box>
