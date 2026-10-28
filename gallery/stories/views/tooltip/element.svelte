<script>
  let { text = 'Save the file' } = $props();
</script>

<box padding={12} align-items="start" gap={8}>
  <tooltip anchor="subject" {text} />
</box>
