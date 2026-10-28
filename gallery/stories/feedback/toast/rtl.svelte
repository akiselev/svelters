<script>
  let host = $state();
  $effect(() => {
    if (!host) return;
    host.call('show', 'تم حفظ board.kicad_pcb', 'info', 0, ['فتح']);
    host.call('show', 'البصمة R12 تتداخل مع C3', 'warning', 0);
  });
</script>

<box dir="rtl" padding={8} flex-grow={1}>
  <label>محرر اللوحة</label>
  <toast-host bind:this={host} anchor="subject" position="absolute" left={0} top={0} right={0} bottom={0} />
</box>
