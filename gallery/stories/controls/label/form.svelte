<script>
  let { disabled = false } = $props();
</script>

<grid padding={4} column-gap={8} row-gap={2} grid-template-columns="auto 1fr" align-items="center">
  <label anchor="subject" for="name" enabled={!disabled}>Name</label>
  <text-edit anchor="name" text="R12" />
  <label for="value">Value</label>
  <text-edit anchor="value" text="4.7k" />
</grid>
