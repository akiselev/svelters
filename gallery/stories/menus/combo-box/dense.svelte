<script>
  let { disabled = false } = $props();
</script>

<box padding={2} flex-direction="row" gap={4} align-items="center">
  <label>Grid</label>
  <combo-box anchor="subject" accessible-name="Grid" options={['0.1 mm', '0.25 mm', '0.5 mm', '1 mm']} selected={2} enabled={!disabled} />
  <checkbox text="Snap" checked />
</box>
