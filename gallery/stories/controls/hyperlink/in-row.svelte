<script>
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={4} align-items="baseline">
  <label>No license found.</label>
  <hyperlink anchor="subject" text="Add one" url="https://example.com/license" enabled={!disabled} />
  <label>or ignore.</label>
</box>
