<script>
  let { disabled = false } = $props();
</script>

<box padding={4} gap={0} align-items="start">
  <checkbox text="Word wrap" checked />
  <checkbox anchor="subject" text="Minimap" enabled={!disabled} />
  <checkbox text="Breadcrumbs" checked />
  <checkbox text="Sticky scroll" indeterminate />
</box>
