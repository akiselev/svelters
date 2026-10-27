<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <combo-box anchor="subject" accessible-name="الوحدات" options={['مليمتر', 'بوصة']} selected={0} enabled={!disabled} />
</box>
