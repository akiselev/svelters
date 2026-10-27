<script>
  let { value = 'unchecked', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <checkbox
    anchor="subject"
    text="Show line numbers"
    checked={value === 'checked'}
    indeterminate={value === 'indeterminate'}
    enabled={!disabled}
  />
</box>
