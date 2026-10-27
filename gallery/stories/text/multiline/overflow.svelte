<script>
  let { disabled = false } = $props();
  const text = Array.from({ length: 12 }, (_, i) => `Line ${i + 1}: net N${i + 1} routed`).join('\n');
</script>

<box padding={8}>
  <text-edit anchor="subject" multiline height={96} accessible-name="Log" {text} enabled={!disabled} />
</box>
