<script>
  let { selected = 0, disabled = false } = $props();
</script>

<box padding={8} flex-grow={1}>
  <tab-view anchor="subject" {selected} flex-grow={1} enabled={!disabled}>
    <tab title="Schematic"><box padding={8}><label>Schematic page</label></box></tab>
    <tab title="Board"><box padding={8}><label>Board page</label></box></tab>
    <tab title="3D"><box padding={8}><label>3D page</label></box></tab>
  </tab-view>
</box>
