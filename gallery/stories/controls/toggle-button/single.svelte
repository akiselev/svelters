<script>
  let { checked = false, frame = true, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <toggle-button anchor="subject" text="Bold" {checked} {frame} enabled={!disabled} />
</box>
