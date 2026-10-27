<script>
  let { size = 16, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <icon anchor="subject" family="" glyph="★" {size} accessible-name="Favourite" enabled={!disabled} />
</box>
