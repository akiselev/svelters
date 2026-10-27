<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <slider anchor="subject" text="Line height" min={1} max={3} step={0.1} value={1.4} enabled={!disabled} />
</box>
