<script>
  let { disabled = false } = $props();
</script>

<box dir="rtl" padding={12} align-items="start">
  <hyperlink anchor="subject" text="افتح الوثائق" url="https://example.com/docs" enabled={!disabled} />
</box>
