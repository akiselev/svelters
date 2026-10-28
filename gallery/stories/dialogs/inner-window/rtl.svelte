<script>
  let { heading = 'مدير الطبقات' } = $props();
</script>

<box dir="rtl" flex-grow={1}>
  <inner-window anchor="subject" {heading} x={24} y={20} width={300} height={200} collapsible>
    <box padding={8} gap={4}>
      <checkbox text="F.Cu" checked />
      <checkbox text="B.Cu" checked />
      <checkbox text="F.SilkS" />
    </box>
  </inner-window>
</box>
