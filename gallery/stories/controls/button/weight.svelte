<script>
  let { weight = 400, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <button anchor="subject" text="Continue" font-weight={weight} enabled={!disabled} />
</box>
