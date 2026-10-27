<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <text-edit anchor="subject" text="Generated from the schematic" read-only enabled={!disabled} />
</box>
