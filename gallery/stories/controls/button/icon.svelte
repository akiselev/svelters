<script>
  // `icon` is an icon registry key; the gallery host registers none of its own, so a missing
  // icon shows what the button does without one.
  let { caption = 'Save', disabled = false } = $props();
</script>

<box padding={12} align-items="start">
  <button anchor="subject" icon="file.save" text={caption} accessible-name="Save" enabled={!disabled} />
</box>
