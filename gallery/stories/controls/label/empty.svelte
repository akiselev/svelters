<script>
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={6} align-items="center">
  <label>[</label>
  <label anchor="subject" text="" enabled={!disabled} />
  <label>]</label>
</box>
