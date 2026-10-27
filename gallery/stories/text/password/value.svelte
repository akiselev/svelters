<script>
  let { text = '', disabled = false } = $props();
</script>

<box padding={12}>
  <text-edit anchor="subject" password hint-text="Password" {text} enabled={!disabled} />
</box>
