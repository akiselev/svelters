<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start" width={336}>
  <switch anchor="subject" text="Send anonymous usage reports to help improve the editor" active enabled={!disabled} />
</box>
