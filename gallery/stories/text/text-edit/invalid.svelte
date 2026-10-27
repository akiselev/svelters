<script>
  let { disabled = false } = $props();
</script>

<box padding={12} gap={2}>
  <text-edit anchor="subject" text="4.7q" invalid accessible-name="Value" enabled={!disabled} />
  <label>Not a value: use a number and a unit prefix such as 4.7k.</label>
</box>
