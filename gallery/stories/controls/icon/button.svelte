<script>
  let { selected = false, disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <icon anchor="subject" family="" glyph="⚙" frame {selected} tooltip="Settings" enabled={!disabled} />
</box>
