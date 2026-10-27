<script>
  let { checked = true, disabled = false } = $props();
</script>

<box padding={8}>
  <frame anchor="subject" label="Proxy" checkable {checked} gap={4} enabled={!disabled}>
    <text-edit text="proxy.example.com" />
    <text-edit text="8080" />
  </frame>
</box>
