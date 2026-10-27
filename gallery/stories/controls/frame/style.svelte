<script>
  let { style = 'group', disabled = false } = $props();
</script>

<box padding={8}>
  <frame anchor="subject" frame-style={style} gap={4} enabled={!disabled}>
    <label>Board outline</label>
    <checkbox text="Snap to grid" checked />
  </frame>
</box>
