<script>
  let { active = false, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <switch anchor="subject" accessible-name="Auto save" {active} enabled={!disabled} />
</box>
