<script>
  let { disabled = false } = $props();
</script>

<box padding={8}>
  <text-edit
    anchor="subject"
    multiline
    height={96}
    accessible-name="Description"
    text="A low-noise linear regulator for the analog rail. Keep the input capacitor within two millimetres of pin one and route the feedback trace away from the switching node."
    enabled={!disabled}
  />
</box>
