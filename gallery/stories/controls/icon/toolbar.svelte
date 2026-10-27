<script>
  let { disabled = false } = $props();
</script>

<box padding={4} flex-direction="row" gap={2} align-items="center">
  <icon family="" glyph="◀" frame tooltip="Back" />
  <icon family="" glyph="▶" frame tooltip="Forward" />
  <separator orientation="vertical" />
  <icon anchor="subject" family="" glyph="✎" frame selected tooltip="Edit" enabled={!disabled} />
  <icon family="" glyph="✂" frame tooltip="Cut" />
  <icon family="" glyph="⌫" frame tooltip="Delete" />
</box>
