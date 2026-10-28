<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <color-editor anchor="subject" value="#2e86c180" alpha="blend" swatches={['#c0392b', '#27ae60', '#2e86c1', '#f1c40f', '#8e44ad', '#000000', '#ffffff']} accessible-name="Layer colour" enabled={!disabled} />
</box>
