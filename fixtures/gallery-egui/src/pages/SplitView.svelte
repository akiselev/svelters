<script>
  // WG4a: <split-view> with bind:divider-position, and a nested vertical split.
  let position = $state(120);
  let dragging = $state(false);
</script>

<box gap={4} flex-grow={1}>
  <label>Position: {Math.round(position)}</label>
  <label>Dragging: {dragging}</label>
  <split-view
    accessible-name="Main split"
    bind:divider-position={position}
    min-size={60}
    flex-grow={1}
    ondrag-started={() => (dragging = true)}
    ondrag-stopped={() => (dragging = false)}
  >
    <box padding={6}><label>Tree pane</label></box>
    <split-view orientation="vertical" accessible-name="Editor split">
      <box padding={6}><label>Editor pane</label></box>
      <box padding={6}><label>Console pane</label></box>
    </split-view>
  </split-view>
  <button onclicked={() => (position = 200)}>Reset split</button>
</box>
