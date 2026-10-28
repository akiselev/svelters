<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <calendar anchor="subject" value="2026-10-14" min="2026-10-05" max="2026-10-23" calendar-week={false} first-day="sunday" accessible-name="Build window" enabled={!disabled} />
</box>
