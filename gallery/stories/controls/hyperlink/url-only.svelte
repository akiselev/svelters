<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <hyperlink anchor="subject" url="https://example.com/releases/latest" enabled={!disabled} />
</box>
