<script>
  let { disabled = false } = $props();
</script>

<box padding={8} gap={2} align-items="start">
  <radio-button text="Spaces" value="spaces" group="tabs" />
  <radio-button anchor="subject" text="Tabs" value="tabs" group="tabs" enabled={!disabled} />
  <radio-button text="Detect from the file's content when it opens" value="detect" group="tabs" />
</box>
