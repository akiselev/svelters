<script>
  let { disabled = false } = $props();
</script>

<box padding={8} flex-direction="row" gap={16} height={72}>
  <slider anchor="subject" accessible-name="Level" orientation="vertical" value={70} show-value={false} enabled={!disabled} />
  <slider accessible-name="Level 2" orientation="vertical" value={20} show-value={false} />
</box>
