<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <combo-box anchor="subject" accessible-name="الطبقة" options={['النحاس العلوي', 'داخلي ١', 'داخلي ٢', 'النحاس السفلي']} selected={1} open enabled={!disabled} />
</box>
