<script>
  let { padding = 12 } = $props();
</script>

<window title="Settings" {padding} gap={8}>
  <label text="Project settings" />
  <text-edit text="cadatomic" accessible-name="Name" />
  <checkbox text="Open the last project at start" checked />
  <box flex-direction="row" gap={8} justify-content="end">
    <button text="Cancel" />
    <button text="Save" default />
  </box>
</window>
