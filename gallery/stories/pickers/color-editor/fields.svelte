<script>
  let { fields = 'hex rgb', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <color-editor anchor="subject" value="#c0392b" {fields} accessible-name="Net colour" enabled={!disabled} />
</box>
