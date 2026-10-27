<script>
  // skelter has no number or quantity field kind: a narrow text field is what an app uses today.
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={6} align-items="center">
  <label>Width</label>
  <text-edit anchor="subject" text="12.50" width={72} accessible-name="Width" enabled={!disabled} />
  <label>mm</label>
</box>
