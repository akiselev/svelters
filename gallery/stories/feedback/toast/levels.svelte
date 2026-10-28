<script>
  let { corner = 'bottom-right' } = $props();
  let host = $state();
  $effect(() => {
    if (!host) return;
    host.call('show', 'Saved board.kicad_pcb', 'info', 0);
    host.call('show', 'Design rules passed: 0 errors, 0 warnings', 'success', 0);
    host.call('show', 'Footprint R12 overlaps C3', 'warning', 0);
    host.call('show', 'The autorouter stopped: no route for net VCC_3V3', 'error', 0);
  });
</script>

<box padding={8} flex-grow={1}>
  <label>Board editor</label>
  <toast-host bind:this={host} anchor="subject" {corner} position="absolute" left={0} top={0} right={0} bottom={0} />
</box>
