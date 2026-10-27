<script>
  let { text = 'Save the file', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <button anchor="subject" text="Save" tooltip={text} enabled={!disabled} />
</box>
