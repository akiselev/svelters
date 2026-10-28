<script>
  let { disabled = false } = $props();
</script>

<box padding={8} flex-grow={1}>
  <tab-view anchor="subject" selected={1} flex-grow={1} enabled={!disabled}>
    <tab title="power.kicad_sch" closable><box padding={8}><label>power</label></box></tab>
    <tab title="usb-c-power-delivery-controller.kicad_sch" closable><box padding={8}><label>usb</label></box></tab>
    <tab title="board.kicad_pcb" closable><box padding={8}><label>board</label></box></tab>
    <tab title="rules.kicad_dru" closable><box padding={8}><label>rules</label></box></tab>
    <tab title="notes.md" closable><box padding={8}><label>notes</label></box></tab>
  </tab-view>
</box>
