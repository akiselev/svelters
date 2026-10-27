<script>
  let { checked = false, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <radio-button anchor="subject" text="Use the system theme" {checked} enabled={!disabled} />
</box>
