<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <progress-bar anchor="subject" accessible-name="Working" indeterminate enabled={!disabled} />
</box>
