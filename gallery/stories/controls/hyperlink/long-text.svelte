<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start" width={336}>
  <hyperlink anchor="subject" text="Read the full migration guide for projects created with an earlier version" url="https://example.com/migrate" enabled={!disabled} />
</box>
