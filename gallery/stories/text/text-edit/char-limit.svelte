<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <text-edit anchor="subject" text="ABCD" char-limit={4} accessible-name="Code" enabled={!disabled} />
</box>
