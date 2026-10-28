<script>
  let { placement = 'bottom-start' } = $props();
</script>

<box padding={12} flex-grow={1} align-items="center" justify-content="center">
  <box>
    <button text="Layer options" />
    <popup anchor="subject" open {placement}>
      <box padding={8} gap={4}>
        <label>Show copper</label>
        <label>Show silkscreen</label>
      </box>
    </popup>
  </box>
</box>
