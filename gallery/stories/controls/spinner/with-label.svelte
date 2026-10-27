<script>
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={6} align-items="center">
  <spinner anchor="subject" accessible-name="Loading" enabled={!disabled} />
  <label>Loading libraries…</label>
</box>
