<script>
  let { ticks = 'after', disabled = false } = $props();
</script>

<box padding={12}>
  <slider anchor="subject" accessible-name="Zoom" min={0} max={10} step={1} value={4} tick-interval={1} {ticks} enabled={!disabled} />
</box>
