<script>
  let { format = '%Y-%m-%d', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <date-picker anchor="subject" value="2026-10-14" {format} accessible-name="Release date" enabled={!disabled} />
</box>
