<script>
  let { disabled = false } = $props();
</script>

<box padding={12}>
  <text-edit anchor="subject" password invalid text="hunter2" accessible-name="Password" enabled={!disabled} />
</box>
