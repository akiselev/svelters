<script>
  let { orientation = 'horizontal', disabled = false } = $props();
</script>

<box padding={8} flex-grow={1}>
  <split-view anchor="subject" {orientation} divider-position={140} flex-grow={1} enabled={!disabled}>
    <box padding={8}><label>Layers</label></box>
    <box padding={8}><label>Board</label></box>
  </split-view>
</box>
