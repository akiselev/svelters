<script>
  let { selected = 6, menu = false, disabled = false } = $props();
  const files = ['power.kicad_sch', 'usb.kicad_sch', 'mcu.kicad_sch', 'board.kicad_pcb', 'rules.kicad_dru', 'bom.csv', 'notes.md', 'fab.gbr', 'drill.drl'];
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <tab-view anchor="subject" {selected} menu-open={menu} flex-grow={1} enabled={!disabled}>
    {#each files as f}
      <tab title={f} closable><box padding={8}><label>{f}</label></box></tab>
    {/each}
  </tab-view>
</box>
