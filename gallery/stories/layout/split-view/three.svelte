<script>
  let { disabled = false } = $props();
</script>

<box padding={8} flex-grow={1}>
  <split-view anchor="subject" sizes={[100, 180, 100]} flex-grow={1} enabled={!disabled}>
    <box padding={8}><label>Project</label></box>
    <box padding={8}><label>Schematic of the power supply and its regulators</label></box>
    <box padding={8}><label>Properties</label></box>
  </split-view>
</box>
