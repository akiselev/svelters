<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <hyperlink anchor="subject" text="افتح الوثائق" url="https://example.com/docs" enabled={!disabled} />
</box>
