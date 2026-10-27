<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <combo-box anchor="subject" accessible-name="Layer" options={['Top copper', 'Inner 1', 'Inner 2', 'Bottom copper']} selected={1} open enabled={!disabled} />
</box>
