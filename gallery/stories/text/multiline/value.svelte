<script>
  let { text = '', disabled = false } = $props();
</script>

<box padding={8}>
  <text-edit anchor="subject" multiline hint-text="Notes for the assembler" {text} height={96} enabled={!disabled} />
</box>
