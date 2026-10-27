<script>
  let { disabled = false } = $props();
</script>

<box padding={12} width={240}>
  <text-edit anchor="subject" text="/home/user/projects/power-supply/boards/main/layout.kicad_pcb" accessible-name="Path" enabled={!disabled} />
</box>
