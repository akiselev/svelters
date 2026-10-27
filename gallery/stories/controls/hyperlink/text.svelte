<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <hyperlink anchor="subject" text="Open the documentation" url="https://example.com/docs" enabled={!disabled} />
</box>
