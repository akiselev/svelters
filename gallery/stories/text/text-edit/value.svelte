<script>
  let { text = '', disabled = false } = $props();
</script>

<box padding={12}>
  <text-edit anchor="subject" hint-text="Reference designator" {text} enabled={!disabled} />
</box>
