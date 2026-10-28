<script>
  let { 'weekday-format': weekdayFormat = 'short', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <calendar anchor="subject" value="2026-10-14" first-day="monday" weekday-format={weekdayFormat} accessible-name="Release date" enabled={!disabled} />
</box>
