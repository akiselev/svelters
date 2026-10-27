<script>
  let { disabled = false } = $props();
</script>

<box padding={12} flex-direction="row" gap={0} align-items="start">
  <toggle-button text="Left" radio group="align" />
  <toggle-button anchor="subject" text="Center" radio group="align" checked enabled={!disabled} />
  <toggle-button text="Right" radio group="align" />
  <toggle-button text="Justify" radio group="align" />
</box>
