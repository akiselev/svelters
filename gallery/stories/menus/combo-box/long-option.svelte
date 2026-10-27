<script>
  let { disabled = false } = $props();
</script>

<box padding={12} width={240}>
  <combo-box
    anchor="subject"
    accessible-name="Footprint"
    options={['SOIC-8_3.9x4.9mm_P1.27mm_with_thermal_pad', 'QFN-16']}
    selected={0}
    enabled={!disabled}
  />
</box>
