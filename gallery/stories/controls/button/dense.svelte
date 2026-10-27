<script>
  let { disabled = false } = $props();
</script>

<box padding={4} flex-direction="row" gap={2} align-items="center" width={340}>
  <button text="New" small />
  <button text="Open" small />
  <button anchor="subject" text="Save" small enabled={!disabled} />
  <separator orientation="vertical" />
  <button text="Undo" small />
  <button text="Redo" small />
  <button text="Find in files" small />
</box>
