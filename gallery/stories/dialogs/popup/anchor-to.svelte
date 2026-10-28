<!-- The popup is declared after the toolbar, not inside it: `anchor-to` names the button it opens from. The toolbar sits at the window's bottom edge, so a `bottom-*` placement flips above the button. -->
<script>
  let { placement = 'bottom-start' } = $props();
</script>

<box padding={12} flex-grow={1} justify-content="end">
  <box flex-direction="row" gap={4}>
    <button text="Select" />
    <button text="Route" anchor="route" />
    <button text="Measure" />
  </box>
  <popup anchor-to="route" open {placement}>
    <box padding={8} gap={4}>
      <label>Single track</label>
      <label>Differential pair</label>
      <label>Bus</label>
    </box>
  </popup>
</box>
