<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <button anchor="subject" text="" accessible-name="Empty button" enabled={!disabled} />
</box>
