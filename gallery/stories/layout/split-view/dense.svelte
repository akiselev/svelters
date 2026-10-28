<script>
  let { disabled = false } = $props();
</script>

<box padding={4} flex-grow={1}>
  <split-view divider-position={120} flex-grow={1} enabled={!disabled}>
    <box padding={6} gap={4}>
      <label>Explorer</label>
      <label>board.kicad_pcb</label>
      <label>power.kicad_sch</label>
    </box>
    <split-view anchor="subject" orientation="vertical" divider-position={130} enabled={!disabled}>
      <box padding={6}><label>Editor</label></box>
      <box padding={6}><label>Problems: 2 warnings</label></box>
    </split-view>
  </split-view>
</box>
