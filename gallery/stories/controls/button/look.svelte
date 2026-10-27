<script>
  let { look = 'plain', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <button
    anchor="subject"
    text="Save changes"
    default={look === 'default'}
    frame={look !== 'frameless'}
    selected={look === 'selected'}
    small={look === 'small'}
    enabled={!disabled}
  />
</box>
