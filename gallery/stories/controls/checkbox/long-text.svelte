<script>
  let { disabled = false } = $props();
</script>

<box padding={12} align-items="start" width={336}>
  <checkbox anchor="subject" text="Automatically reload files that change on disk while they have no unsaved edits" checked enabled={!disabled} />
</box>
