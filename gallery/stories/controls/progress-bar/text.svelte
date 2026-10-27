<script>
  let { text = 'Indexing 1,204 of 3,000 files', disabled = false } = $props();
</script>

<box padding={12}>
  <progress-bar anchor="subject" fraction={0.4} {text} enabled={!disabled} />
</box>
