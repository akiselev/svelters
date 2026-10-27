<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <slider anchor="subject" accessible-name="Quality" min={0} max={100} value={50} marks="0:Low,50:Medium,100:High" snap-to-marks enabled={!disabled} />
</box>
