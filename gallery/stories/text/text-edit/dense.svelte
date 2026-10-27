<script>
  let { disabled = false } = $props();
</script>

<grid padding={2} column-gap={6} row-gap={1} grid-template-columns="auto 1fr">
  <label>X</label>
  <text-edit anchor="subject" text="10.16" enabled={!disabled} />
  <label>Y</label>
  <text-edit text="-3.81" />
</grid>
