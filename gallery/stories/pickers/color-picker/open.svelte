<script>
  let { value = '#c0392b', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <color-picker anchor="subject" {value} open accessible-name="Copper colour" enabled={!disabled} />
</box>
