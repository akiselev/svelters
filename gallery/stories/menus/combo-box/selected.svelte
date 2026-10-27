<script>
  let { selected = 0, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <combo-box anchor="subject" accessible-name="Units" hint-text="Choose units" options={['Millimetres', 'Inches', 'Mils']} {selected} enabled={!disabled} />
</box>
